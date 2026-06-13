#!/bin/bash
# Copyright (c) eightman 2005-2026
# Furin-lab All rights reserved.
# 動作設計: 受け入れ確認用に1万ファイル規模の比較用フォルダペアを生成する
#
# 使い方: ./make_stress_fixtures.sh [出力先ディレクトリ] [ファイル数]
#   既定: /tmp/DiffLoupeStress に 10000 ファイル × 左右
#   左右で約1%を内容相違、約1%を片側のみにする
set -euo pipefail

BASE="${1:-/tmp/DiffLoupeStress}"
COUNT="${2:-10000}"

rm -rf "$BASE"
mkdir -p "$BASE/left" "$BASE/right"

echo "generating $COUNT files into $BASE ..."
for ((i = 0; i < COUNT; i++)); do
    sub="dir$((i % 100))"
    mkdir -p "$BASE/left/$sub" "$BASE/right/$sub"
    content="file $i: lorem ipsum dolor sit amet consectetur adipiscing elit"
    printf '%s\n' "$content" > "$BASE/left/$sub/file$i.txt"

    if ((i % 100 == 1)); then
        # 同サイズ・内容相違(ハッシュ比較経路)
        printf '%s\n' "${content%?}X" > "$BASE/right/$sub/file$i.txt"
    elif ((i % 100 == 2)); then
        : # 右に作らない(左のみ)
    else
        printf '%s\n' "$content" > "$BASE/right/$sub/file$i.txt"
    fi
done

echo "done: $(find "$BASE/left" -type f | wc -l | tr -d ' ') files (left)"
