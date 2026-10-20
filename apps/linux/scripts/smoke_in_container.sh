#!/usr/bin/env bash
# コンテナ内で Xvfb 上のスモークテストを実行し、スクリーンショットを
# apps/linux/screenshots/ に保存する。check.sh から呼ばれる前提(パスは /work 基準)。
set -euo pipefail

cd /work
BIN="${CARGO_TARGET_DIR:-/work/apps/linux/target}/release/diffloupe-gtk"
if [ ! -x "$BIN" ]; then
    cargo build --release --manifest-path apps/linux/Cargo.toml
fi

mkdir -p /work/apps/linux/screenshots
export GDK_BACKEND=x11
export NO_AT_BRIDGE=1

xvfb-run -a -s "-screen 0 1400x900x24" \
    "$BIN" --smoke-test \
    /work/SwiftUI/TestFixtures/left /work/SwiftUI/TestFixtures/right \
    /work/apps/linux/screenshots

ls -la /work/apps/linux/screenshots/
