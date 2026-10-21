#!/bin/bash
# =====================================================================
# DiffLoupe 検証ハーネス（AIエージェント向け）
#
# 使い方:
#   ./verify.sh --check   ツール存在確認と実行予定ステップの表示のみ（数秒）
#   ./verify.sh           高速検証: swift build + swift test（目安5分以内）
#   ./verify.sh --full    重い検証: 上記 + Scripts/verify_cli.sh + xcodebuild test
#
# 契約:
#   - 出力は "[n/m] ステップ名" 形式。最後に VERIFY PASS / VERIFY FAIL(exit 1)
#   - ツール欠如時: --check は WARN、無印/--full は SKIP を正直に表示
#   - リポジトリの状態は変更しない（ビルド成果物 SwiftUI/.build 等は既存設計通り）
#
# リポ構造（実確認済み）:
#   - Swift Package はリポ直下ではなく SwiftUI/Package.swift にある
#   - CLI検証スクリプトは SwiftUI/Scripts/verify_cli.sh（shebang: zsh）
#   - Xcode プロジェクトは SwiftUI/DiffLoupe.xcodeproj（共有scheme: DiffLoupe）
# コマンドの出典: SwiftUI/README.md
#   xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj build|test
#
# 互換性: bash 3.2 / BSD ユーティリティ（mapfile・grep -P は不使用）
# =====================================================================
set -euo pipefail
cd "$(cd "$(dirname "$0")" && pwd)"

MODE="fast"
case "${1:-}" in
  --check) MODE="check" ;;
  --full)  MODE="full" ;;
  "")      MODE="fast" ;;
  *)
    echo "不明な引数: ${1}（使用可能: --check / 無印 / --full）"
    exit 2
    ;;
esac

STEP=0
TOTAL=0
RUN_COUNT=0
SKIP_COUNT=0

step() { STEP=$((STEP + 1)); echo "[${STEP}/${TOTAL}] $1"; }
ok()   { RUN_COUNT=$((RUN_COUNT + 1)); echo "  OK: $1"; }
skip() { SKIP_COUNT=$((SKIP_COUNT + 1)); echo "  SKIP: $1"; }
fail() {
  echo "  NG: $1"
  echo "VERIFY FAIL"
  exit 1
}
have() { command -v "$1" >/dev/null 2>&1; }

finish() {
  echo "結果: 実行 ${RUN_COUNT} 件 / SKIP ${SKIP_COUNT} 件"
  echo "VERIFY PASS"
  exit 0
}

PKG_DIR="SwiftUI"
CLI_VERIFY="SwiftUI/Scripts/verify_cli.sh"

XCODEBUILD_OK=0
if have xcodebuild && xcodebuild -version >/dev/null 2>&1; then
  XCODEBUILD_OK=1
fi

if [ "$MODE" = "check" ]; then
  TOTAL=2
  step "ツール存在確認"
  if have swift; then
    echo "  ツール swift: 有（$(command -v swift)）"
  else
    echo "  ツール swift: WARN 見つからない（無印の swift build / swift test が実行不可）"
  fi
  if [ "$XCODEBUILD_OK" -eq 1 ]; then
    echo "  ツール xcodebuild: 有（$(xcodebuild -version | head -n 1)）"
  else
    echo "  ツール xcodebuild: WARN 使用できない（--full の xcodebuild test が実行不可）"
  fi
  if have zsh; then
    echo "  ツール zsh: 有（--full の Scripts/verify_cli.sh 実行に使用）"
  else
    echo "  ツール zsh: WARN 見つからない（verify_cli.sh の shebang が zsh のため --full で必要）"
  fi
  step "実行予定ステップ表示"
  echo "  無印 : [1/3] リポ構造確認（SwiftUI/Package.swift）"
  echo "         [2/3] swift build（SwiftUI/ で実行）"
  echo "         [3/3] swift test（SwiftUI/ で実行）"
  echo "  --full: 上記に加えて"
  echo "         [4/5] ${CLI_VERIFY}（CLI終了コード検証。内部で swift build -c release）"
  echo "         [5/5] xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj test（SwiftUI/ で実行）"
  finish
fi

if [ "$MODE" = "full" ]; then TOTAL=5; else TOTAL=3; fi

step "リポ構造確認（SwiftUI/Package.swift）"
HAS_PKG=0
if [ -f "${PKG_DIR}/Package.swift" ]; then
  HAS_PKG=1
  ok "${PKG_DIR}/Package.swift が存在する（Swift Package はサブディレクトリ ${PKG_DIR}/ にある）"
else
  skip "${PKG_DIR}/Package.swift が見つからないため SwiftPM 系ステップをスキップする"
fi

step "ビルド: swift build（${PKG_DIR}/）"
if [ "$HAS_PKG" -eq 0 ]; then
  skip "Package.swift が無い"
elif ! have swift; then
  skip "swift が見つからない"
else
  LOG=$(mktemp "${TMPDIR:-/tmp}/diffloupe_verify_build.XXXXXX")
  echo "  実行中（ログ: ${LOG}）..."
  if (cd "$PKG_DIR" && swift build) >"$LOG" 2>&1; then
    ok "swift build 成功"
  else
    echo "  --- ビルドログ末尾 ---"
    tail -n 40 "$LOG"
    fail "swift build が失敗した（全ログ: ${LOG}）"
  fi
fi

step "テスト: swift test（${PKG_DIR}/）"
if [ "$HAS_PKG" -eq 0 ]; then
  skip "Package.swift が無い"
elif ! have swift; then
  skip "swift が見つからない"
else
  LOG=$(mktemp "${TMPDIR:-/tmp}/diffloupe_verify_test.XXXXXX")
  echo "  実行中（ログ: ${LOG}）..."
  if (cd "$PKG_DIR" && swift test) >"$LOG" 2>&1; then
    # 件数の要約行を可能な範囲で表示する（フレームワーク差異があるため best-effort）
    grep -E "Executed [0-9]+ test|Test run with [0-9]+ test" "$LOG" | tail -n 2 || true
    ok "swift test 成功"
  else
    echo "  --- テストログ末尾 ---"
    tail -n 60 "$LOG"
    fail "swift test が失敗した（全ログ: ${LOG}）"
  fi
fi

if [ "$MODE" = "fast" ]; then
  finish
fi

# --- --full 専用ステップ ---------------------------------------------
step "CLI終了コード検証: ${CLI_VERIFY}"
if [ ! -f "$CLI_VERIFY" ]; then
  skip "${CLI_VERIFY} が見つからない"
elif ! have zsh; then
  skip "zsh が見つからない（verify_cli.sh の shebang が zsh）"
elif ! have swift; then
  skip "swift が見つからない（verify_cli.sh 内部で swift build -c release を実行するため）"
else
  # verify_cli.sh は自身で SwiftUI/ に cd し、TestFixtures を読み取り専用で使う。
  # 出力（OK: ... / ALL PASSED）は検証結果そのものなのでそのまま流す。
  if zsh "$CLI_VERIFY"; then
    ok "verify_cli.sh が ALL PASSED"
  else
    fail "verify_cli.sh が失敗した"
  fi
fi

step "テスト: xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj test（${PKG_DIR}/）"
if [ ! -d "${PKG_DIR}/DiffLoupe.xcodeproj" ]; then
  skip "${PKG_DIR}/DiffLoupe.xcodeproj が見つからない"
elif [ "$XCODEBUILD_OK" -eq 0 ]; then
  skip "xcodebuild が使用できない"
else
  LOG=$(mktemp "${TMPDIR:-/tmp}/diffloupe_verify_xctest.XXXXXX")
  echo "  実行中（ログ: ${LOG}）..."
  if (cd "$PKG_DIR" && xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj test) >"$LOG" 2>&1; then
    ok "xcodebuild test 成功"
  else
    echo "  --- テストログ末尾 ---"
    tail -n 60 "$LOG"
    fail "xcodebuild test が失敗した（全ログ: ${LOG}）"
  fi
fi

finish
