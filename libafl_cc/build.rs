use std::{env, fs::File, io::Write, path::Path, process::Command, str};

#[cfg(target_os = "macos")]
use glob::glob;

#[cfg(target_os = "macos")]
use std::path::PathBuf;

fn dll_extension<'a>() -> &'a str {
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windwos" => "dll",
        "macos" | "ios" => "dylib",
        _ => "so",
    }
}

/// Github Actions for `MacOS` seems to have troubles finding `llvm-config`.
/// Hence, we go look for it ourselves.
#[cfg(target_os = "macos")]
fn find_llvm_config_brew() -> Result<PathBuf, String> {
    match Command::new("brew").arg("--cellar").output() {
        Ok(output) => {
            let brew_cellar_location = str::from_utf8(&output.stdout).unwrap_or_default().trim();
            if brew_cellar_location.is_empty() {
                return Err("Empty return from brew --cellar".to_string());
            }
            let cellar_glob = format!("{}/llvm/*/bin/llvm-config", brew_cellar_location);
            let glob_results = glob(&cellar_glob).unwrap_or_else(|err| {
                panic!("Could not read glob path {} ({})", &cellar_glob, err);
            });
            match glob_results.last() {
                Some(path) => Ok(path.unwrap()),
                None => Err(format!(
                    "No llvm-config found in brew cellar with pattern {}",
                    cellar_glob
                )),
            }
        }
        Err(err) => Err(format!("Could not execute brew --cellar: {:?}", err)),
    }
}

fn find_llvm_config() -> String {
    env::var("LLVM_CONFIG").unwrap_or_else(|_| {
        // for Ghithub Actions, we check if we find llvm-config in brew.
        #[cfg(target_os = "macos")]
        match find_llvm_config_brew() {
            Ok(llvm_dir) => llvm_dir.to_str().unwrap().to_string(),
            Err(err) => {
                println!("cargo:warning={}", err);
                // falling back to system llvm-config
                "llvm-config".to_string()
            }
        }
        #[cfg(not(target_os = "macos"))]
        "llvm-config".to_string()
    })
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let src_dir = Path::new("src");

    let dest_path = Path::new(&out_dir).join("clang_constants.rs");
    let mut clang_constants_file = File::create(&dest_path).expect("Could not create file");

    let llvm_config = find_llvm_config();

    if let Ok(output) = Command::new(&llvm_config).args(&["--bindir"]).output() {
        let llvm_bindir = Path::new(
            str::from_utf8(&output.stdout)
                .expect("Invalid llvm-config output")
                .trim(),
        );

        write!(
            &mut clang_constants_file,
            "// These constants are autogenerated by build.rs

            pub const CLANG_PATH: &str = {:?};
            pub const CLANGXX_PATH: &str = {:?};
            ",
            llvm_bindir.join("clang"),
            llvm_bindir.join("clang++")
        )
        .expect("Could not write file");

        println!("cargo:rerun-if-changed=src/cmplog-routines-pass.cc");

        let output = Command::new(&llvm_config)
            .args(&["--cxxflags"])
            .output()
            .expect("Failed to execute llvm-config");
        let cxxflags = str::from_utf8(&output.stdout).expect("Invalid llvm-config output");

        let output = Command::new(&llvm_config)
            .args(&["--ldflags"])
            .output()
            .expect("Failed to execute llvm-config");
        let ldflags = str::from_utf8(&output.stdout).expect("Invalid llvm-config output");

        let cxxflags: Vec<&str> = cxxflags.trim().split_whitespace().collect();
        let mut ldflags: Vec<&str> = ldflags.trim().split_whitespace().collect();

        match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
            // Needed on macos.
            // Explanation at https://github.com/banach-space/llvm-tutor/blob/787b09ed31ff7f0e7bdd42ae20547d27e2991512/lib/CMakeLists.txt#L59
            "macos" | "ios" => {
                ldflags.push("-undefined");
                ldflags.push("dynamic_lookup");
            }
            _ => (),
        };

        let _ = Command::new(llvm_bindir.join("clang++"))
            .args(&cxxflags)
            .arg(src_dir.join("cmplog-routines-pass.cc"))
            .args(&ldflags)
            .args(&["-fPIC", "-shared", "-o"])
            .arg(out_dir.join(format!("cmplog-routines-pass.{}", dll_extension())))
            .status()
            .expect("Failed to compile cmplog-routines-pass.cc");
    } else {
        write!(
            &mut clang_constants_file,
            "// These constants are autogenerated by build.rs

pub const CLANG_PATH: &str = \"clang\";
pub const CLANGXX_PATH: &str = \"clang++\";
    "
        )
        .expect("Could not write file");

        println!(
            "cargo:warning=Failed to locate the LLVM path using {}, we will not build LLVM passes",
            llvm_config
        );
    }

    println!("cargo:rerun-if-changed=build.rs");
}