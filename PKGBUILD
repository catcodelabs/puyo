# Maintainer: CatCodeLabs <voxythecat@gmail.com>
pkgname=puyo
pkgver=1.0.0
pkgrel=3
pkgdesc="Precariously Unified Yay Orchestrator - Universal Meta-Wrapper for Arch Linux"
arch=('x86_64')
url="https://github.com/catcodelabs/puyo"
license=('GPL-3.0-or-later')
depends=('pacman' 'git' 'libarchive' 'yay')
makedepends=('cargo')
source=()
sha256sums=()

build() {
    cargo build --release --locked --target-dir target
}

package() {
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
}