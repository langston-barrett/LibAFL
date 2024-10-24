mod args;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
use {
    crate::args::Args,
    anyhow::{anyhow, Result},
    clap::Parser,
    nix::unistd::execvp,
    std::ffi::CString,
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use {
    crate::args::Args,
    anyhow::{anyhow, Result},
    clap::Parser,
    nix::{
        sys::{personality, personality::Persona},
        unistd::execvp,
    },
    std::ffi::CString,
};

#[cfg(any(target_os = "linux", target_os = "android"))]
fn disable_aslr() -> Result<()> {
    let mut persona = personality::get().map_err(|e| anyhow!("Failed to get personality: {e:}"))?;
    persona |= Persona::ADDR_NO_RANDOMIZE;
    personality::set(persona).map_err(|e| anyhow!("Failed to set personality: {e:}"))?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn disable_aslr() -> Result<()> {
    let mut status = libc::PROC_ASLR_FORCE_DISABLE;
    let r = unsafe {
        libc::procctl(
            libc::P_PID,
            0,
            libc::PROC_ASLR_CTL,
            &mut status as *mut i32 as *mut libc::c_void,
        )
    };
    if r < 0 {
        return Err(anyhow!("Failed to set aslr control"));
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    disable_aslr()?;

    let cargs = args
        .argv()
        .iter()
        .map(|x| CString::new(x.clone()).map_err(|e| anyhow!("Failed to read argument: {e:}")))
        .collect::<Result<Vec<CString>>>()?;

    execvp(&cargs[0], &cargs).map_err(|e| anyhow!("Failed to exceve: {e:}"))?;
    Ok(())
}
