#!/usr/bin/env bash
# release バイナリ + .desktop + アイコンの tarball を作る。
# コンテナ内(または libgtk-4-dev 済み Linux)で実行する。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="$REPO_ROOT/apps/linux/dist"
BIN="$REPO_ROOT/apps/linux/target/release/diffloupe-gtk"

if [ ! -x "$BIN" ]; then
    cargo build --release --manifest-path "$REPO_ROOT/apps/linux/Cargo.toml"
fi

VER=$(grep '^version' "$REPO_ROOT/apps/linux/Cargo.toml" | head -1 | cut -d'"' -f2)
PKG="diffloupe-gtk-${VER}-linux-x86_64"
STAGE="$OUT/$PKG"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/icons/hicolor/256x256/apps" \
    "$STAGE/share/icons/hicolor/128x128/apps" "$STAGE/share/icons/hicolor/64x64/apps" \
    "$STAGE/share/icons/hicolor/48x48/apps"

install -m755 "$BIN" "$STAGE/bin/diffloupe-gtk"
install -m644 "$REPO_ROOT/apps/linux/data/com.furin-lab.diffloupe.desktop" \
    "$STAGE/share/applications/"
install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-256.png" \
    "$STAGE/share/icons/hicolor/256x256/apps/diffloupe.png"
install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-128.png" \
    "$STAGE/share/icons/hicolor/128x128/apps/diffloupe.png"
install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-64.png" \
    "$STAGE/share/icons/hicolor/64x64/apps/diffloupe.png"
install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-48.png" \
    "$STAGE/share/icons/hicolor/48x48/apps/diffloupe.png"

mkdir -p "$OUT"
tar -C "$OUT" -czf "$OUT/$PKG.tar.gz" "$PKG"
ls -lh "$OUT/$PKG.tar.gz"
ls -lh "$BIN"

# AppImage(ツールがあれば生成、なければスキップ)
if command -v appimagetool >/dev/null 2>&1; then
    APPDIR="$OUT/AppDir"
    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
        "$APPDIR/usr/share/icons/hicolor/256x256/apps"
    install -m755 "$BIN" "$APPDIR/usr/bin/diffloupe-gtk"
    install -m644 "$REPO_ROOT/apps/linux/data/com.furin-lab.diffloupe.desktop" "$APPDIR/"
    install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-256.png" "$APPDIR/diffloupe.png"
    install -m644 "$REPO_ROOT/apps/linux/data/icons/diffloupe-256.png" \
        "$APPDIR/usr/share/icons/hicolor/256x256/apps/diffloupe.png"
    printf '#!/bin/sh\nexec "$APPDIR/usr/bin/diffloupe-gtk" "$@"\n' > "$APPDIR/AppRun"
    chmod +x "$APPDIR/AppRun"
    appimagetool "$APPDIR" "$OUT/$PKG.AppImage" && ls -lh "$OUT/$PKG.AppImage"
else
    echo "appimagetool not found; tarball only"
fi
