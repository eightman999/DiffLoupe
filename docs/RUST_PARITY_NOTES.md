# RUST_PARITY_NOTES — Rust コアと Swift 実装の差異メモ

Swift 実装の観測可能な挙動を基準に移植し、仕様上の意図的差異・残る近似をここに記録する。

## 意図的な一致実装(実測で確認済み)

- **レポート JSON** は `JSONSerialization(.prettyPrinted|.sortedKeys)` とバイト一致するよう自前のシリアライザを使用。キー順は観測した照合順を固定キーで再現。
- **Double の文字列化**: Swift の `String(Double)` と同じ規則を再現。10進表記は |v| ∈ [1e-4, 2^53]、それ以外は `d.ddde±XX` 指数表記。構造化正規化の `renderJSON` は整数値 double の末尾 `.0` を除去(1.0→"1")。
- **巨大整数リテラル**(`i64`/`u64` を超える)は JSONSerialization の NSDecimalNumber 相当としてリテラル文字列をそのまま保持。
- **文字エンコード**: `String(data:encoding:.utf8)` は先頭 BOM(U+FEFF)を除去するため、`normalizedData` 経路でも先頭 BOM を除去。ただし `String(decoding:as:)` 相当の構造化入力(生データ)は BOM を残す(構造化正規化側は入口で BOM を別途除去)。
- **空白の定義**: `CharacterSet.whitespaces` はスペース(U+0020)とタブ(U+0009)のみ。`\r` は残る。`trimEnd`・CSV 未引用フィールドの trim・`split_whitespace` 相当はこの集合で実装。
- **CSV の `\r`**: Swift の parseCSV は `\r` を独立した行区切りとして扱う(`\r\n` は「\r の行区切り + 空行 + \n の行区切り」)。引用符・デリミタ検出・未引用 trim も同規則。
- **XML**: 入力の `\r\n`/`\r` は XML 仕様どおり事前に `\n` 正規化。属性値は「リテラル空白→スペース → entity 参照解決」の順。属性順はスカラー値ソート。複数ルート要素・非整形式・未終端要素はパース失敗でフォールバック。
- **`URL(fileURLWithPath:).path` 相当の正規化**: `fs::canonicalize` を使う(後述の差異あり)。
- **ファイル名照合・glob**: grapheme(拡張書記素クラスタ)単位、`?` は1クラスタ、`*`/`**`・区切り規則・NFC 等価を再現。
- **進捗・サマリ・CSV/MD/HTML レポート**のテキストは差分テストで全一致確認。
- **更新日時(ISO8601)の丸め規則**: Foundation `ISO8601DateFormatter` は
  「`Date`(秒小数)を最近接ミリ秒へ丸め(同値は偶数方向)→ 秒へ切り捨て(floor)」して出力する。
  実測: `x.4999`/`x.5`/`x.5001`/`x.9994` は当該秒、`x.9995` 以降は次秒。
  `-0.0005`→`-0ms`、` -1.0005`→`-1000ms` で偶数丸めを確認。
  Rust 側は `report::iso8601_date` で `round_ties_even(t*1000)/1000` を floor し、
  JSON/CSV/MD/HTML のエントリ日付すべてに適用。内部保持は epoch 秒 `f64`(秒切り捨てしない)。
  人間向けサマリはエントリ日付を出さないため非対象。ウォッチャー用スナップショットは
  `SystemTime` 保持で小数秒の変更検出を維持。

## SPEC.md との差異(実装基準で記録)

- CLI 既定は `recursive=false`(エンジン既定は true)。WinMerge `/r` 非指定に合わせた CLI 側の既定を優先。
- `.DS_Store`/`.git`/`node_modules` は「名前一致の既定除外」であって「ドット始まり=除外」ではない。`.hidden.txt` 等の隠しファイルは通常比較される。
- ディレクトリはツリー上にエントリとして現れるが、サマリ集計はファイルのみ。
- 差分計算は「メタデータ+SHA-256」の分類が主経路で、バイト逐次比較はフォールバック。

## 残る近似・既知の差異(差分テストで顕在化しなかった範囲)

- **`fs::canonicalize` は全シンボリックリンクを解決する**が、Foundation の `URL.path` は `/tmp`→`/private/tmp` を解決する一方で `/var/folders/...`(firmlink 相当)をそのまま返すケースがあった。絶対パス表示は差分テスト側の正規化で吸収しており、機能差異はない。
- **`localizedStandardCompare`**: ツリー内の表示順のみに使い、数字連の数値比較・大小文字の主比較・句読点類の後置を近似で実装。レポートの JSON/CSV/MD/HTML には影響しない。
- **JSONSerialization のエスケープ差異**: U+2028/U+2029 等のレアケースは未確認。
- **XML パーサ差異**: quick-xml と NSXMLParser の受理/拒否境界(DOCTYPE 内の内部実体宣言・非 UTF-8/宣言エンコーディング混在)は近似。実用上の XML では差が出にくいが、厳密等価ではない。
- **テキストエンコード検出**: Shift_JIS/EUC-JP は「両方解釈できる曖昧な入力」で検出優先順が ICU 準拠と微妙に違う可能性がある(Shift_JIS を先に試行)。標準的な日本語テキストでは一致。
- **grapheme 分割**: UAX#29 の完全実装ではなく、結合文字・ZWJ・絵文字修飾・国旗 RI ペアをカバーする近似。
- **CLI の `isDirectory` 判定**: ファイルシステム上の属性で判定するため、URL ベース判定と異なる稀なケース(壊れた特殊ファイル等)があり得る。
- **シグナル**: SIGINT/SIGTERM は 25ms ポーリングでキャンセルトークンへ反映。Swift の Task キャンセルとのタイミング差はあるが観測結果は同等。
- **`sha2` の asm フィーチャ**: MSVC/musl 向けは cc-rs のツールチェーン依存でホストにツールが要るため、当該ターゲットでは無効化(機能差異なし・SHA-256 実装は等価)。

## 差分テストの範囲

`core/tests/parity/run.sh` は TestFixtures/生成 fixtures/demo/stress について、
JSON/CSV/MD/HTML レポート・人間向けサマリ・進捗(stderr)・終了コードを
verify_cli.sh と同等のケース + 内容比較モード + include/exclude で比較する
(タイムスタンプと絶対パスは正規化)。
mtime フィクスチャ(`.0`/`.4999`/`.5`/`.5001`/`.9999` 秒の utimensat 設定)を追加し、
小数秒の丸め差によるフレークを回帰テスト化した(86ケース)。

## 非決定性の対策(監査済み)

- 並列ハッシュの完了順: ワーカー結果を元インデックスへ格納(indexed 集約)。
- パス走査: `BTreeSet` で左右パスを結合、エントリは `relative_path` で最終ソート。
- リネーム候補: `BTreeMap` + 明示ソート済みベクタで決定的。
- ツリー: `BTreeMap` 構成、子は「ディレクトリ優先 → localized 近似比較」で決定的。
- `watcher` スナップショット: エントリを `relative_path` でソート(FS 走査順に依存しない)。
- `generated_at`: run.sh で正規化。
- テスト用一時ディレクトリ名: PID + アトミックカウンタ(時刻由来の衝突を排除)。

## ユニットテストの 1:1 移植

Swift 非 UI テスト 371 件のうち 362 件を個別 `#[test]` として移植(対象外 9 件は
UserDefaults/AppModel 依存)。対応表と件数は `core/tests/parity/TEST_MAP.md`。
追加で ISO8601 丸め規則のユニットテスト 1 件を `comparison_report_tests.rs` に配置。

## 追加依存: uniffi(diffloupe-ffi 限定)
macOS SwiftUI アプリから Rust コアを呼ぶために `uniffi 0.32` を使う(Swift バインディングの自動生成と型安全な FFI のため。手書き C ABI より保守が軽い)。バインディング生成器は `bindgen` フィーチャに分離し、配布 static library には含めない。詳細は `docs/RUST_MAC.md`。

## Linux GUI(apps/linux)で追加した依存クレート

`diffloupe-gtk` 本体(core には依存を追加していない)。

| クレート | 理由 |
|---|---|
| `gtk4` (gtk4-rs 0.9、gdk-pixbuf/cairo/gsk/gio/glib を包含) | GTK4 ネイティブ UI バインディング。必須依存。 |
| `cairo-rs` (png フィーチャのみ) | スモークテスト用画像生成と PNG 出力。cairo 本体は gtk4 経由で既にリンクされるため、フィーチャ追加でバインディングだけ有効化。 |
| `serde` / `serde_json` | 比較履歴(`history.json`)のシリアライズ。core も利用しているため新規リンクは最小。 |

`apps/edition`(diffloupe-edition)は依存ゼロのツールキット非依存クレート。

## Linux GUI の意図的な差異(機能差異は docs/RUST_LINUX.md)

- ウィンドウ位置の永続化は X11 のみ(Wayland は位置取得不可)。サイズ/最大化は両方で復元。
- 設定/履歴の保存先は `$XDG_CONFIG_HOME/diffloupe/`(macOS の UserDefaults 相当の OS ネイティブな場所)。キー名は Swift 版と同一。
- 画像の EXIF 回転・HEIC は gdk-pixbuf ローダー依存(Swift 版は CIImage で常時回転適用)。
- コンテキストメニューの「開く/フォルダを表示」は `gtk4::FileLauncher` に委譲。
- `--compare-time` 計測モードはエンジン素の速度比較用にエントリ上限を外す(GUI 通常経路は Free の 2000 件上限が効く)。

## core への追加 API(GUI 用、振る舞い変更なし)

- `diffloupe-core/src/lib.rs`: `mod unicode` → `pub mod unicode`(1行)。GTK 側がインライン差分の grapheme インデックスをバイト/文字オフセットへ変換するために、core と同一の grapheme 分割規則を再利用するため公開。
- clippy 緑化のみの機械的修正(振る舞い不変): `hex_diff.rs` の冗長な `.max(0)` 除去、`text_loader.rs` の `chunks_exact` → `as_chunks` 置換、`diffloupe-cli/main.rs` の関数ポインタキャスト形式変更(新 Rust の `function_casts_as_integer` リント対応)。

## Windows GUI(apps/windows, diffloupe-win)に関する追記

### 追加依存

- `windows` 0.62(Windows ターゲットのみ): Win32 API バインディング。
  TreeView/ListView/カスタム描画/共通ダイアログ(IFileOpenDialog)/WIC 画像デコード/
  PrintWindow キャプチャに必要な最小限のフィーチャのみ有効化。
  `windows` クレート選定理由: 公式の薄いバインディングで中間ランタイムを持たず、
  exe 単体・軽量という要件に最も適合するため。
- `serde`/`serde_json`: 設定(`%APPDATA%\DiffLoupe\settings.json`)と履歴の永続化。
  core の許可済みクレートを再利用(新規依存ではない)。
- `diffloupe-edition`(`apps/edition`): SwiftUI の `DiffLoupeEdition` プロトコル相当の
  UI 非依存トレイト + `FreeEdition`。Pro 差し替えの拡張点のみを提供。

画像デコードは WIC(OS 標準)を使い、`image` クレートは入れない方針を維持した。

### core への追加 API

- `inline_diff::grapheme_ranges_to_utf16(text, ranges) -> Vec<Range<usize>>`:
  `InlineHighlight` の書記素インデックス範囲を Win32 描画用の UTF-16 範囲に変換。
  追加のみで既存動作は変更しない。
- `git_snapshot.rs`: `GIT_EXECUTABLE`/`TAR_EXECUTABLE` を `#[cfg]` で切替。
  Windows では `git`/`tar` を PATH 検索に委ね、`detect()` の実行ファイル
  存在確認をスキップする。非 Windows の動作は不変更。

### Windows 版の意図的な差異・実装方針

- 結果一覧は ListView(TreeView 相当の階層は行インデント+折りたたみで表現)、
  テキスト差分はインライン強調を行うため Win32 標準コントロールでは表現できず
  カスタム描画(grid.rs)とした。
- ドラッグ&ドロップは WM_DROPFILES(フォルダのみ受理)。
- ダークモードは DwmGetWindowAttribute による判定で配色を切替える範囲に留める。
- 画像比較・コピー計画・三方差分・git ref 比較・レポート出力は core API をそのまま使用し、
  UI 側にロジックを複製しない。
- Free 版の 2,000 エントリ上限は core の `CompareOptions::FREE_MAX_ENTRIES` を
  `FreeEdition::max_folder_entries` 経由で適用し、上限到達時は日本語アラートを出す。

## Windows での git スナップショットの改行

`GitRepository::materialize_snapshot`(`git archive <ref> | tar -x`)の内容は「その ref をチェックアウトしたときと同じ」とする。
Windows の git は既定で `core.autocrlf=true` のため、`git archive` の出力もチェックアウトと同じ変換を受け、テキストが CRLF になる
(macOS/Linux の Swift 版・Rust 版では LF)。core はこれを正規化しない(ユーザーの git 設定に従うのが自然な振る舞いのため)。
テスト(`git_snapshot_tests.rs`)側は、Windows のときだけ CRLF を LF に揃えてから内容を比較する。詳細は `core/tests/parity/TEST_MAP.md`。
