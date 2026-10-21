#!/bin/zsh
# DiffLoupe の Rust コア(diffloupe-ffi)をビルドし、Swift 側が使う成果物を生成する。
#
# 生成物(SwiftUI/RustCore/ 以下、git 管理外):
#   libdiffloupe_ffi.a           aarch64 + x86_64 のユニバーサル static library
#   include/                     diffloupeFFI.h と module.modulemap(clang モジュール diffloupeFFI)
#   bindings/diffloupe.swift     UniFFI 生成の Swift バインディング(プロジェクトが直接コンパイル)
#   diffloupeFFI.xcframework     SwiftPM の binaryTarget 用(universal .a + headers)
#
# 使い方: SwiftUI/Scripts/build_rust_core.sh
#   Xcode ビルドではプロジェクトの Run Script フェーズから自動実行される。
#   SwiftPM(swift build/test)にはビルドフックが無いため、事前に手動実行が必要
#   (verify_cli.sh / build_cli.sh は先頭で本スクリプトを呼ぶ)。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SWIFTUI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CORE_DIR="$(cd "$SWIFTUI_DIR/../core" && pwd)"
OUT_DIR="$SWIFTUI_DIR/RustCore"

RUST_PROFILE="${RUST_PROFILE:-release-ffi}"
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

# x86_64 ターゲットが無ければ導入する(Apple Silicon 環境では未導入のことがある)
installed="$(rustup target list --installed)"
for t in "${TARGETS[@]}"; do
    case "$installed" in
        *"$t"*) ;;
        *) echo "==> rustup target add $t"; rustup target add "$t" ;;
    esac
done

for t in "${TARGETS[@]}"; do
    echo "==> cargo build --profile $RUST_PROFILE -p diffloupe-ffi ($t)"
    (cd "$CORE_DIR" && cargo build --profile "$RUST_PROFILE" -p diffloupe-ffi --lib --target "$t")
done

mkdir -p "$OUT_DIR/include" "$OUT_DIR/bindings"

# ユニバーサル static library
lipo -create \
    "$CORE_DIR/target/aarch64-apple-darwin/$RUST_PROFILE/libdiffloupe_ffi.a" \
    "$CORE_DIR/target/x86_64-apple-darwin/$RUST_PROFILE/libdiffloupe_ffi.a" \
    -output "$OUT_DIR/libdiffloupe_ffi.a"

# Swift バインディング生成(host arch の cdylib からメタデータを抽出)
(cd "$CORE_DIR" && cargo run --profile "$RUST_PROFILE" -p diffloupe-ffi --features bindgen --bin uniffi-bindgen -- \
    generate --library "target/aarch64-apple-darwin/$RUST_PROFILE/libdiffloupe_ffi.dylib" \
    --language swift --out-dir "$OUT_DIR/bindings")

# clang モジュール diffloupeFFI(header + modulemap)を include/ に配置
cp "$OUT_DIR/bindings/diffloupeFFI.h" "$OUT_DIR/include/diffloupeFFI.h"
cp "$OUT_DIR/bindings/diffloupeFFI.modulemap" "$OUT_DIR/include/module.modulemap"

# 生成された Swift バインディングを DiffLoupeCore ターゲットのソースツリーへ配置する。
# SwiftPM はターゲットパス外のソースをコンパイルできないため、コピーして参照する
# (生成物のため git 管理外;.gitignore で Engine/Generated を除外)。
mkdir -p "$SWIFTUI_DIR/DiffLoupe/Engine/Generated"
cp "$OUT_DIR/bindings/diffloupe.swift" "$SWIFTUI_DIR/DiffLoupe/Engine/Generated/diffloupe.swift"

# SwiftPM binaryTarget 用の xcframework(生成済みなら差し替え)
rm -rf "$OUT_DIR/diffloupeFFI.xcframework"
xcodebuild -create-xcframework \
    -library "$OUT_DIR/libdiffloupe_ffi.a" \
    -headers "$OUT_DIR/include" \
    -output "$OUT_DIR/diffloupeFFI.xcframework" >/dev/null

echo "==> done: $OUT_DIR"
lipo -archs "$OUT_DIR/libdiffloupe_ffi.a"
