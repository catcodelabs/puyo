use crate::deb::process_deb_file;
use crate::manifest::PuyoManifest;
use crate::rpm::process_rpm_file;
use anyhow::{bail, Result};
use colored::*;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Default)]
pub struct TransactionPipeline {
    pub update_native: bool,
    pub update_flatpak: bool,
    pub update_snap: bool,
    pub who_cares: bool,
    pub clean: bool,
    pub dry_run: bool,
    pub passthrough_args: Vec<String>,
    pub swaps: Vec<(String, String)>,
    pub native_install: Vec<String>,
    pub github_install: Vec<String>,
    pub native_remove: Vec<String>,
    pub flatpak_install: Vec<String>,
    pub flatpak_remove: Vec<String>,
    pub snap_install: Vec<String>,
    pub snap_remove: Vec<String>,
    pub search_queries: Vec<String>,
}

impl TransactionPipeline {
    pub fn has_any_update(&self) -> bool {
        self.update_native || self.update_flatpak || self.update_snap
    }
}

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

pub fn handle_github_install(target: &str) -> Result<()> {
    let build_dir = env::temp_dir().join("puyo_build");
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)?;
    }

    let repo_url = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else if Path::new(target).exists() || target.starts_with('/') || target.starts_with("./") {
        target.to_string()
    } else {
        format!("https://github.com/{}.git", target)
    };

    println!(
        "{} Cloning repository: {}",
        "==>".blue().bold(),
        repo_url
    );

    let status = Command::new("git")
        .args(["clone", "--depth", "1", &repo_url, build_dir.to_str().unwrap()])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo")
        .status()?;

    if !status.success() {
        bail!("Failed to clone repository: {}", repo_url);
    }

    if Path::new(&build_dir).join(".puyo").exists() {
        println!(
            "{} Found .puyo manifest! Synthesizing PKGBUILD...",
            "==>".green().bold()
        );
        let manifest = PuyoManifest::load_from_dir(&build_dir)?;
        let pkgbuild_content = manifest.generate_pkgbuild();

        std::fs::write(build_dir.join("PKGBUILD"), pkgbuild_content)?;

        let orig_dir = env::current_dir()?;
        env::set_current_dir(&build_dir)?;
        run_command("makepkg", &["-si"])?;
        env::set_current_dir(orig_dir)?;
    } else {
        println!(
            "{}",
            "Warning: No .puyo manifest found in repository root.".yellow()
        );
    }

    Ok(())
}

pub fn execute_pipeline(pipeline: TransactionPipeline) -> Result<()> {
    // 0. Immediate Native Passthrough Pass
    if !pipeline.passthrough_args.is_empty() {
        let mut args: Vec<&str> = pipeline.passthrough_args.iter().map(|s| s.as_str()).collect();
        let targets: Vec<&str> = pipeline.native_install.iter().map(|s| s.as_str()).collect();
        args.extend(targets);

        if pipeline.dry_run {
            println!(
                "{} Pass-through command: yay {}",
                "[dry-run]".cyan().bold(),
                args.join(" ")
            );
            return Ok(());
        }

        return run_command("yay", &args);
    }

    if !pipeline.search_queries.is_empty() {
        for query in &pipeline.search_queries {
            println!("{} Searching Native/AUR: {}", "==>".green().bold(), query);
            let _ = run_command("yay", &["-Ss", query]);

            if is_command_available("flatpak") {
                println!("{} Searching Flatpak: {}", "==>".green().bold(), query);
                let _ = run_command("flatpak", &["search", query]);
            }
            if is_command_available("snap") {
                println!("{} Searching Snap: {}", "==>".green().bold(), query);
                let _ = run_command("snap", &["find", query]);
            }
        }
        return Ok(());
    }

    if is_pipeline_empty(&pipeline) {
        println!(
            "{}",
            "puyo: No targets provided. Run 'puyo --help' or 'puyo' to update system.".yellow()
        );
        return Ok(());
    }

    display_preflight_summary(&pipeline);

    if pipeline.dry_run {
        println!("{}", "[dry-run] Execution plan printed. No changes were made.".cyan().bold());
        return Ok(());
    }

    if !confirm_prompt("Proceed with transaction pipeline?")? {
        println!("{}", "Transaction cancelled by user.".yellow());
        return Ok(());
    }

    let mut failed_steps: Vec<(&'static str, Vec<String>)> = Vec::new();

    // 1. System Upgrades Pass
    if pipeline.update_native {
        println!("{} Running native system updates...", "==>".green().bold());
        if let Err(e) = run_command("yay", &["-Syu", "--noconfirm"]) {
            if !pipeline.who_cares {
                bail!("Native system update failed: {}", e);
            }
            eprintln!("{}", "Warning: Native update failed. Continuing due to --who-cares...".yellow());
        }
    }

    if pipeline.update_flatpak && is_command_available("flatpak") {
        println!("{} Updating Flatpaks...", "==>".green().bold());
        let _ = run_command("flatpak", &["update", "-y"]);
    }

    if pipeline.update_snap && is_command_available("snap") {
        println!("{} Updating Snaps...", "==>".green().bold());
        let _ = run_command("sudo", &["snap", "refresh"]);
    }

    // 2. Native Removal Pass
    if !pipeline.native_remove.is_empty() {
        let refs: Vec<&str> = pipeline.native_remove.iter().map(|s| s.as_str()).collect();
        let mut args = vec!["pacman", "-Rns", "--noconfirm"];
        args.extend(refs);

        if let Err(e) = run_command("sudo", &args) {
            if !pipeline.who_cares {
                bail!("Native removal failed: {}", e);
            }
            failed_steps.push(("Native Remove", pipeline.native_remove.clone()));
        }
    }

    // 3. GitHub Manifest Installs Pass
    if !pipeline.github_install.is_empty() {
        for target in &pipeline.github_install {
            if let Err(e) = handle_github_install(target) {
                if !pipeline.who_cares {
                    bail!("GitHub installation failed for target '{}': {}", target, e);
                }
                failed_steps.push(("GitHub Install", vec![target.clone()]));
            }
        }
    }

    // 4. Native Installation Pass
    if !pipeline.native_install.is_empty() {
        let mut regular_pkgs = Vec::new();

        for pkg in &pipeline.native_install {
            if pkg.ends_with(".deb") {
                let _ = handle_deb_install(pkg);
            } else if pkg.ends_with(".rpm") {
                let _ = handle_rpm_install(pkg);
            } else {
                regular_pkgs.push(pkg.as_str());
            }
        }

        if !regular_pkgs.is_empty() {
            let mut args = vec!["-S", "--noconfirm"];
            args.extend(regular_pkgs);

            if let Err(_) = run_command("yay", &args) {
                if !pipeline.who_cares {
                    bail!("Native installation batch failed.");
                }
                failed_steps.push(("Native Install", pipeline.native_install.clone()));
            }
        }
    }

    // 5. Flatpak Passes
    if !pipeline.flatpak_remove.is_empty() {
        let refs: Vec<&str> = pipeline.flatpak_remove.iter().map(|s| s.as_str()).collect();
        let mut args = vec!["uninstall", "-y"];
        args.extend(refs);

        if let Err(_) = run_command("flatpak", &args) {
            if !pipeline.who_cares {
                bail!("Flatpak removal batch failed.");
            }
            failed_steps.push(("Flatpak Remove", pipeline.flatpak_remove.clone()));
        }
    }

    if !pipeline.flatpak_install.is_empty() {
        let refs: Vec<&str> = pipeline.flatpak_install.iter().map(|s| s.as_str()).collect();
        let mut args = vec!["install", "-y"];
        args.extend(refs);

        if let Err(_) = run_command("flatpak", &args) {
            if !pipeline.who_cares {
                bail!("Flatpak install batch failed.");
            }
            failed_steps.push(("Flatpak Install", pipeline.flatpak_install.clone()));
        }
    }

    // 6. Snap Passes
    if !pipeline.snap_remove.is_empty() {
        for target in &pipeline.snap_remove {
            let _ = run_command("sudo", &["snap", "remove", target]);
        }
    }

    if !pipeline.snap_install.is_empty() {
        for target in &pipeline.snap_install {
            let _ = run_command("sudo", &["snap", "install", target]);
        }
    }

    // 7. Cleanup Pass
    if pipeline.clean {
        let _ = execute_cleanup();
    }

    // Granular Fallback Loop for --who-cares failures
    if !failed_steps.is_empty() && pipeline.who_cares {
        println!("\n{}", "==> Phase 2: Retrying failed batch targets individually (--who-cares active)...".yellow().bold());
        for (action, targets) in failed_steps {
            for target in targets {
                match action {
                    "Native Install" => {
                        println!("Retrying individual install: {}", target);
                        let _ = run_command("yay", &["-S", &target, "--noconfirm"]);
                    }
                    "Native Remove" => {
                        println!("Retrying individual removal: {}", target);
                        let _ = run_command("sudo", &["pacman", "-Rns", &target, "--noconfirm"]);
                    }
                    "Flatpak Install" => {
                        let _ = run_command("flatpak", &["install", "-y", &target]);
                    }
                    "Flatpak Remove" => {
                        let _ = run_command("flatpak", &["uninstall", "-y", &target]);
                    }
                    _ => {}
                }
            }
        }
    }

    println!("\n{}", "[puyo] Pipeline execution finished.".green().bold());
    Ok(())
}

fn execute_cleanup() -> Result<()> {
    println!("{}", "==> Executing system cleanup sweeps...".cyan().bold());

    println!("Checking for orphaned native packages...");
    if let Ok(output) = Command::new("pacman").args(["-Qtdq"]).output() {
        let orphans = String::from_utf8_lossy(&output.stdout);
        let orphan_list: Vec<&str> = orphans.lines().collect();

        if !orphan_list.is_empty() {
            println!("Removing orphans: {}", orphan_list.join(" "));
            let mut args = vec!["pacman", "-Rns", "--noconfirm"];
            args.extend(orphan_list);
            let _ = run_command("sudo", &args);
        } else {
            println!("No native orphan packages found.");
        }
    }

    println!("Pruning package cache...");
    let _ = run_command("yay", &["-Sc", "--noconfirm"]);

    if is_command_available("flatpak") {
        println!("Cleaning unused Flatpak runtimes...");
        let _ = run_command("flatpak", &["uninstall", "--unused", "-y"]);
    }

    println!("{}", "[puyo] Cleanup complete!".green().bold());
    Ok(())
}

fn display_preflight_summary(p: &TransactionPipeline) {
    println!("\n{}", "================ [ puyo Execution Plan ] ================".cyan().bold());

    if p.update_native && p.update_flatpak && p.update_snap {
        println!("  {} Update all repositories & backends", "[Update]".yellow());
    } else {
        if p.update_native {
            println!("  {} Update Native/AUR packages", "[Native Update]".yellow());
        }
        if p.update_flatpak {
            println!("  {} Update Flatpak runtimes & apps", "[Flatpak Update]".yellow());
        }
        if p.update_snap {
            println!("  {} Update Snap packages", "[Snap Update]".yellow());
        }
    }

    if !p.swaps.is_empty() {
        for (rem, inst) in &p.swaps {
            println!("  {} {} ➔ {}", "[Atomic Swap]".yellow().bold(), rem.red(), inst.green());
        }
    }

    // Filter out swap packages from standard remove/install lists for clean reporting
    let swap_removes: Vec<&str> = p.swaps.iter().map(|(r, _)| r.as_str()).collect();
    let swap_installs: Vec<&str> = p.swaps.iter().map(|(_, i)| i.as_str()).collect();

    let filtered_native_remove: Vec<&str> = p.native_remove
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !swap_removes.contains(s))
        .collect();

    let filtered_native_install: Vec<&str> = p.native_install
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !swap_installs.contains(s))
        .collect();

    if !filtered_native_remove.is_empty() {
        println!("  {} {}", "[Native Remove]".red(), filtered_native_remove.join(", "));
    }
    if !filtered_native_install.is_empty() {
        println!("  {} {}", "[Native Install]".green(), filtered_native_install.join(", "));
    }
    if !p.github_install.is_empty() {
        println!("  {} {}", "[GitHub Install]".green().bold(), p.github_install.join(", "));
    }
    if !p.flatpak_remove.is_empty() {
        println!("  {} {}", "[Flatpak Remove]".red(), p.flatpak_remove.join(", "));
    }
    if !p.flatpak_install.is_empty() {
        println!("  {} {}", "[Flatpak Install]".green(), p.flatpak_install.join(", "));
    }
    if !p.snap_remove.is_empty() {
        println!("  {} {}", "[Snap Remove]".red(), p.snap_remove.join(", "));
    }
    if !p.snap_install.is_empty() {
        println!("  {} {}", "[Snap Install]".green(), p.snap_install.join(", "));
    }
    if p.clean {
        println!("  {} Sweep orphans, pacman cache, and flatpak runtimes", "[Clean]".cyan());
    }
    if p.who_cares {
        println!("  {} Granular retry mode enabled (--who-cares)", "[Mode]".bold());
    }
    if p.dry_run {
        println!("  {} DRY RUN MODE (No changes will be applied)", "[Mode]".cyan().bold());
    }
    println!("{}\n", "========================================================".cyan().bold());
}

fn is_pipeline_empty(p: &TransactionPipeline) -> bool {
    !p.has_any_update()
        && !p.clean
        && p.passthrough_args.is_empty()
        && p.native_install.is_empty()
        && p.github_install.is_empty()
        && p.native_remove.is_empty()
        && p.flatpak_install.is_empty()
        && p.flatpak_remove.is_empty()
        && p.snap_install.is_empty()
        && p.snap_remove.is_empty()
}

fn confirm_prompt(prompt: &str) -> Result<bool> {
    print!("{} [Y/n]: ", prompt.bold());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();
    Ok(trimmed.is_empty() || trimmed == "y" || trimmed == "yes")
}

fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}