use crate::deb::process_deb_file;
use crate::manifest::PuyoManifest;
use crate::rpm::process_rpm_file;
use anyhow::{bail, Result};
use colored::*;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct TransactionPipeline {
    pub update_native: bool,
    pub update_flatpak: bool,
    pub update_snap: bool,
    pub force: bool,
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
    pub not_found: Vec<(String, String)>,
}

impl TransactionPipeline {
    pub fn has_any_update(&self) -> bool {
        self.update_native || self.update_flatpak || self.update_snap
    }
}

pub fn enforce_non_root() -> Result<()> {
    if let Ok(uid) = std::env::var("SUDO_UID") {
        if uid == "0" || std::env::var("USER").unwrap_or_default() == "root" {
            bail!("{}", "ERROR: Do not run puyo with sudo directly. Puyo will request sudo credentials only when invoking pacman.".red().bold());
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

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = duration.subsec_millis();

    if hours > 0 {
        format!("{}h {}m {}.{:03}s", hours, minutes, seconds, millis)
    } else if minutes > 0 {
        format!("{}m {}.{:03}s", minutes, seconds, millis)
    } else {
        format!("{}.{:03}s", seconds, millis)
    }
}

pub fn handle_deb_install(deb_path: &str) -> Result<()> {
    println!(
        "{} Processing .deb archive",
        "==>".green().bold()
    );
    process_deb_file(Path::new(deb_path))?;
    Ok(())
}

pub fn handle_rpm_install(rpm_path: &str) -> Result<()> {
    println!(
        "{} Processing .rpm archive",
        "==>".green().bold()
    );
    process_rpm_file(Path::new(rpm_path))?;
    Ok(())
}

pub fn handle_manifest_install(target: &str) -> Result<()> {
    let target_path = Path::new(target);
    let build_dir = if target_path.exists() && target_path.is_dir() {
        std::fs::canonicalize(target_path)?
    } else if target.starts_with('/') || target.starts_with("./") || target.starts_with("../") {
        bail!("Local path does not exist or is not a directory: {}", target);
    } else {
        // Remote GitHub clone logic here...
        let temp_dir = env::temp_dir().join("puyo_build");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }

        let repo_url = if target.starts_with("http://") || target.starts_with("https://") {
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
            .args(["clone", "--depth", "1", &repo_url, temp_dir.to_str().unwrap()])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "echo")
            .status()?;

        if !status.success() {
            bail!("Failed to clone repository: {}", repo_url);
        }
        temp_dir
    };

    if build_dir.join(".puyo").exists() {
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
        bail!("Error: No .puyo manifest found in target directory: {:?}", build_dir);
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
            "puyo: No targets provided. Run 'puyo --help' for help or 'puyo' to update system.".yellow()
        );
        return Ok(());
    }

    // --- Pre-flight Existence Verification Pass ---
    let mut missing = Vec::new();

    for pkg in &pipeline.native_install {
        if pkg.ends_with(".deb") || pkg.ends_with(".rpm") {
            if !Path::new(pkg).exists() {
                missing.push((pkg.clone(), "Local File".to_string()));
            }
        } else {
            let status = Command::new("yay")
                .args(["-Si", pkg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !status {
                missing.push((pkg.clone(), "Native/AUR".to_string()));
            }
        }
    }

    for (_rem, inst) in &pipeline.swaps {
        let status = Command::new("yay")
            .args(["-Si", inst])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !status {
            missing.push((inst.clone(), "Atomic Swap Target".to_string()));
        }
    }

    for target in &pipeline.github_install {
        let path = Path::new(target);
        if path.exists() && path.is_dir() {
            if !path.join(".puyo").exists() {
                missing.push((target.clone(), "Local Manifest Directory (Missing .puyo)".to_string()));
            }
        }
    }

    if is_command_available("flatpak") {
        for pkg in &pipeline.flatpak_install {
            let status = Command::new("flatpak")
                .args(["info", pkg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !status {
                missing.push((pkg.clone(), "Flatpak".to_string()));
            }
        }
    }

    if is_command_available("snap") {
        for pkg in &pipeline.snap_install {
            let status = Command::new("snap")
                .args(["info", pkg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !status {
                missing.push((pkg.clone(), "Snap".to_string()));
            }
        }
    }

    let mut pipeline = pipeline;
    pipeline.not_found = missing;

    display_preflight_summary(&pipeline);

    if !pipeline.not_found.is_empty() {
        print!("{} One or more target packages could not be found. Proceed anyway? [y/N]: ", "Warning:".yellow().bold());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("{}", "Transaction cancelled by user due to missing packages.".yellow());
            return Ok(());
        }
    }

    if pipeline.dry_run {
        println!("{}", "[dry-run] Execution plan printed. No changes were made.".cyan().bold());
        return Ok(());
    }

    if !confirm_prompt("Proceed with transaction pipeline?")? {
        println!("{}", "Transaction cancelled by user.".yellow());
        return Ok(());
    }

    println!("{} Authenticating sudo session...", "==>".blue().bold());
    let sudo_status = Command::new("sudo").arg("-v").status()?;
    if !sudo_status.success() {
        bail!("Sudo authentication failed.");
    }

    let pipeline_start = Instant::now();
    let mut failed_steps: Vec<(&'static str, Vec<String>)> = Vec::new();

    // 1. System Upgrades Pass
    if pipeline.update_native {
        let bucket_timer = Instant::now();
        println!("{} Running native system updates...", "==>".green().bold());
        let res = run_command("yay", &["-Syu", "--noconfirm"]);
        println!("  {} Completed in {}", "[Native Update]".dimmed(), format_duration(bucket_timer.elapsed()).green());

        if let Err(e) = res {
            if !pipeline.force {
                bail!("Native system update failed: {}", e);
            }
            eprintln!("{}", "Warning: Native update failed. Continuing due to --force...".yellow());
        }
    }

    if pipeline.update_flatpak && is_command_available("flatpak") {
        let bucket_timer = Instant::now();
        println!("{} Updating Flatpaks...", "==>".green().bold());
        let _ = run_command("flatpak", &["update", "-y"]);
        println!("  {} Completed in {}", "[Flatpak Update]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    if pipeline.update_snap && is_command_available("snap") {
        let bucket_timer = Instant::now();
        println!("{} Updating Snaps...", "==>".green().bold());
        let _ = run_command("sudo", &["snap", "refresh"]);
        println!("  {} Completed in {}", "[Snap Update]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 2. Native Removal Pass
    if !pipeline.native_remove.is_empty() {
        let bucket_timer = Instant::now();
        let refs: Vec<&str> = pipeline.native_remove.iter().map(|s| s.as_str()).collect();
        let mut args = vec!["pacman", "-Rns", "--noconfirm"];
        args.extend(refs);

        if let Err(e) = run_command("sudo", &args) {
            if !pipeline.force {
                bail!("Native removal failed: {}", e);
            }
            failed_steps.push(("Native Remove", pipeline.native_remove.clone()));
        }
        println!("  {} Completed in {}", "[Native Remove]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 3. Manifest / GitHub Installs Pass
    if !pipeline.github_install.is_empty() {
        let bucket_timer = Instant::now();
        for target in &pipeline.github_install {
            if let Err(e) = handle_manifest_install(target) {
                if !pipeline.force {
                    bail!("Manifest installation failed for target '{}': {}", target, e);
                }
                failed_steps.push(("GitHub Install", vec![target.clone()]));
            }
        }
        println!("  {} Completed in {}", "[Manifest Install]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 4. Atomic Swap Pass
    if !pipeline.swaps.is_empty() {
        let bucket_timer = Instant::now();
        for (rem, inst) in &pipeline.swaps {
            println!("{} Executing atomic swap: {} ➔ {}", "==>".green().bold(), rem.red(), inst.green());
            
            let remove_status = Command::new("sudo")
                .args(["pacman", "-Rdd", "--noconfirm", rem])
                .status()?;

            if !remove_status.success() {
                if !pipeline.force {
                    bail!("Atomic swap failed: could not remove old package '{}'", rem);
                }
                eprintln!("{}", format!("Warning: Failed to remove '{}', continuing due to --force...", rem).yellow());
            }

            let install_status = Command::new("yay")
                .args(["-S", "--noconfirm", inst])
                .status()?;

            if !install_status.success() {
                if !pipeline.force {
                    bail!("Atomic swap failed: could not install replacement package '{}'", inst);
                }
                failed_steps.push(("Atomic Swap Install", vec![inst.clone()]));
            }
        }
        println!("  {} Completed in {}", "[Atomic Swap]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 5. Native Installation Pass
    if !pipeline.native_install.is_empty() {
        let bucket_timer = Instant::now();
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
            let mut args = vec!["yay", "-S", "--noconfirm", "--answerclean", "All", "--answerdiff", "All"];
            args.extend(regular_pkgs);
            let cmd_str = args.join(" ");

            println!(
                "{} {} {}",
                "==>".blue().bold(),
                "Executing:".bold(),
                cmd_str.dimmed()
            );

            let status = Command::new("sh")
                .arg("-c")
                .arg(&cmd_str)
                .status()?;

            if !status.success() {
                if !pipeline.force {
                    bail!("Native installation batch failed.");
                }
                failed_steps.push(("Native Install", pipeline.native_install.clone()));
            }
        }
        println!("  {} Completed in {}", "[Native Install]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 6. Flatpak Passes
    if !pipeline.flatpak_remove.is_empty() || !pipeline.flatpak_install.is_empty() {
        let bucket_timer = Instant::now();
        if !pipeline.flatpak_remove.is_empty() {
            let refs: Vec<&str> = pipeline.flatpak_remove.iter().map(|s| s.as_str()).collect();
            let mut args = vec!["uninstall", "-y"];
            args.extend(refs);

            if let Err(_) = run_command("flatpak", &args) {
                if !pipeline.force {
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
                if !pipeline.force {
                    bail!("Flatpak install batch failed.");
                }
                failed_steps.push(("Flatpak Install", pipeline.flatpak_install.clone()));
            }
        }
        println!("  {} Completed in {}", "[Flatpak Pass]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 7. Snap Passes
    if !pipeline.snap_remove.is_empty() || !pipeline.snap_install.is_empty() {
        let bucket_timer = Instant::now();
        for target in &pipeline.snap_remove {
            let _ = run_command("sudo", &["snap", "remove", target]);
        }
        for target in &pipeline.snap_install {
            let _ = run_command("sudo", &["snap", "install", target]);
        }
        println!("  {} Completed in {}", "[Snap Pass]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // 8. Cleanup Pass
    if pipeline.clean {
        let bucket_timer = Instant::now();
        let _ = execute_cleanup();
        println!("  {} Completed in {}", "[Clean Pass]".dimmed(), format_duration(bucket_timer.elapsed()).green());
    }

    // Granular Fallback Loop for --force failures
    if !failed_steps.is_empty() && pipeline.force {
        println!("\n{}", "==> Phase 2: Retrying failed batch targets individually (--force active)...".yellow().bold());
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

    println!(
        "\n{} Pipeline execution finished in {}.",
        "[puyo]".green().bold(),
        format_duration(pipeline_start.elapsed()).bold()
    );
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
        println!("  {} {}", "[Manifest Install]".green().bold(), p.github_install.join(", "));
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
    if !p.not_found.is_empty() {
        let missing_str: Vec<String> = p
            .not_found
            .iter()
            .map(|(pkg, engine)| format!("{} ({})", pkg, engine))
            .collect();
        println!("  {} {}", "[Not Found]".red().bold(), missing_str.join(", "));
    }
    if p.clean {
        println!("  {} Sweep orphans, pacman cache, and flatpak runtimes", "[Clean]".cyan());
    }
    if p.force {
        println!("  {} Granular retry mode enabled (--force)", "[Mode]".bold());
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
        && p.swaps.is_empty()
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