mod deb;
mod executor;
mod manifest;
mod rpm;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use executor::{
    enforce_non_root, handle_deb_install, handle_rpm_install, run_command,
};
use manifest::PuyoManifest;
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "puyo",
    author = "CatCodeLabs",
    version = "0.1.0",
    about = "puyo - Universal Meta-Wrapper for Arch Linux"
)]
struct Cli {
    #[arg(short = 'S', long)]
    sync: bool,

    #[arg(short = 'y', long)]
    refresh: bool,

    #[arg(short = 'u', long)]
    sysupgrade: bool,

    #[arg(short = 'R', long)]
    remove: bool,

    #[arg(short = 's', long)]
    search: bool,

    targets: Vec<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install { targets: Vec<String> },
    Upgrade,
    Remove { targets: Vec<String> },
    Search { query: String },
    Deb { path: String },
    Rpm { path: String },
}
fn main() -> Result<()> {
    enforce_non_root()?;
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Install { targets } => return handle_install(targets),
            Commands::Upgrade => return run_command("yay", &["-Syu"]),
            Commands::Remove { targets } => {
                let mut args = vec!["pacman", "-Rns"];
                let refs: Vec<&str> = targets.iter().map(|s| s.as_str()).collect();
                args.extend(refs);
                return run_command("sudo", &args);
            }
            Commands::Search { query } => return run_command("yay", &["-Ss", &query]),
            Commands::Deb { path } => return handle_deb_install(&path),
            Commands::Rpm { path } => return handle_rpm_install(&path),
        }
    }

    if cli.sync {
        if cli.refresh && cli.sysupgrade {
            return run_command("yay", &["-Syu"]);
        } else if !cli.targets.is_empty() {
            return handle_install(cli.targets);
        }
    } else if cli.remove && !cli.targets.is_empty() {
        let mut args = vec!["pacman", "-Rns"];
        let refs: Vec<&str> = cli.targets.iter().map(|s| s.as_str()).collect();
        args.extend(refs);
        return run_command("sudo", &args);
    } else if cli.search && !cli.targets.is_empty() {
        return run_command("yay", &["-Ss", &cli.targets[0]]);
    }

    println!(
        "{}",
        "puyo: No valid command or flags provided. Use --help for usage.".yellow()
    );
    Ok(())
}

fn handle_install(targets: Vec<String>) -> Result<()> {
    for target in targets {
        if target.ends_with(".deb") {
            println!("{} Local .deb file: {}", "==>".green().bold(), target);
            handle_deb_install(&target)?;
        } else if target.ends_with(".rpm") {
            println!("{} Local .rpm file: {}", "==>".green().bold(), target);
            handle_rpm_install(&target)?;
        } else if target.starts_with("https://github.com/") || target.contains('/') {
            println!(
                "{} GitHub repository target: {}",
                "==>".green().bold(),
                target
            );
            handle_github_install(&target)?;
        } else {
            println!(
                "{} Routing target to yay/pacman: {}",
                "==>".green().bold(),
                target
            );
            run_command("yay", &["-S", &target])?;
        }
    }
    Ok(())
}

fn handle_github_install(target: &str) -> Result<()> {
    let build_dir = std::env::temp_dir().join("puyo_build");
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
    }

    // Determine target URL/Path correctly
    let repo_url = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else if Path::new(target).exists() || target.starts_with('/') || target.starts_with("./") {
        // Local path
        target.to_string()
    } else {
        // GitHub owner/repo shorthand (e.g., "catcodelab/puyo")
        format!("https://github.com/{}.git", target)
    };

    println!(
        "{} {} {}",
        "==>".blue().bold(),
        "Executing:".bold(),
        format!("git clone --depth 1 {} {:?}", repo_url, build_dir).dimmed()
    );

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", &repo_url, build_dir.to_str().unwrap()])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo")
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to clone repository: {}", repo_url);
    }

    if Path::new(&build_dir).join(".puyo").exists() {
        println!(
            "{} Found .puyo manifest! Synthesizing PKGBUILD...",
            "==>".green().bold()
        );
        let manifest = PuyoManifest::load_from_dir(&build_dir)?;
        let pkgbuild_content = manifest.generate_pkgbuild();

        std::fs::write(build_dir.join("PKGBUILD"), pkgbuild_content)?;

        let orig_dir = std::env::current_dir()?;
        std::env::set_current_dir(&build_dir)?;
        run_command("makepkg", &["-si"])?;
        std::env::set_current_dir(orig_dir)?;
    } else {
        println!(
            "{}",
            "Warning: No .puyo manifest found in repository root.".yellow()
        );
    }

    Ok(())
}