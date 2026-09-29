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
* **Safe Non-Root Builds:** Enforces privilege separation—compilation runs strictly as an unprivileged user, elevating to `sudo` only when handing off `.pkg.tar.zst` archives to `pacman`.
* **Declarative GitHub Builds (`.puyo`):** Automatically builds GitHub repos using `.puyo` manifest files.
* **Automated `.deb` Conversion:** Converts Debian binaries via `debtap` and registers them directly into the local `pacman` database.

---

## How It Works




```
                                  ┌─ Official Repos ──> pacman
                                  ├─ Chaotic-AUR ─────> pacman
User Input (puyo) ───────────────>├─ AUR ─────────────> yay / makepkg
                                  ├─ Local .deb ──────> debtap + pacman -U
                                  └─ GitHub Repo ─────> .puyo / PKGBUILD generator
```

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

### Quick Install

Clone the repository and install using `makepkg`:

```bash
git clone https://github.com/catcodelabs/puyo
cd puyo
makepkg -si
```

---

## Usage

`puyo` supports both traditional `pacman` flags and modern human-readable commands interchangeably.

### 1. Basic Package Management


Standard installation (Official repos & AUR):
```bash
puyo -S package
# or
puyo install package
```


Full system update (Syncs repos, AUR, and tracked sources):
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


Package search
```bash
puyo -Ss package
# or
puyo search package
```

### 2. Installing Local `.deb` Files

Pass a path to any local `.deb` package. `puyo` will invoke `debtap`, translate dependencies, generate a `.pkg.tar.zst` archive, and install it via `pacman`:

```bash
puyo -S ./package.deb
# or
puyo deb install ./package.deb
```

### 3. Installing Directly from GitHub

`puyo` can clone, build, and package software directly from a GitHub repository, provided a `.puyo` file sits at the repository's root:

```bash
puyo -S https://github.com/user/project
# or
puyo git install user/project
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
puyo is part of CatCodeLabs, and is licensed under the GNU General Public License v3.0. See `LICENSE` for more information.
