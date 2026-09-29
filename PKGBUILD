# Maintainer: CatCodeLabs <voxythecat@gmail.com>
pkgname=puyo
pkgver=0.1.0
pkgrel=1
pkgdesc="Precariously Unified Yay Orchestrator - Universal Meta-Wrapper for Arch Linux"
arch=('x86_64')
url="https://github.com/catcodelabs/puyo"
license=('GPL-3.0-or-later')
depends=('pacman' 'git' 'libarchive' 'yay')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$pkgname-$pkgver"
    cargo build --release --locked
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
}