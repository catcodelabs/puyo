# puyo
> **P**uyo **U**nifies **Y**our **O**perations  
> *A universal meta-wrapper for Arch-based Linux Distros.*

<p align="center">
  <img src="https://img.shields.io/badge/license-GPLv3-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/language-Rust-orange.svg" alt="Rust">


`puyo` is a single wrapper designed to solve the fragmentation friction on Arch Linux. While traditional helpers like `yay` or `paru` handle official repositories and the AUR, `puyo` bridges the gap by routing official repos, custom binary mirrors (like `chaotic-aur`), the AUR, local `.deb` and `.rpm` files, and direct GitHub repositories through a single unified pipeline.

The cardinal rule of `puyo`: **`pacman` remains the single source of truth.** Everything `puyo` touches is packaged into a standard `.pkg.tar.zst` before installation, guaranteeing that every file on your system is cleanly tracked, upgradable, and easily removable.

---

## Key Features

* **Universal Source Ingestion:** Install software from official repos, the AUR, local Debian packages (`.deb`), local RPM packages (`.rpm`) or raw GitHub repos with one command.
* **Complete System Tracking:** No more untracked binaries in `/usr/bin` from `sudo make install`. Every source is packaged into a native Arch package.
* **Dual Syntax Support:** Use native `pacman` flags (`puyo -S`) or human-friendly verbs (`puyo install`).
* **Safe Non-Root Builds:** Compilation runs strictly as an unprivileged user, elevating to `sudo` only when handing off `.pkg.tar.zst` archives to `pacman`.
* **Declarative GitHub Builds (`.puyo`):** Automatically builds GitHub repos using `.puyo` manifest files.
* **Automated `.deb` and `.rpm` Conversion:** Extracts the package data from Debian and Fedora package files and registers them directly into the local `pacman` database by generating a PKGBUILD on the fly.
* **Smart Reinstall & Pre-flight Verification:** Automatically detects if a target is already installed to prompt for reinstallation, and cross-checks package availability (including multi-remote Flatpak search resolution) before executing pipelines.

---

## How It Works & Bucket-Based Execution

When you pass a list of targets and flags to `puyo`, it parses your input into an internal **Transaction Pipeline**. Targets are automatically categorized into isolated operational "buckets" based on their source type, operational verb, or active engine modifier.



```
                                  ┌─ Native/AUR Bucket ───> yay -S
                                  ├─ Local .deb Bucket ────> bsdtar engine + pacman -U
                                  ├─ Local .rpm Bucket ───> bsdtar engine + pacman -U
User Input (puyo)
                                  ├─ GitHub Bucket ───────> clone + .puyo manifest build
                                  ├─ Flatpak Bucket ──────> flatpak install
                                  └─ Snap Bucket ─────────> snap install
```

Instead of executing targets sequentially as they appear on the command line, `puyo` orders execution across these distinct buckets:

1. **System Upgrades**: Runs native updates (`yay -Syu`), Flatpak upgrades, and Snap refreshes.
2. **Removals**: Executes native `pacman -Rns`, Flatpak uninstalls, and Snap removals.
3. **GitHub Builds**: Clones repositories containing `.puyo` manifests and builds native packages.
4. **Native & Archive Installs**: Batches standard pacman/AUR packages together and converts `.deb`/`.rpm` archives into native `.pkg.tar.zst` packages via `bsdtar` before calling `pacman -U`.
5. **Atomic Swap**: Attempts to swap one package with another, by first uninstalling the previous one and then installing the new package.
6. **Flatpak & Snap Installs**: Batches remaining containerized app installations.
7. **Cleanup Sweep**: Prunes orphans and backend caches if `clean` / `-C` is passed.

If `--force` (`-f`) is active, any bucket that fails will defer its error, allowing remaining buckets to complete before retrying failed individual targets in the failed bucket in a granular second phase.

---

### Comprehensive Testing Command

To verify every bucket in a single multi-engine transaction (including backend switching, archive conversion, GitHub manifest builds, atomic swaps, and system cleanup), run:

```bash
puyo swap old-pkg new-pkg \
     native htop \
     ./sample.deb ./sample.rpm \
     catcodelabs/puyo \
     flat install org.gimp.GIMP \
     snap install code \
     clean \
     --force      --dry-run

```

or alternatively:

```bash
puyo swap old-pkg new-pkg htop ./sample.deb ./sample.rpm catcodelabs/puyo flatinstall org.gimp.GIMP snapinstall code clean -f --dry-run

```

This single command demonstrates `puyo`'s bucket sorting in action:

* **Removes** `old-pkg` and **installs** `new-pkg` as an atomic swap.
* Routes `htop` to the **Native/AUR bucket**.
* Passes `./sample.deb` and `./sample.rpm` through the **Archive Conversion bucket**.
* Clones and builds the **GitHub bucket** using the `.puyo` manifest.
* Switches engines to install GIMP via the **Flatpak bucket** and Code via the **Snap bucket**.
* Runs system orphan/cache cleanup at the end while deferring individual failures via `--force`.
* `--dry-run` ensures nothing actually happens, so you can find out what we're doing internally without actually modifying your system.

---

## Installation

### Prerequisites

Ensure basic build tools, Rust, Git, and `libarchive` are installed:

```bash
sudo pacman -S --needed base-devel rust git libarchive
```

If you don't have `yay` installed yet, set it up via the AUR:

```bash
git clone https://aur.archlinux.org/yay.git /tmp/yay
cd /tmp/yay
makepkg -si
```

`snap` and `flatpak` are technically optional, but highly recommended dependencies. Installation instructions can be found on their respective websites.

### Quick Install

Clone the repository and install using `makepkg`:

```bash
git clone https://github.com/catcodelabs/puyo
cd puyo
makepkg -si
```

---

## Usage

`puyo` supports traditional `pacman` flags, direct flag passthrough, and modern human-readable commands interchangeably.

### 1. Basic Package Management

Standard installation (Official repos & AUR):

```bash
puyo -S package
# or
puyo install package
# or simpy
puyo package
```

Full system update across all engines (native repos, AUR, Flatpak, Snap):

```bash
puyo -Syu
# or
puyo upgrade
# or simply:
puyo
```

Clean package removal:

```bash
puyo -R package
# or
puyo remove package
```

Package search across all active backends:

```bash
puyo -s query
# or
puyo search query
```

Direct passthrough (queries, status checks, local binary files, etc...):

```bash
puyo -Qs package
puyo -Si package
```

### 2. Multi-Backend Operations (Flatpak & Snap)

Route commands to specific engines on the fly:

```bash
# Engine switches
puyo flat install org.gimp.GIMP
puyo snap install code

# One-shot commands
puyo flatinstall org.gimp.GIMP
puyo snapremove code
```

An Engine Switch routes all following arguments to the respective engine, requiring another switch to change. A one-shot command passes only the next argument to the specified engine.

### 3. Atomic Swaps, Cleanup & Granular Mode

Replace package A with package B in a single command:

```bash
puyo swap old-package new-package
```

Sweep native orphans and prune backend package caches:

```bash
puyo clean
# or
puyo -C
```

Run transactions with error-deferral mode enabled:

```bash
puyo install pkg1 pkg2 pkg3 --force
# or
puyo install pkg1 pkg2 pkg3 -f
```

If a bucket fails to install, this flag causes `puyo` to retry each transaction in the failed bucket one-by-one to nail the cause of the fail, after completing all other buckets successfully.

### 4. Installing Local `.deb` and `.rpm` Files

Pass a path to any local `.deb` or `.rpm` package. `puyo` will invoke its `bsdtar`-based extraction engine, translate metadata, generate a native package, and install it via `pacman`:

```bash
puyo -S ./package.deb
puyo -S ./package.rpm
# or
puyo install ./package.deb
puyo install ./package.rpm
# or 
puyo ./package.deb
puyo ./package.rpm
```

### 5. Installing Directly from GitHub

`puyo` can clone, build, and package software directly from a GitHub repository, provided a `.puyo` file sits at the repository's root:

```bash
puyo [https://github.com/user/project](https://github.com/user/project)
# or
puyo user/project
# or in a local repo/directory
puyo .
```

---

## The `.puyo` Manifest (For Developers)

If you maintain a project on GitHub and want to make it trivially easy for Arch users to build and track your software, drop a `.puyo` file like this in your repository root:

```toml
[package]
name = "<package name>"
version = "<package version>"
pkgrel = <package release>
description = "<package description>"

[build]
cmd = "<package build command>"
binary = "<relative path to the output binary>"
binary_name = "<final binary name in PATH>"
depends = ["<package dependency 1>, <package dependency 2>"]
makedepends = ["<make dependency 1>, <make dependency 2>"]
```

When `puyo` detects this file during a GitHub build, it synthesizes a clean, standard `PKGBUILD` dynamically without requiring you to maintain Arch-specific packaging scripts.

---

## Architecture & Security

`puyo` strictly enforces Arch Linux security best practices:

1. **Never Run As Root:** `puyo` will refuse to execute if invoked directly with `sudo`, ensuring build operations stay safely unprivileged.
2. **Contextual Workspace Execution:** Local builds happen directly in-place, while remote GitHub repositories and package conversions (`.deb`/`.rpm`) are handled safely inside temporary staging directories.
3. **Optimized Privilege Handshake & Keep-Alive Daemon:** Sudo authentication is verified via an initial pre-flight prompt (`sudo -v`) right before executing system transactions. A background daemon thread then automatically pings `sudo -n true` every 60 seconds (protected by an RAII cleanup guard) to ensure long-running compilation or build steps do not fail because you missed a `sudo` password prompt.

---

## License
`puyo` is part of [CatCodeLabs](https://github.com/catcodelabs), and is therefore licensed under the GNU General Public License v3.0. See `LICENSE` for more information.
