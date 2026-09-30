# puyo

> **P**recariously **U**nified **Y**ay **O**rchestrator  
> *A universal meta-wrapper for Arch-based Linux Distros.*

`puyo` is a single wrapper designed to solve the fragmentation friction on Arch Linux. While traditional helpers like `yay` or `paru` handle official repositories and the AUR, `puyo` bridges the gap by routing official repos, custom binary mirrors (like `chaotic-aur`), the AUR, local `.deb` and `.rpm` files, and direct GitHub repositories through a single unified pipeline.

The cardinal rule of `puyo`: **`pacman` remains the single source of truth.** Everything `puyo` touches is packaged into a standard `.pkg.tar.zst` before installation, guaranteeing that every file on your system is cleanly tracked, upgradable, and easily removable.

---

## Key Features

* **Universal Source Ingestion:** Install software from official repos, the AUR, local Debian packages (`.deb`), or raw GitHub repos with one command.
* **Complete System Tracking:** No more untracked binaries in `/usr/bin` from `sudo make install`. Every source is packaged into a native Arch package.
* **Dual Syntax Support:** Use native `pacman` flags (`puyo -Syu`) or human-friendly verbs (`puyo install`, `puyo upgrade`).
* **Safe Non-Root Builds:** Enforces privilege separation - compilation runs strictly as an unprivileged user, elevating to `sudo` only when handing off `.pkg.tar.zst` archives to `pacman`.
* **Declarative GitHub Builds (`.puyo`):** Automatically builds GitHub repos using `.puyo` manifest files.
* **Automated `.deb` and `.rpm` Conversion:** Extracts the package data from Debian and Fedora package files and registers them directly into the local `pacman` database by generating a PKGBUILD on the fly.

---

## How It Works & Bucket-Based Execution

When you pass a list of targets and flags to `puyo`, it parses your input into an internal **Transaction Pipeline**. Targets are automatically categorized into isolated operational "buckets" based on their source type, operational verb, or active engine modifier.

```
                                  ┌─ Native/AUR Bucket ──> yay -S
                                  ├─ Local .deb Bucket ───> bsdtar engine + pacman -U
User Input (puyo) ───────────────>├─ Local .rpm Bucket ───> bsdtar engine + pacman -U
                                  ├─ GitHub Bucket ───────> clone + .puyo manifest build
                                  ├─ Flatpak Bucket ──────> flatpak install
                                  └─ Snap Bucket ─────────> snap install

```

Instead of executing targets sequentially as they appear on the command line, `puyo` orders execution across these distinct buckets:

1. **System Upgrades**: Runs native updates (`yay -Syu`), Flatpak upgrades, and Snap refreshes.
2. **Removals**: Executes native `pacman -Rns`, Flatpak uninstalls, and Snap removals.
3. **GitHub Builds**: Clones repositories containing `.puyo` manifests and builds native packages.
4. **Native & Archive Installs**: Batches standard pacman/AUR packages together and converts `.deb`/`.rpm` archives into native `.pkg.tar.zst` packages via `bsdtar` before calling `pacman -U`.
5. **Flatpak & Snap Installs**: Batches remaining containerized app installations.
6. **Cleanup Sweep**: Prunes orphans and backend caches if `clean` / `-C` is passed.

If `--who-cares` (`-f`) is active, any bucket that fails will defer its error, allowing remaining buckets to complete before retrying failed individual targets in a granular second phase.

---

### Comprehensive Testing Command

To verify every bucket in a single multi-engine transaction (including backend switching, archive conversion, GitHub manifest builds, atomic swaps, and system cleanup), run:

```bash
puyo swap old-pkg new-pkg \
     native htop \
     ./sample.deb ./sample.rpm \
     https://github.com/catcodelabs/puyo \
     flat install org.gimp.GIMP \
     snap install code \
     clean --who-cares \
     --dry-run

```
or if you're feeling crazy:
```bash
puyo swap old-pkg new-pkg native htop ./sample.deb ./sample.rpm https://github.com/catcodelabs/puyo flat install org.gimp.GIMP snap install code clean --who-cares --dry-run

```

This single command demonstrates `puyo`'s bucket sorting in action:

* **Removes** `old-pkg` and **installs** `new-pkg` as an atomic swap.
* Routes `htop` to the **Native/AUR bucket**.
* Passes `./sample.deb` and `./sample.rpm` through the **Archive Conversion bucket**.
* Clones and builds the **GitHub bucket** using the `.puyo` manifest.
* Switches engines to install GIMP via the **Flatpak bucket** and Code via the **Snap bucket**.
* Runs system orphan/cache cleanup at the end while deferring individual failures via `--who-cares`.
* `--dry-run` ensures nothing actually happens, so you can find out what we're doing internally without actually modifying your system

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

`snap` and `flatpak` are optional yet highly recommended dependencies. Installation instructions can be found on their respective websites.

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

```

Full system update across all engines (native repos, AUR, Flatpak, Snap):

```bash
puyo -Syu
# or
puyo upgrade

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

Direct passthrough (queries, status checks, local binary files):

```bash
puyo -Qs package
puyo -Si package

```

### 2. Multi-Backend Operations (Flatpak & Snap)

Route commands to specific engines on the fly:

```bash
# Engine switches
puyo flat install flathub org.gimp.GIMP
puyo snap install code

# One-shot commands
puyo flatinstall org.gimp.GIMP
puyo snapremove code

```

### 3. Atomic Swaps, Cleanup & Granular Mode

Replace package A with package B in a single transaction:

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
puyo install pkg1 pkg2 pkg3 --who-cares

```

### 4. Installing Local `.deb` and `.rpm` Files

Pass a path to any local `.deb` or `.rpm` package. `puyo` will invoke its `bsdtar`-based extraction engine, translate metadata, generate a native package, and install it via `pacman`:

```bash
puyo -S ./package.deb
puyo -S ./package.rpm
# or
puyo install ./package.deb
puyo install ./package.rpm

```

### 5. Installing Directly from GitHub

`puyo` can clone, build, and package software directly from a GitHub repository, provided a `.puyo` file sits at the repository's root:

```bash
puyo https://github.com/user/project

```

---

## The `.puyo` Manifest (For Developers)

If you maintain a project on GitHub and want to make it trivially easy for Arch users to build and track your software, drop a `.puyo` file in your repository root:

```toml
[package]
name = "mytool"
description = "A fast CLI utility"

[build]
makedepends = ["cargo", "git"]
depends = ["openssl", "glibc"]
cmd = "cargo build --release"
binary = "target/release/mytool"
```


When `puyo` detects this file during a GitHub build, it synthesizes a clean, standard `PKGBUILD` dynamically without requiring you to maintain Arch-specific packaging scripts.

---

## Architecture & Security

`puyo` strictly enforces Arch Linux security best practices:

1. **Never Run as Root:** `puyo` will refuse to execute if invoked directly with `sudo`.
2. **Isolated Workspaces:** All builds take place in `~/.cache/puyo/builds/` as an unprivileged user.
3. **Privilege Elevation Handoff:** Root access via `sudo` is requested strictly at the final step when passing compiled binary packages to `pacman -U`.

---

## License
`puyo` is part of [CatCodeLabs](https://github.com/catcodelabs), and is therefore licensed under the GNU General Public License v3.0. See `LICENSE` for more information.
