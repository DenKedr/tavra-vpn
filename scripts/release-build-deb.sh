#!/usr/bin/env bash
set -euo pipefail
# Run on Linux after cargo build --release --locked -p tavra-control -p tavra-node.
component=${1:?control or node}; version=${2:-0.1.0}; arch=${3:-amd64}
case "$component" in control|node) ;; *) exit 2;; esac
case "$arch" in amd64|arm64) ;; *) exit 2;; esac
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.+~-][A-Za-z0-9.+~-]+)?$ ]] || exit 2
command -v dpkg-deb >/dev/null
case "$arch" in
 amd64) readelf -h "target/release/tavra-$component" | grep -q "Advanced Micro Devices X86-64" ;;
 arm64) readelf -h "target/release/tavra-$component" | grep -q "AArch64" ;;
esac
root=$(mktemp -d); trap 'rm -rf "$root"' EXIT
pkg=tavra-$component
install -d "$root/DEBIAN" "$root/opt/tavra/bin" "$root/etc/tavra" "$root/lib/systemd/system" "$root/usr/share/doc/$pkg"
install -m 755 "target/release/$pkg" "$root/opt/tavra/bin/$pkg"
install -m 644 "packaging/linux/$pkg.service" "$root/lib/systemd/system/"
install -m 644 "packaging/linux/$component.example" "$root/etc/tavra/$component.example"
install -m 644 docs/release-builds.md "$root/usr/share/doc/$pkg/README.md"
cat > "$root/DEBIAN/control" <<CONTROL
Package: $pkg
Version: $version
Section: net
Priority: optional
Architecture: $arch
Maintainer: TAVRA maintainers <maintainers@tavra.invalid>
Depends: libc6 (>= 2.35), libgcc-s1, adduser
Description: TAVRA $component development preview
 Configuration required. No working VPN tunnel is included in this preview.
CONTROL
sed "s/@COMPONENT@/$component/g" packaging/linux/postinst > "$root/DEBIAN/postinst"
sed "s/@COMPONENT@/$component/g" packaging/linux/prerm > "$root/DEBIAN/prerm"
cp packaging/linux/postrm "$root/DEBIAN/postrm"
chmod 755 "$root/DEBIAN/"{postinst,prerm,postrm}
(cd "$root"; find opt etc lib usr -type f | LC_ALL=C sort > "usr/share/doc/$pkg/owned-files.txt")
mkdir -p dist
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
dpkg-deb --root-owner-group --build "$root" "dist/${pkg}_${version}_${arch}.deb"
