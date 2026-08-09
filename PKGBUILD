pkgname=browserfetch
pkgver=0.1.0
pkgrel=1
pkgdesc='Fastfetch-style browser information tool for Linux desktops'
arch=('x86_64')
url='https://github.com/Mar2ianen/browserfetch'
license=('MIT')
depends=('chafa' 'xdg-utils')
makedepends=('cargo')
_commit='3e7cadcee2d62f2c68de4302fd3f8aab8c938219'
source=("$pkgname-$pkgver.tar.gz::https://github.com/Mar2ianen/browserfetch/archive/$_commit.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$srcdir/browserfetch-$_commit"
    cargo build --release --locked
}

package() {
    cd "$srcdir/browserfetch-$_commit"

    install -Dm755 target/release/browserfetch "$pkgdir/usr/bin/browserfetch"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 completions/_browserfetch \
        "$pkgdir/usr/share/zsh/site-functions/_browserfetch"
    install -Dm644 completions/browserfetch.bash \
        "$pkgdir/usr/share/bash-completion/completions/browserfetch"
    install -Dm644 completions/browserfetch.fish \
        "$pkgdir/usr/share/fish/vendor_completions.d/browserfetch.fish"
}
