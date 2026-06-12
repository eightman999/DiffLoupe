#!/bin/bash
# Copyright (c) eightman 2005-2026
# Furin-lab All rights reserved.
# 動作設計: 既存のresources/icons/icon.pngからmacOSアプリ用AppIcon.icnsを生成する
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_PNG="$SCRIPT_DIR/../../resources/icons/icon.png"
OUT_ICNS="$SCRIPT_DIR/../DiffLoupe/AppIcon.icns"
WORK_DIR="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$WORK_DIR"

# 元画像が256pxのため、256pxまでのサイズで構成する
sips -z 16 16     "$SRC_PNG" --out "$WORK_DIR/icon_16x16.png"      >/dev/null
sips -z 32 32     "$SRC_PNG" --out "$WORK_DIR/icon_16x16@2x.png"   >/dev/null
sips -z 32 32     "$SRC_PNG" --out "$WORK_DIR/icon_32x32.png"      >/dev/null
sips -z 64 64     "$SRC_PNG" --out "$WORK_DIR/icon_32x32@2x.png"   >/dev/null
sips -z 128 128   "$SRC_PNG" --out "$WORK_DIR/icon_128x128.png"    >/dev/null
sips -z 256 256   "$SRC_PNG" --out "$WORK_DIR/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "$SRC_PNG" --out "$WORK_DIR/icon_256x256.png"    >/dev/null

iconutil -c icns "$WORK_DIR" -o "$OUT_ICNS"
echo "generated: $OUT_ICNS"
