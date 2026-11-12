#!/bin/sh
# Builds FDE and packs a release archive: dist/fde-VERSION-ARCH-linux.tar.gz
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="$(uname -m)"
name="fde-$version-$arch-linux"
cargo build --release --locked --workspace
out="dist/$name"
rm -rf "$out"
mkdir -p "$out/bin" "$out/share/fde/wallpapers" "$out/data" "$out/docs"
for p in fde fde-panel fde-lock fde-notify fde-shot fde-settings fde-music; do
    install -m 755 "target/release/$p" "$out/bin/$p"
done
strip "$out"/bin/* 2>/dev/null || true
find assets/wallpapers -maxdepth 1 -type f -exec install -m 644 {} "$out/share/fde/wallpapers/" \;
install -m 644 data/fde.desktop data/fde-portals.conf "$out/data/"
install -m 755 tools/install.sh tools/start-fde "$out/"
install -m 644 README.md LICENSE SECURITY.md "$out/"
install -m 644 docs/HARDWARE.md docs/fde.example.toml "$out/docs/"
tar -C dist -czf "dist/$name.tar.gz" "$name"
( cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256" )
echo "dist/$name.tar.gz"
