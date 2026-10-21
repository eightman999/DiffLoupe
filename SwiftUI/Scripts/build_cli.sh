#!/bin/zsh
# diffloupe CLI リリースビルド(CI向け)。swift build -c release を実行する。
set -euo pipefail
cd "$(dirname "$0")/.."

# CLIはRustコア(diffloupe-ffi)をリンクするため、先にユニバーサルライブラリを生成する
bash "$(dirname "$0")/build_rust_core.sh"

swift build -c release --product diffloupe
echo "diffloupe: $(pwd)/.build/release/diffloupe"
