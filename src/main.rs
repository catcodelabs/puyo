mod config;
mod deb;
mod executor;
mod manifest;
mod rpm;

use anyhow::Result;
use executor::{enforce_non_root, execute_pipeline, TransactionPipeline};
use std::env;
use std::path::Path;
use colored::*;

#[derive(PartialEq)]
enum EngineTarget {
    Native,
    Flatpak,
    Snap,
}

enum OperationMode {
    Install,
    Remove,
    Search,
}

fn main() -> Result<()> {
    enforce_non_root()?;

    let raw_args: Vec<String> = env::args().skip(1).collect();
    let mut pipeline = TransactionPipeline::default();

    if raw_args.is_empty() {
        pipeline.update_native = true;
        pipeline.update_flatpak = true;
        pipeline.update_snap = true;
        return execute_pipeline(pipeline);
    }

    let mut current_engine = EngineTarget::Native;
    let mut current_mode = OperationMode::Install;
    let mut iter = raw_args.into_iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--force" | "-f" => {
                pipeline.force = true;
            }
            "--dry-run" => {
                pipeline.dry_run = true;
            }
            "clean" | "-C" => {
                pipeline.clean = true;
            }
            "init" => {
                initialize_config()?;
                return Ok(());
            }
            "--version" | "-V" => {
                let splashes = [
                    "Your friendly neighborhood package manager!",
                    "Smooth pipelines, zero friction.",
                    "Now bathed in magnificent magenta!",
                    "Unstoppable power, pocket-sized simplicity.",
                    "Ready to wrangle your packages.",
                    "Up to date and looking great.",
                    "Puyo Unifies Your Operations!",
                    "AUR, Flatpak, Snap, and beyond.",
                    "Brewing fresh binaries since v1.2.0.",
                    "Ready for action, commander."
                ];

// Get true high-entropy randomness using memory address (ASLR), PID, and nanoseconds
                let stack_var = 0;
                let memory_entropy = &stack_var as *const i32 as usize;
                let seed = memory_entropy
                    .wrapping_add(std::process::id() as usize)
                    .wrapping_add(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize,
                    );
                let splash = splashes[seed % splashes.len()];

                println!("Running puyo v{}", env!("CARGO_PKG_VERSION").white());
                println!("{}", splash.magenta().bold());
                return Ok(());
            }

            // Full System Update
            "update" | "upgrade" | "-Syu" | "-yu" | "-u" => {
                pipeline.update_native = true;
                pipeline.update_flatpak = true;
                pipeline.update_snap = true;
            }

            // Backend-Specific Updates
            "nativeupdate" | "pacman-update" => {
                pipeline.update_native = true;
            }
            "flatupdate" | "flatpak-update" | "flatupgrade" => {
                pipeline.update_flatpak = true;
            }
            "snapupdate" | "snap-update" | "snaprefresh" => {
                pipeline.update_snap = true;
            }

            // Engine Switches (Contextual modifiers — default mode resets to Install)
            "flat" | "flatpak" => {
                current_engine = EngineTarget::Flatpak;
                current_mode = OperationMode::Install;
            }
            "snap" => {
                current_engine = EngineTarget::Snap;
                current_mode = OperationMode::Install;
            }
            "native" | "arch" | "aur" => {
                current_engine = EngineTarget::Native;
                current_mode = OperationMode::Install;
            }

            // Universal Operational Mode verbs/flags
            "install" | "-S" => {
                current_mode = OperationMode::Install;
            }
            "remove" | "-R" | "-Rs" | "-Rn" | "-Rns" => {
                current_mode = OperationMode::Remove;
            }
            "search" | "-s" | "-Ss" => {
                current_mode = OperationMode::Search;
            }

            // One-Shot Engine Prefix Verbs
            "flatinstall" | "flatpak-install" => {
                current_engine = EngineTarget::Flatpak;
                current_mode = OperationMode::Install;
            }
            "flatremove" | "flatpak-remove" | "flatuninstall" => {
                current_engine = EngineTarget::Flatpak;
                current_mode = OperationMode::Remove;
            }
            "snapinstall" | "snap-install" => {
                current_engine = EngineTarget::Snap;
                current_mode = OperationMode::Install;
            }
            "snapremove" | "snap-remove" | "snapuninstall" => {
                current_engine = EngineTarget::Snap;
                current_mode = OperationMode::Remove;
            }

            "swap" => {
                let target_remove = iter.next();
                let target_install = iter.next();

                if let (Some(rem), Some(inst)) = (target_remove, target_install) {
                    pipeline.swaps.push((rem.clone(), inst.clone()));
                } else {
                    eprintln!("Error: 'swap' requires exactly two arguments: <old_pkg> <new_pkg>");
                    std::process::exit(1);
                }
                current_engine = EngineTarget::Native;
                current_mode = OperationMode::Install;
            }

            // Help
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }

            // Target Package Names, URLs, Paths, or Native Passthrough Flags
            target => {
                if target.starts_with('-') {
                    pipeline.passthrough_args.push(target.to_string());
                } else {
                    match current_mode {
                        OperationMode::Install => match current_engine {
                            EngineTarget::Native => {
                                if is_manifest_target(target) {
                                    pipeline.github_install.push(target.to_string());
                                } else {
                                    pipeline.native_install.push(target.to_string());
                                }
                            }
                            EngineTarget::Flatpak => pipeline.flatpak_install.push(target.to_string()),
                            EngineTarget::Snap => pipeline.snap_install.push(target.to_string()),
                        },
                        OperationMode::Remove => match current_engine {
                            EngineTarget::Native => pipeline.native_remove.push(target.to_string()),
                            EngineTarget::Flatpak => pipeline.flatpak_remove.push(target.to_string()),
                            EngineTarget::Snap => pipeline.snap_remove.push(target.to_string()),
                        },
                        OperationMode::Search => {
                            pipeline.search_queries.push(target.to_string());
                        }
                    }
                }
            }
        }
    }

    execute_pipeline(pipeline)
}

// Config 
fn initialize_config() -> Result<()> { 
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|p| p.join("puyo"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine configuration directory."))?;

    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");

    if config_path.exists() {
        println!("{} Configuration file already exists at {:?}", "==>".yellow().bold(), config_path);
        return Ok(());
    }

    let default_content = r#"[hooks]
# Commands executed sequentially after a successful full system upgrade (-Syu)
post_upgrade = [
    "echo 'Puyo system upgrade completed successfully!'",
    # Example: send a desktop notification disguised as a specific app name
    # "notify-send -a 'Puyo' 'Upgrade Complete' 'All packages and runtimes are now up to date.'"
]
"#;

    std::fs::write(&config_path, default_content)?;
    println!("{} Created default configuration at {:?}", "==>".green().bold(), config_path);
    Ok(())
}

fn is_manifest_target(target: &str) -> bool {
    let path = Path::new(target);
    (path.exists() && path.is_dir())
        || (target.contains('/') && !target.ends_with(".deb") && !target.ends_with(".rpm")) // match any link or .deb file or .rpm
}

fn print_help() {
    println!(
        r#"puyo - Universal Meta-Wrapper & Transaction Manager for Arch Linux ({version}):

USAGE:
puyo [COMMANDS] [PACKAGES] [FLAGS]

COMMON COMMANDS:
<no arg>, update, -Syu  Full system update across all engines (yay, flatpak, snap)
install, -S             Mark target packages for installation
remove, -R              Mark target packages for removal
swap <pkgA> <pkgB>      Atomic 1-to-1 replacement (removes pkgA, installs pkgB while ignoring dependencies)
clean, -C               Sweep orphaned packages and prune backend caches
search, -s              Search package databases across enabled backends
init                    Initialize ~/.config/puyo/config.toml default configuration
Note that any standard pacman arguments should work.

ENGINE SWITCHES & ONE-SHOTS:
native, arch            Route subsequent commands to Yay/pacman
flat, flatpak           Route subsequent commands to Flatpak engine
snap                    Route subsequent commands to Snap engine
flatinstall/flatremove  One-shot Flatpak operation
snapinstall/snapremove  One-shot Snap operation

FLAGS:
-f, --force             Defer failures and execute isolated retries on broken targets
--dry-run               Print transaction plan and exit without modifying system
-h, --help              Print help information"#,
        version = env!("CARGO_PKG_VERSION")
    );
}