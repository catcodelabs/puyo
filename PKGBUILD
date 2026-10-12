# Generated dynamically by puyo
pkgname="puyo"
pkgver=1.1.1
pkgrel=3
pkgdesc="Universal Meta-Wrapper & Transaction Manager for Arch Linux"
arch=('x86_64')
depends=('pacman' 'yay' 'flatpak' 'snapd' 'glibc')
makedepends=('cargo' 'git')

build() {
    cargo build --release
}

package() {
    cargo install --path .. --root "$pkgdir/usr"
}
