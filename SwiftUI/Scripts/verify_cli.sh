#!/bin/zsh
# diffloupe CLI 検証スクリプト(CI向け)。リポジトリ同梱の TestFixtures で読み取り専用に動作する。
# 実データへの書き込みはせず、レポート出力は一時ディレクトリへ退避する。
set -euo pipefail
cd "$(dirname "$0")/.."

swift build -c release --product diffloupe
BIN="$(pwd)/.build/release/diffloupe"
LEFT="$(pwd)/TestFixtures/left"
RIGHT="$(pwd)/TestFixtures/right"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

# check <説明> <期待終了コード> -- <コマンド...>
check() {
  local desc="$1" want="$2"; shift 2
  [ "$1" = "--" ] && shift
  set +e
  "$@" >"$WORK/out.txt" 2>"$WORK/err.txt"
  local got=$?
  set -e
  if [ "$got" != "$want" ]; then
    echo "--- stderr ---" >&2; cat "$WORK/err.txt" >&2
    echo "--- stdout ---" >&2; cat "$WORK/out.txt" >&2
    fail "$desc: 期待する終了コード $want に対し $got を返しました"
  fi
  echo "OK: $desc (exit $got)"
}

# 1. 差異があるフィクスチャの比較 → 1
check "差異あり比較は終了コード1" 1 -- "$BIN" -q "$LEFT" "$RIGHT"
# 2. 同一ツリー同士の比較 → 0
check "同一ツリー比較は終了コード0" 0 -- "$BIN" -q "$LEFT" "$LEFT"
# 3. パスがディレクトリでない → 2
check "非ディレクトリ引数は終了コード2" 2 -- "$BIN" -q "$LEFT/same.txt" "$RIGHT"
# 4. ヘルプ → 0
check "ヘルプは終了コード0" 0 -- "$BIN" --help
# 5. バージョン → 0
check "バージョンは終了コード0" 0 -- "$BIN" --version

# 6. JSONレポートをstdoutへ(UTF-8文字列、終了コードは1)
set +e
"$BIN" -q -f json "$LEFT" "$RIGHT" >"$WORK/report.json" 2>"$WORK/err.txt"
got_json=$?
set -e
[ "$got_json" -eq 1 ] || fail "JSONレポート(stdout)の終了コードが $got_json でした"
grep -q '"format" : "DiffLoupeComparisonReport"' "$WORK/report.json" \
  || fail "JSONレポート(stdout)の内容が不正です"
echo "OK: JSONレポートをstdoutへ(UTF-8) (exit $got_json)"

# 7. CSVレポートをファイルへ(新規パス) → 1
check "CSVレポートのファイル出力は終了コード1" 1 -- \
  "$BIN" -q -f csv -o "$WORK/report.csv" "$LEFT" "$RIGHT"
[ -s "$WORK/report.csv" ] || fail "CSVレポートファイルが作成されていません"
grep -q "相対パス,状態" "$WORK/report.csv" || fail "CSVレポートの内容が不正です"
echo "OK: CSVレポートをファイルへ書き込み"

# 8. 既存ファイルへの上書きは拒否 → 2
check "既存ファイルへの上書きは終了コード2" 2 -- \
  "$BIN" -q -f csv -o "$WORK/report.csv" "$LEFT" "$RIGHT"

# 9. 親ディレクトリ欠如 → 2
check "親ディレクトリ欠如は終了コード2" 2 -- \
  "$BIN" -q -f csv -o "$WORK/no-such-dir/report.csv" "$LEFT" "$RIGHT"

# 10. 既定除外(.git)が有効であること。除外対象を一時ツリーに作って確認
mkdir -p "$WORK/exclude-a/.git" "$WORK/exclude-b"
printf x >"$WORK/exclude-a/.git/config"
printf x >"$WORK/exclude-b/only.txt"
check "既定除外(.git)は対象外(残差分は片側のみ)" 1 -- \
  "$BIN" -q "$WORK/exclude-a" "$WORK/exclude-b"

# 11. 構造化比較: JSONキー順の表記揺れ
mkdir -p "$WORK/js-a" "$WORK/js-b"
printf '{"b":2,"a":1}' >"$WORK/js-a/data.json"
printf '{"a":1,"b":2}' >"$WORK/js-b/data.json"
check "構造化なしではJSONキー順差を検出(1)" 1 -- "$BIN" -q "$WORK/js-a" "$WORK/js-b"
check "--structured json でJSONキー順差を正規化(0)" 0 -- \
  "$BIN" -q --structured json "$WORK/js-a" "$WORK/js-b"
check "--structured auto は拡張子で判定(0)" 0 -- \
  "$BIN" -q --structured auto "$WORK/js-a" "$WORK/js-b"

# 12. 構造化比較: エイリアスと無効値
check "--normalize-json エイリアス(0)" 0 -- \
  "$BIN" -q --normalize-json "$WORK/js-a" "$WORK/js-b"
check "未知の構造化モードは終了コード2" 2 -- \
  "$BIN" -q --structured yaml "$WORK/js-a" "$WORK/js-b"

# 13. 構造化比較: XML属性順の表記揺れ
mkdir -p "$WORK/xml-a" "$WORK/xml-b"
printf '<a b="2" a="1"/>' >"$WORK/xml-a/data.xml"
printf '<a a="1" b="2"/>' >"$WORK/xml-b/data.xml"
check "--structured xml でXML属性順差を正規化(0)" 0 -- \
  "$BIN" -q --structured xml "$WORK/xml-a" "$WORK/xml-b"

# 14. 構造化比較: JSON数値の精度(IEEE-754 の往復保持)。
#     %g既定6桁だと 0.1234567 と 0.1234568 が同一化して一致扱いになる境界値
mkdir -p "$WORK/prec-a" "$WORK/prec-b"
printf '{"a":0.1234567}' >"$WORK/prec-a/data.json"
printf '{"a":0.1234568}' >"$WORK/prec-b/data.json"
check "構造化なしでは精度差を検出(1)" 1 -- "$BIN" -q "$WORK/prec-a" "$WORK/prec-b"
check "--structured json でも近接doubleを区別(1)" 1 -- \
  "$BIN" -q --structured json "$WORK/prec-a" "$WORK/prec-b"

# 15. 構造化比較: UTF-8 BOM の除去(BOM有無で一致)
mkdir -p "$WORK/bom-a" "$WORK/bom-b"
printf 'a,b\n"x,y",z\n' >"$WORK/bom-b/data.csv"
printf '\xEF\xBB\xBFa,b\n"x,y",z\n' >"$WORK/bom-a/data.csv"
check "構造化なしではBOM差を検出(1)" 1 -- "$BIN" -q "$WORK/bom-a" "$WORK/bom-b"
check "--structured csv でBOM差を正規化(0)" 0 -- \
  "$BIN" -q --structured csv "$WORK/bom-a" "$WORK/bom-b"

echo "ALL PASSED"
