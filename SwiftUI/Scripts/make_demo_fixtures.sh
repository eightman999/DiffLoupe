#!/bin/bash
# Copyright (c) eightman 2005-2026
# Furin-lab All rights reserved.
# 動作設計: スクリーンショット用のデモ比較データ(Webサイト納品物の新旧2版)を生成する。
#           一致 / 相違 / 左のみ / 右のみ / サブフォルダ差分 が一通り現れるよう構成。
#
# 使い方: ./Scripts/make_demo_fixtures.sh [出力先]
#   既定: ~/Desktop/DiffLoupe_Demo  に A(v1) と B(v2) を生成
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ICON="$SCRIPT_DIR/../../legacy/cpp/resources/icons/icon.png"   # 同一画像素材として流用
OUT="${1:-$HOME/Desktop/DiffLoupe_Demo}"
A="$OUT/A"
B="$OUT/B"

rm -rf "$OUT"
mkdir -p "$A/css" "$A/js" "$A/images" "$B/css" "$B/js" "$B/images"

# ---------------------------------------------------------------------------
# index.html : 相違(タイトル変更・nav追加・本文変更/追加 → 横並び差分が映える)
# ---------------------------------------------------------------------------
cat > "$A/index.html" <<'HTML'
<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <title>ふりんカフェ</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <header>
    <h1>ふりんカフェ</h1>
    <nav>
      <a href="index.html">ホーム</a>
      <a href="about.html">お店について</a>
    </nav>
  </header>
  <main>
    <p>季節の豆をていねいに焙煎しています。</p>
  </main>
</body>
</html>
HTML

cat > "$B/index.html" <<'HTML'
<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <title>ふりんカフェ | 自家焙煎コーヒー</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <header>
    <h1>ふりんカフェ</h1>
    <nav>
      <a href="index.html">ホーム</a>
      <a href="about.html">お店について</a>
      <a href="contact.html">お問い合わせ</a>
    </nav>
  </header>
  <main>
    <p>季節の豆を、職人がていねいに焙煎しています。</p>
    <p>2026年春、テラス席がオープンしました。</p>
  </main>
</body>
</html>
HTML

# ---------------------------------------------------------------------------
# README.md : 相違(行追加が分かりやすい)
# ---------------------------------------------------------------------------
cat > "$A/README.md" <<'MD'
# ふりんカフェ Webサイト

自家焙煎コーヒー店のコーポレートサイトです。

## ページ構成
- ホーム
- お店について

## 制作メモ
- v1.0 初回納品
MD

cat > "$B/README.md" <<'MD'
# ふりんカフェ Webサイト

自家焙煎コーヒー店のコーポレートサイトです。

## ページ構成
- ホーム
- お店について
- お問い合わせ

## 制作メモ
- v1.0 初回納品
- v1.1 お問い合わせページ追加、テラス席を告知
MD

# ---------------------------------------------------------------------------
# style.css : 相違(色・余白の変更とルール追加)
# ---------------------------------------------------------------------------
cat > "$A/style.css" <<'CSS'
body {
  font-family: sans-serif;
  color: #333333;
  margin: 0;
}

header {
  background: #6f4e37;
  padding: 16px;
}

nav a {
  margin-right: 12px;
}
CSS

cat > "$B/style.css" <<'CSS'
body {
  font-family: "Hiragino Sans", sans-serif;
  color: #2b2b2b;
  margin: 0;
  line-height: 1.7;
}

header {
  background: #5a3e2b;
  padding: 24px;
}

nav a {
  margin-right: 16px;
  text-decoration: none;
}
CSS

# ---------------------------------------------------------------------------
# config.json : 相違(バージョン・日付)
# ---------------------------------------------------------------------------
cat > "$A/config.json" <<'JSON'
{
  "siteName": "furin-cafe",
  "version": "1.0.0",
  "updated": "2025-12-01",
  "analytics": true
}
JSON

cat > "$B/config.json" <<'JSON'
{
  "siteName": "furin-cafe",
  "version": "1.1.0",
  "updated": "2026-03-15",
  "analytics": false
}
JSON

# ---------------------------------------------------------------------------
# js/main.js : 相違
# ---------------------------------------------------------------------------
cat > "$A/js/main.js" <<'JS'
document.addEventListener("DOMContentLoaded", () => {
  console.log("furin cafe site loaded");
});
JS

cat > "$B/js/main.js" <<'JS'
document.addEventListener("DOMContentLoaded", () => {
  console.log("furin cafe site loaded");
  initMenuToggle();
});

function initMenuToggle() {
  const btn = document.querySelector(".menu-toggle");
  btn?.addEventListener("click", () => document.body.classList.toggle("open"));
}
JS

# ---------------------------------------------------------------------------
# 一致するファイル(両側同一内容) : about.html / css/reset.css
# ---------------------------------------------------------------------------
for ROOT in "$A" "$B"; do
  cat > "$ROOT/about.html" <<'HTML'
<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <title>お店について | ふりんカフェ</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <h1>お店について</h1>
  <p>2015年、住宅街の小さな一角ではじめました。</p>
</body>
</html>
HTML

  cat > "$ROOT/css/reset.css" <<'CSS'
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
img { max-width: 100%; height: auto; }
CSS
done

# ---------------------------------------------------------------------------
# 一致する画像(両側同一バイト) : images/logo.png, images/hero.jpg
# ---------------------------------------------------------------------------
if [ -f "$ICON" ]; then
  for ROOT in "$A" "$B"; do
    cp "$ICON" "$ROOT/images/logo.png"
    cp "$ICON" "$ROOT/images/hero.jpg"
  done
fi

# ---------------------------------------------------------------------------
# 左のみ(v1にあってv2で削除) : banner_2025.png, js/tracking.js
# ---------------------------------------------------------------------------
[ -f "$ICON" ] && cp "$ICON" "$A/banner_2025.png"
cat > "$A/js/tracking.js" <<'JS'
// 旧アクセス解析タグ(v2で削除)
(function () {
  window.dataLayer = window.dataLayer || [];
})();
JS

# ---------------------------------------------------------------------------
# 右のみ(v2で追加) : contact.html, CHANGELOG.md, js/darkmode.js
# ---------------------------------------------------------------------------
cat > "$B/contact.html" <<'HTML'
<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <title>お問い合わせ | ふりんカフェ</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <h1>お問い合わせ</h1>
  <form>
    <input type="email" placeholder="メールアドレス">
    <button type="submit">送信</button>
  </form>
</body>
</html>
HTML

cat > "$B/CHANGELOG.md" <<'MD'
# 変更履歴

## v1.1.0 (2026-03-15)
- お問い合わせページを追加
- ダークモード対応
- テラス席オープンの告知を追加
MD

cat > "$B/js/darkmode.js" <<'JS'
// v1.1 追加: ダークモード切替
const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");
prefersDark.addEventListener("change", (e) => {
  document.body.classList.toggle("dark", e.matches);
});
JS

echo "==> 生成完了: $OUT"
echo "    A (v1, 左): $(find "$A" -type f | wc -l | tr -d ' ') files"
echo "    B (v2, 右): $(find "$B" -type f | wc -l | tr -d ' ') files"
