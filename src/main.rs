mod deb;
mod executor;
mod manifest;
mod rpm;

use anyhow::Result;
use executor::{enforce_non_root, execute_pipeline, TransactionPipeline};
use std::env;

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
            // Global flags
            "--who-cares" | "-f" => {
                pipeline.who_cares = true;
            }
            "--dry-run" => {
                pipeline.dry_run = true;
            }
            "clean" | "-C" => {
                pipeline.clean = true;
            }
            "--version" | "-V" => {
                println!("puyo {}", env!("CARGO_PKG_VERSION"));
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

            // Explicit 1-to-1 Swap Command
            "swap" => {
                let target_remove = iter.next();
                let target_install = iter.next();

                if let (Some(rem), Some(inst)) = (target_remove, target_install) {
                    pipeline.swaps.push((rem.clone(), inst.clone()));
                    pipeline.native_remove.push(rem);
                    pipeline.native_install.push(inst);
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
                    // Forward unknown flags directly to native yay/pacman (e.g. -Qs, -Si, -U, -Qdt)
                    pipeline.passthrough_args.push(target.to_string());
                } else {
                    match current_mode {
                        OperationMode::Install => match current_engine {
                            EngineTarget::Native => {
                                if is_github_target(target) {
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

fn is_github_target(target: &str) -> bool {
    target.starts_with("https://github.com/")
        || (target.contains('/') && !target.ends_with(".deb") && !target.ends_with(".rpm"))
}

fn print_help() {
    println!(
        r#"puyo - Universal Meta-Wrapper & Transaction Manager for Arch Linux

USAGE:
    puyo [MODIFIERS] [VERBS] [PACKAGES...]

COMMANDS & VERBS:
    puyo, -Syu                  Full system update across all engines (yay, flatpak, snap)
    install, -S                 Mark target packages for installation
    remove, -R                  Mark target packages for removal
    swap <pkgA> <pkgB>          Atomic 1-to-1 replacement (removes pkgA, installs pkgB)
    clean, -C                   Sweep orphaned packages and prune backend caches
    search, -s                  Search package databases across enabled backends

ENGINE SWITCHES & ONE-SHOTS:
    native, arch                Route subsequent targets to Yay
    flat, flatpak               Route subsequent targets to Flatpak engine
    snap                        Route subsequent targets to Snap engine
    flatinstall / flatremove    One-shot Flatpak operation
    snapinstall / snapremove    One-shot Snap operation

FLAGS:
    -f, --who-cares             Defer failures and execute isolated retries on broken targets
    --dry-run                   Print transaction plan and exit without modifying system
    -h, --help                  Print help information
"#
    );
}