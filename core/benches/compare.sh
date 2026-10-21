#!/bin/bash
# DiffLoupe ベンチマーク: Swift CLI vs Rust CLI の wall time 比較
# 使い方: core/benches/compare.sh [反復回数]
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SWIFT_BIN="$ROOT/SwiftUI/.build/release/diffloupe"
RUST_BIN="$ROOT/core/target/release/diffloupe"
REPS="${1:-5}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ ! -x "$SWIFT_BIN" ] || [ ! -x "$RUST_BIN" ]; then
  echo "両バイナリを release ビルドしてください" >&2; exit 1
fi

bench() {
  local name="$1" bin="$2"; shift 2
  local best=999999
  for _ in $(seq "$REPS"); do
    local t0=$(python3 -c 'import time;print(time.time())')
    "$bin" "$@" >/dev/null 2>&1
    local t1=$(python3 -c 'import time;print(time.time())')
    local d=$(python3 -c "print(($t1-$t0)*1000)")
    best=$(python3 -c "print(min($best,$d))")
  done
  echo "$best"
}

echo "== ストレスフィクスチャ(10,000ファイル)生成 =="
"$ROOT/SwiftUI/Scripts/make_stress_fixtures.sh" "$WORK/stress" 10000 >/dev/null 2>&1
SL="$(find "$WORK/stress" -maxdepth 1 -type d | sed -n '2p')"
SR="$(find "$WORK/stress" -maxdepth 1 -type d | sed -n '3p')"
echo "left=$SL right=$SR"

echo "== 大きなテキスト行差分用(20000行超→比較はハッシュ経路) =="
mkdir -p "$WORK/big_l" "$WORK/big_r"
python3 -c "open('$WORK/big_l/big.txt','w').write(''.join(f'line {i} abc\n' for i in range(35000)))"
python3 -c "open('$WORK/big_r/big.txt','w').write(''.join(f'line {i} abd\n' for i in range(35000)))"

echo "== 大きなバイナリ(64MiB) =="
mkdir -p "$WORK/bin_l" "$WORK/bin_r"
dd if=/dev/urandom of="$WORK/bin_l/big.bin" bs=1m count=64 2>/dev/null
cp "$WORK/bin_l/big.bin" "$WORK/bin_r/big.bin"
python3 -c "
import os
with open('$WORK/bin_r/big.bin','r+b') as f:
    f.seek(32*1024*1024); f.write(b'XXXX')
"

echo "name, swift_ms, rust_ms, ratio"
S=$(bench "stress" "$SWIFT_BIN" -r -q "$SL" "$SR"); R=$(bench "stress" "$RUST_BIN" -r -q "$SL" "$SR")
python3 -c "print(f'stress-10k, {$S:.1f}, {$R:.1f}, {$R/$S:.3f}')"
S=$(bench "bigtext" "$SWIFT_BIN" -q "$WORK/big_l" "$WORK/big_r"); R=$(bench "bigtext" "$RUST_BIN" -q "$WORK/big_l" "$WORK/big_r")
python3 -c "print(f'big-text-diff, {$S:.1f}, {$R:.1f}, {$R/$S:.3f}')"
S=$(bench "bigbin" "$SWIFT_BIN" -q "$WORK/bin_l" "$WORK/bin_r"); R=$(bench "bigbin" "$RUST_BIN" -q "$WORK/bin_l" "$WORK/bin_r")
python3 -c "print(f'big-binary-hash, {$S:.1f}, {$R:.1f}, {$R/$S:.3f}')"
