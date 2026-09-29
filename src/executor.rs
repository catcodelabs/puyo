use crate::deb::process_deb_file;
use crate::rpm::process_rpm_file;
use anyhow::{bail, Result};
use colored::*;
use std::path::Path;
use std::process::Command;

pub fn enforce_non_root() -> Result<()> {
    if let Ok(uid) = std::env::var("SUDO_UID") {
        if uid == "0" || std::env::var("USER").unwrap_or_default() == "root" {
            bail!("{}", "Error: Do not run puyo with sudo directly. Puyo will request sudo credentials only when invoking pacman.".red().bold());
        }
    }
    Ok(())
}

pub fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    println!(
        "{} {} {}",
        "==>".blue().bold(),
        "Executing:".bold(),
        format!("{} {}", cmd, args.join(" ")).dimmed()
    );

    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        bail!("Command '{}' failed with status: {}", cmd, status);
    }
    Ok(())
}

pub fn handle_deb_install(deb_path: &str) -> Result<()> {
    println!(
        "{} Processing .deb archive via internal puyo engine...",
        "==>".green().bold()
    );
    process_deb_file(Path::new(deb_path))?;
    Ok(())
}

pub fn handle_rpm_install(rpm_path: &str) -> Result<()> {
    println!(
        "{} Processing .rpm archive via internal puyo engine...",
        "==>".green().bold()
    );
    process_rpm_file(Path::new(rpm_path))?;
    Ok(())
}