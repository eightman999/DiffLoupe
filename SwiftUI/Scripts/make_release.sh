#!/bin/bash
# Copyright (c) eightman 2005-2026
# Furin-lab All rights reserved.
# 動作設計: DiffLoupe(Swift版)をReleaseビルドし、配布用の .dmg と .zip を dist/ に生成する
#
# 使い方: ./Scripts/make_release.sh
#   出力: SwiftUI/dist/DiffLoupe-<version>.dmg
#         SwiftUI/dist/DiffLoupe-<version>-macOS.zip
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJ_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJ_DIR"

DERIVED="build/DerivedData"
DIST="dist"
STAGING="$(mktemp -d)/DiffLoupe"

echo "==> Release ビルド中..."
xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj \
    -configuration Release -derivedDataPath "$DERIVED" build >/dev/null

APP="$DERIVED/Build/Products/Release/DiffLoupe.app"
[ -d "$APP" ] || { echo "エラー: $APP が見つかりません"; exit 1; }

VERSION="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")"
echo "==> バージョン: $VERSION"

# 配布時にGatekeeperの隔離属性が付かないよう、ローカルの拡張属性を除去
xattr -cr "$APP" 2>/dev/null || true

mkdir -p "$DIST" "$STAGING"
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"   # ドラッグでインストールできるように

# --- .dmg 生成 ---
DMG="$DIST/DiffLoupe-$VERSION.dmg"
rm -f "$DMG"
echo "==> $DMG を生成中..."
hdiutil create -volname "DiffLoupe $VERSION" \
    -srcfolder "$STAGING" -ov -format UDZO "$DMG" >/dev/null

# --- .zip 生成(.appのみ) ---
ZIP="$DIST/DiffLoupe-$VERSION-macOS.zip"
rm -f "$ZIP"
echo "==> $ZIP を生成中..."
( cd "$(dirname "$APP")" && ditto -c -k --keepParent "DiffLoupe.app" "$PROJ_DIR/$ZIP" )

rm -rf "$(dirname "$STAGING")"

echo ""
echo "==> 完了"
ls -lh "$DMG" "$ZIP" | awk '{print "    " $5 "\t" $9}'
echo ""
echo "    SHA-256:"
shasum -a 256 "$DMG" "$ZIP" | sed 's/^/    /'
