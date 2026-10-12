# Generated dynamically by puyo
pkgname="puyo"
pkgver=1.1.0
pkgrel=1
pkgdesc="Universal Meta-Wrapper & Transaction Manager for Arch Linux"
arch=('x86_64')
depends=('pacman' 'flatpak' 'snapd' 'glibc')
makedepends=('cargo' 'git')

build() {
    cargo build --release
}

package() {
    install -Dm755 "target/release/puyo" "$pkgdir/usr/bin/puyo"
}
