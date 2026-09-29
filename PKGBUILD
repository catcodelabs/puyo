pkgname=puyo
pkgver=0.1.0
pkgrel=1
pkgdesc="Precariously Unified Yay Orchestrator - Universal Meta-Wrapper for Arch Linux"
arch=('x86_64')
url="https://github.com/catcodelabs/puyo"
license=('GPL-3.0-or-later')
depends=('pacman' 'git' 'libarchive' 'yay')
makedepends=('cargo')

build() {
    cargo build --release --locked --target-dir target
}

package() {
    install -Dm755 "target/release/puyo" "$pkgdir/usr/bin/puyo"
}