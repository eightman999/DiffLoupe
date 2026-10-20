#!/bin/bash
# DiffLoupe 差分テスト(Swift CLI vs Rust CLI)。
# 同一引数で両 CLI を実行し、JSON/CSV/Markdown/HTML レポート・人間向けサマリ・
# 終了コードを比較する。タイムスタンプと絶対パスは正規化する。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
SWIFT_BIN="$ROOT/SwiftUI/.build/release/diffloupe"
RUST_BIN="$ROOT/core/target/release/diffloupe"
FIX="$ROOT/core/tests/parity/fixtures"

if [ ! -x "$SWIFT_BIN" ]; then
  echo "Swift CLI がありません: cd SwiftUI && swift build -c release --product diffloupe" >&2
  exit 1
fi
if [ ! -x "$RUST_BIN" ]; then
  echo "Rust CLI がありません: cd core && cargo build --release" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

TOTAL=0
MATCH=0
FAILED_CASES=""

# 正規化: 生成日時・mtime・絶対パスを除去
norm() {
  sed -E \
    -e 's/"generatedAt" : "[^"]*"/"generatedAt" : "X"/g' \
    -e 's/"ModificationDate" : "[^"]*"/"ModificationDate" : "X"/g' \
    -e 's/日時: [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z/日時: X/g' \
    -e 's/- 日時: .*/- 日時: X/g' \
    -e "s|/private$WORK|WORK|g" \
    -e 's|\\/private\\/var|\\/var|g' \
    -e "s|$WORK|WORK|g" \
    -e "s|$ROOT|ROOT|g" \
    -e 's/\[[^]]*No such file[^]]*\]/[E]/g' \
    -e 's/  \[[^]]*\]$/  [E]/'
}

# run_case <名前> <比較対象(stdout|stderr|both)> -- <引数...> @@ <左> <右>
run_case() {
  local name="$1" mode="$2"; shift 2
  [ "$1" = "--" ] && shift
  local args=()
  while [ $# -gt 0 ] && [ "$1" != "@@" ]; do args+=("$1"); shift; done
  shift
  local L="$1" R="$2"
  TOTAL=$((TOTAL + 1))
  "$SWIFT_BIN" ${args[@]+"${args[@]}"} "$L" "$R" >"$WORK/sw_out" 2>"$WORK/sw_err"; local se=$?
  "$RUST_BIN" ${args[@]+"${args[@]}"} "$L" "$R" >"$WORK/rs_out" 2>"$WORK/rs_err"; local re=$?
  local ok=1
  [ "$se" != "$re" ] && ok=0
  if [ "$mode" != "stderr" ]; then
    norm <"$WORK/sw_out" >"$WORK/sw_out_n"; norm <"$WORK/rs_out" >"$WORK/rs_out_n"
    cmp -s "$WORK/sw_out_n" "$WORK/rs_out_n" || ok=0
  fi
  if [ "$mode" != "stdout" ]; then
    norm <"$WORK/sw_err" >"$WORK/sw_err_n"; norm <"$WORK/rs_out" >/dev/null; norm <"$WORK/rs_err" >"$WORK/rs_err_n"
    cmp -s "$WORK/sw_err_n" "$WORK/rs_err_n" || ok=0
  fi
  if [ "$ok" = 1 ]; then
    MATCH=$((MATCH + 1))
  else
    FAILED_CASES="$FAILED_CASES\n  $name (exit $se/$re)"
    [ "${VERBOSE:-0}" = 1 ] && { echo "FAIL: $name"; diff "$WORK/sw_out_n" "$WORK/rs_out_n" | head -10; diff "$WORK/sw_err_n" "$WORK/rs_err_n" | head -10; }
  fi
}

# run_file <名前> <形式> -- <左> <右> : -o ファイル出力の中身を比較
run_file() {
  local name="$1" fmt="$2"; shift 2
  [ "$1" = "--" ] && shift
  local L="$1" R="$2"
  TOTAL=$((TOTAL + 1))
  local sf="$WORK/sw_$name.$fmt" rf="$WORK/rs_$name.$fmt"
  "$SWIFT_BIN" -q -o "$sf" "$L" "$R" >"$WORK/sw_out" 2>"$WORK/sw_err"; local se=$?
  "$RUST_BIN" -q -o "$rf" "$L" "$R" >"$WORK/rs_out" 2>"$WORK/rs_err"; local re=$?
  local ok=1
  [ "$se" != "$re" ] && ok=0
  norm <"$sf" >"$WORK/a_n"; norm <"$rf" >"$WORK/b_n"
  cmp -s "$WORK/a_n" "$WORK/b_n" || ok=0
  sed "s|$sf|OUT|" "$WORK/sw_out" | norm >"$WORK/sw_out_n"
  sed "s|$rf|OUT|" "$WORK/rs_out" | norm >"$WORK/rs_out_n"
  cmp -s "$WORK/sw_out_n" "$WORK/rs_out_n" || ok=0
  if [ "$ok" = 1 ]; then MATCH=$((MATCH + 1)); else FAILED_CASES="$FAILED_CASES\n  $name"; fi
}

# exit_only <名前> <期待一致> -- <引数...>(両CLI共通、引数のみ)
exit_pair() {
  local name="$1"; shift
  TOTAL=$((TOTAL + 1))
  "$SWIFT_BIN" "$@" >"$WORK/sw_out" 2>"$WORK/sw_err"; local se=$?
  "$RUST_BIN" "$@" >"$WORK/rs_out" 2>"$WORK/rs_err"; local re=$?
  local ok=1
  [ "$se" != "$re" ] && ok=0
  norm <"$WORK/sw_out" >"$WORK/a"; norm <"$WORK/rs_out" >"$WORK/b"
  cmp -s "$WORK/a" "$WORK/b" || ok=0
  norm <"$WORK/sw_err" >"$WORK/a"; norm <"$WORK/rs_err" >"$WORK/b"
  cmp -s "$WORK/a" "$WORK/b" || ok=0
  if [ "$ok" = 1 ]; then MATCH=$((MATCH + 1)); else FAILED_CASES="$FAILED_CASES\n  $name"; fi
}

# ---------------------------------------------------------------------------
# 追加フィクスチャ生成(エンコード・改行・構造化)。fixtures/gen へ生成。
# ---------------------------------------------------------------------------
GEN="$FIX/gen"
mkdir -p "$GEN/l" "$GEN/r"
python3 - "$GEN" <<'PY'
import os, sys
g = sys.argv[1]
def w(p, data):
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, 'wb').write(data)
# UTF-16 LE/BE + BOM
w(f'{g}/l/utf16le.txt', b'\xff\xfe' + '日本語UTF16\n二行目'.encode('utf-16-le'))
w(f'{g}/r/utf16le.txt', b'\xff\xfe' + '日本語UTF16\n二行目'.encode('utf-16-le'))
w(f'{g}/l/utf16be.txt', b'\xfe\xff' + 'BE日本語'.encode('utf-16-be'))
w(f'{g}/r/utf16be.txt', b'\xfe\xff' + 'BE日本語'.encode('utf-16-be'))
w(f'{g}/l/utf32.txt', b'\x00\x00\xfe\xff' + 'UTF32'.encode('utf-32-be'))
w(f'{g}/r/utf32.txt', b'\x00\x00\xfe\xff' + 'UTF32'.encode('utf-32-be'))
# Shift_JIS / EUC-JP / ISO-2022-JP
w(f'{g}/l/sjis.txt', 'シフトJISの文書\nこんにちは'.encode('shift_jis'))
w(f'{g}/r/sjis.txt', 'シフトJISの文書\nこんにちは'.encode('shift_jis'))
w(f'{g}/l/sjis_d.txt', 'シフトJIS'.encode('shift_jis'))
w(f'{g}/r/sjis_d.txt', 'シフトJIS変更'.encode('shift_jis'))
w(f'{g}/l/euc.txt', 'EUC-JPテスト\n日本語'.encode('euc_jp'))
w(f'{g}/r/euc.txt', 'EUC-JPテスト\n日本語'.encode('euc_jp'))
w(f'{g}/l/iso2022.txt', 'ISO-2022-JPテスト'.encode('iso2022_jp'))
w(f'{g}/r/iso2022.txt', 'ISO-2022-JPテスト'.encode('iso2022_jp'))
# UTF-8 BOM
w(f'{g}/l/bom8.txt', b'\xef\xbb\xbf' + 'BOM付き'.encode())
w(f'{g}/r/bom8.txt', b'\xef\xbb\xbf' + 'BOM付き'.encode())
# 改行コード
w(f'{g}/l/crlf.txt', b'a\r\nb\r\nc\r\n')
w(f'{g}/r/crlf.txt', b'a\nb\nc\n')
w(f'{g}/l/cr.txt', b'x\ry\rz\r')
w(f'{g}/r/cr.txt', b'x\ny\nz\n')
w(f'{g}/l/ws.txt', b'x  y   z  \nfoo\t\n')
w(f'{g}/r/ws.txt', b'x y z\nfoo\n')
w(f'{g}/l/case.txt', b'Hello WORLD\nSecond LINE\n')
w(f'{g}/r/case.txt', b'hello world\nsecond line\n')
# 構造化
w(f'{g}/l/keys.json', b'{"b":2,"a":1}')
w(f'{g}/r/keys.json', b'{\n  "a" : 1,\n  "b" : 2\n}')
w(f'{g}/l/prec.json', b'{"a":0.30000000000000004}')
w(f'{g}/r/prec.json', b'{"a":0.30000000000000004}')
w(f'{g}/l/ints.json', b'{"i":1.0,"e":1e2,"neg":-0,"big":9007199254740993}')
w(f'{g}/r/ints.json', b'{"big":9007199254740993,"neg":-0,"e":1e2,"i":1.0}')
w(f'{g}/l/attr.xml', b'<a b="2" a="1"><c>x</c></a>')
w(f'{g}/r/attr.xml', b'<a a="1" b="2">\n  <c>x</c>\n</a>')
w(f'{g}/l/cmt.xml', b'<a><!-- note --><b value="v">text</b></a>')
w(f'{g}/r/cmt.xml', b'<a>\n  <!-- note -->\n  <b value="v">text</b>\n</a>')
w(f'{g}/l/quote.csv', b'a,"x,y",1\nb,"said ""hi""",2\n')
w(f'{g}/r/quote.csv', b'a,x,y,1\nb,"said ""hi""",2\n')
w(f'{g}/l/tab.tsv', b'a\tb\tc\n1\t2\t3\n')
w(f'{g}/r/tab.tsv', b'a\tb\tc\n1\t2\t3\n')
w(f'{g}/l/bomcsv.csv', b'\xef\xbb\xbfa,b\n"x,y",z\n')
w(f'{g}/r/bomcsv.csv', b'a,b\n"x,y",z\n')
w(f'{g}/l/bad.json', b'{bad json')
w(f'{g}/r/bad.json', b'{bad json')
# 片側のみ・バイナリ・隠しファイル
w(f'{g}/l/only_l.txt', b'left only')
w(f'{g}/r/only_r.txt', b'right only')
w(f'{g}/l/same.bin', bytes(range(256)) * 8)
w(f'{g}/r/same.bin', bytes(range(256)) * 8)
w(f'{g}/l/diff.bin', bytes(range(256)) * 8)
w(f'{g}/r/diff.bin', bytes(range(255, -1, -1)) * 8)
w(f'{g}/l/.hidden.txt', b'hidden')
w(f'{g}/r/.hidden.txt', b'hidden')
w(f'{g}/l/sub/deep.txt', b'AAA')
w(f'{g}/r/sub/deep.txt', b'BBB')
PY

# ---------------------------------------------------------------------------
# 小数秒 mtime フィクスチャ(完全決定的: ISO8601 表示丸め規則のピン留め)
# Foundation は「最近接ミリ秒(同値は偶数)→秒へ切り捨て」で表示するため、
# 境界の .4999/.5/.5001/.9994/.9995/.9999 を固定 mtime で生成する。
# ---------------------------------------------------------------------------
MT="$FIX/mtime"
mkdir -p "$MT/l/sub" "$MT/r/sub"
python3 - "$MT" <<'PY'
import os, sys
mt = sys.argv[1]
base = 1761000000  # 固定 epoch 秒(2025-10-20T22:40:00Z 付近)
cases = [("m000",0.0),("m499",0.4999),("m500",0.5),("m501",0.5001),
         ("m9994",0.9994),("m9995",0.9995),("m9999",0.9999)]
for name, frac in cases:
    for side in ("l","r"):
        p = f"{mt}/{side}/{name}.txt"
        open(p,"wb").write(b"mtime-probe")
        ns = base*1_000_000_000 + int(frac*1_000_000_000)
        os.utime(p, ns=(ns,ns))
# ディレクトリの表示日時も丸め規則を通るよう固定化する
for side in ("l","r"):
    ns = base*1_000_000_000 + 999_500_000
    os.utime(f"{mt}/{side}/sub", ns=(ns,ns))
PY

L_TEST="$ROOT/SwiftUI/TestFixtures/left"
R_TEST="$ROOT/SwiftUI/TestFixtures/right"
L_GEN="$GEN/l"; R_GEN="$GEN/r"

# デモ/ストレスフィクスチャ(スクリプト出力は一時ディレクトリへ)
"$ROOT/SwiftUI/Scripts/make_demo_fixtures.sh" "$WORK/demo" >/dev/null 2>&1 || true
"$ROOT/SwiftUI/Scripts/make_stress_fixtures.sh" "$WORK/stress" 500 >/dev/null 2>&1 || true

# === レポート形式(TestFixtures / gen fixtures / demo / stress) ===
for pair in "TEST:$L_TEST:$R_TEST" "GEN:$L_GEN:$R_GEN"; do
  name="${pair%%:*}"; rest="${pair#*:}"; LL="${rest%%:*}"; RR="${rest#*:}"
  for fmt in json csv md html; do
    run_case "${name}-${fmt}-r" both -- -r -q -f "$fmt" @@ "$LL" "$RR"
    run_case "${name}-${fmt}-nr" both -- -q -f "$fmt" @@ "$LL" "$RR"
  done
  run_case "${name}-summary" both -- -r @@ "$LL" "$RR"
  run_case "${name}-summary-nr" both -- @@ "$LL" "$RR"
done

if [ -d "$WORK/demo/A" ]; then
  run_case "demo-json" both -- -r -q -f json @@ "$WORK/demo/A" "$WORK/demo/B"
  run_case "demo-summary" both -- -r @@ "$WORK/demo/A" "$WORK/demo/B"
fi
if [ -d "$WORK/stress" ]; then
  for d in "$WORK"/stress/*/; do :; done
  SL="$(find "$WORK/stress" -maxdepth 1 -type d -name '*left*' -o -name 'A' -o -name 'left' | head -1)"
  SR="$(find "$WORK/stress" -maxdepth 1 -type d -name '*right*' -o -name 'B' -o -name 'right' | head -1)"
  if [ -n "$SL" ] && [ -n "$SR" ]; then
    run_case "stress-json" both -- -r -q -f json @@ "$SL" "$SR"
    run_case "stress-summary" both -- -r @@ "$SL" "$SR"
  fi
fi

# === 小数秒 mtime(日付表示丸めの決定性ケース) ===
for fmt in json csv md html; do
  run_case "mtime-${fmt}" both -- -r -q -f "$fmt" @@ "$MT/l" "$MT/r"
done

# === 内容比較モード ===
run_case "cc-line-endings" both -- -r -q --ignore-line-endings @@ "$L_GEN" "$R_GEN"
run_case "cc-trailing-ws" both -- -r -q --ignore-trailing-whitespace @@ "$L_GEN" "$R_GEN"
run_case "cc-normalize-ws" both -- -r -q --normalize-whitespace @@ "$L_GEN" "$R_GEN"
run_case "cc-ignore-case" both -- -r -q --ignore-case @@ "$L_GEN" "$R_GEN"
run_case "cc-all" both -- -r -q --ignore-line-endings --ignore-trailing-whitespace --normalize-whitespace --ignore-case @@ "$L_GEN" "$R_GEN"
run_case "cc-all-json" both -- -r -q --ignore-case --ignore-line-endings -f json @@ "$L_GEN" "$R_GEN"

# === 構造化比較 ===
for m in auto json xml csv off; do
  run_case "struct-$m" both -- -r -q --structured "$m" @@ "$L_GEN" "$R_GEN"
done
run_case "struct-json-report" both -- -r -q --structured json -f json @@ "$L_GEN" "$R_GEN"
run_case "struct-csv-report" both -- -r -q --structured csv -f csv @@ "$L_GEN" "$R_GEN"

# === インクルード/エクスクルード ===
run_case "inc-txt" both -- -r -q -i '*.txt' @@ "$L_GEN" "$R_GEN"
run_case "inc-nested" both -- -r -q -i 'sub/**' @@ "$L_GEN" "$R_GEN"
run_case "inc-qmark" both -- -r -q -i 'a?c*' @@ "$L_GEN" "$R_GEN"
run_case "exc-json" both -- -r -q -e '*.json' @@ "$L_GEN" "$R_GEN"
run_case "exc-dir" both -- -r -q -e 'sub' @@ "$L_GEN" "$R_GEN"
run_case "exc-substar" both -- -r -q -e 'sub/*' @@ "$L_GEN" "$R_GEN"
run_case "inc-exc" both -- -r -q -i '*.csv' -i '*.json' -e 'bad*' @@ "$L_GEN" "$R_GEN"
run_case "inc-json" both -- -r -q -i '*.csv' -i '*.json' -e 'bad*' -f json @@ "$L_GEN" "$R_GEN"
run_case "preset-sc" both -- -r -q --preset source-control @@ "$L_GEN" "$R_GEN"
run_case "preset-mac" both -- -r -q --preset macos-metadata @@ "$L_GEN" "$R_GEN"

# === リネーム検出 ===
mkdir -p "$WORK/rn_l/sub" "$WORK/rn_r/sub2"
printf 'same content' > "$WORK/rn_l/old.txt"
printf 'same content' > "$WORK/rn_r/new.txt"
printf 'nested' > "$WORK/rn_l/sub/deep.txt"
printf 'nested' > "$WORK/rn_r/sub2/deep.txt"
run_case "renames" both -- -r -q --detect-renames @@ "$WORK/rn_l" "$WORK/rn_r"
run_case "renames-json" both -- -r -q --detect-renames -f json @@ "$WORK/rn_l" "$WORK/rn_r"

# === verify_cli.sh と同等ケース ===
run_case "vc-diff" both -- -q @@ "$L_TEST" "$R_TEST"
run_case "vc-same" both -- -q @@ "$L_TEST" "$L_TEST"
exit_pair "vc-help" -- --help
exit_pair "vc-version" -- --version
exit_pair "vc-notdir" -- -q "$L_TEST/same.txt" "$R_TEST"
exit_pair "vc-nopathts" -- -q
exit_pair "vc-extraargs" -- -q "$L_TEST" "$R_TEST" "$R_TEST"
exit_pair "vc-badopt" -- --bogus-option "$L_TEST" "$R_TEST"
exit_pair "vc-badfmt" -- -q -f yaml "$L_TEST" "$R_TEST"
exit_pair "vc-badstruct" -- -q --structured yaml "$L_TEST" "$R_TEST"
exit_pair "vc-missing-val" -- -f
exit_pair "vc-ddash" -- -q -- "$L_TEST" "$R_TEST"
exit_pair "vc-norecursive" -- -r --no-recursive "$L_TEST" "$R_TEST"
exit_pair "vc-slash" -- /r /x "$L_TEST" "$R_TEST"
exit_pair "vc-slash-exit" -- /r /enableexitcode "$L_TEST" "$R_TEST"
exit_pair "vc-slash-label" -- /r /dl 左 /dr 右 "$L_TEST" "$R_TEST"
exit_pair "vc-slash-or" -- /r /or "$WORK/sw_or.html" "$L_TEST" "$R_TEST"

mkdir -p "$WORK/js-a" "$WORK/js-b"
printf '{"b":2,"a":1}' >"$WORK/js-a/data.json"
printf '{"a":1,"b":2}' >"$WORK/js-b/data.json"
run_case "vc-struct-off" both -- -q @@ "$WORK/js-a" "$WORK/js-b"
run_case "vc-struct-json" both -- -q --structured json @@ "$WORK/js-a" "$WORK/js-b"
run_case "vc-struct-auto" both -- -q --structured auto @@ "$WORK/js-a" "$WORK/js-b"
run_case "vc-normalize-json" both -- -q --normalize-json @@ "$WORK/js-a" "$WORK/js-b"
run_case "vc-normalize-xml" both -- -q --normalize-xml @@ "$WORK/js-a" "$WORK/js-b"

mkdir -p "$WORK/prec-a" "$WORK/prec-b"
printf '{"a":0.1234567}' >"$WORK/prec-a/data.json"
printf '{"a":0.1234568}' >"$WORK/prec-b/data.json"
run_case "vc-prec-strict" both -- -q @@ "$WORK/prec-a" "$WORK/prec-b"
run_case "vc-prec-json" both -- -q --structured json @@ "$WORK/prec-a" "$WORK/prec-b"

mkdir -p "$WORK/exc-a/.git" "$WORK/exc-b"
printf x >"$WORK/exc-a/.git/config"; printf x >"$WORK/exc-b/only.txt"
run_case "vc-default-excl" both -- -q @@ "$WORK/exc-a" "$WORK/exc-b"
run_case "vc-no-excl" both -- -q -x @@ "$WORK/exc-a" "$WORK/exc-b"

# 出力ファイル系
run_file "out-csv" csv -- "$L_GEN" "$R_GEN"
run_file "out-md" md -- "$L_TEST" "$R_TEST"
run_file "out-html" html -- "$L_GEN" "$R_GEN"
run_file "out-json" json -- "$L_GEN" "$R_GEN"

exit_pair "vc-overwrite" -- -q -f csv -o "$WORK/sw_or.html" "$L_TEST" "$R_TEST"
exit_pair "vc-noparent" -- -q -f csv -o "$WORK/no-such-dir/r.csv" "$L_TEST" "$R_TEST"
exit_pair "vc-badext" -- -q -o "$WORK/out.xyz" "$L_TEST" "$R_TEST"

echo "-----"
echo "差分テスト: $MATCH/$TOTAL 一致"
if [ "$MATCH" != "$TOTAL" ]; then
  echo -e "不一致ケース:$FAILED_CASES"
  exit 1
fi
echo "ALL PARITY CASES MATCH"
