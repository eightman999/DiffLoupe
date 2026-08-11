#!/bin/zsh
# diffloupe CLI リリースビルド(CI向け)。swift build -c release を実行する。
set -euo pipefail
cd "$(dirname "$0")/.."

swift build -c release --product diffloupe
echo "diffloupe: $(pwd)/.build/release/diffloupe"
