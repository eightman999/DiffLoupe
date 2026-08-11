# WORK_LOG — DiffLoupe macOSネイティブ(Swift)版 v1

## フェーズ0: 仕様の確定(2026-06-12)

### やったこと
- C++版([src/core/comparator.cpp](../legacy/cpp/src/core/comparator.cpp) / comparator.h / diffresult.h)と
  Python版([Python/comparator.py](../legacy/Python/comparator.py))の比較ロジックを読解。
- 比較アルゴリズム仕様書 [SPEC.md](SPEC.md) を作成。
- ツールチェーン確認: Xcode 26.3 / Swift 6.2.4(Swift 5 言語モードでビルド予定)/ macOS 26.5。

### 判断とその理由
- **mtime を同一性判定に使わない(既存からの変更)**:
  既存両版は「サイズ+mtime 完全一致 → ハッシュ省略で一致」とするが、内容相違の見逃しリスクがある。
  v1 はプロンプト推奨の「サイズ比較 → 不一致なら相違確定、一致ならハッシュ比較」を採用(SPEC.md §2.3)。
- **隠しファイルを既定で比較対象に含める(既存からの変更)**:
  既存の `showHidden` トグル(既定オフ)に代えて、`.git` / `.DS_Store` / `node_modules` の
  既定除外パターン(トグル可)を採用。納品物突き合わせ用途では全ファイル把握が重要なため(SPEC.md §2.2)。
- **片側のみ・読込エラーのステータスを明示**:
  Python版の「片側のみ→一致」(B2)、両版の「両側読込不可→一致」(B3)は不具合と判断し、
  Swift版では `leftOnly` / `rightOnly` / `error` を明示する。
- **ディレクトリもエントリとして記録(既存からの拡張)**:
  既存はファイルのみ記録するため空ディレクトリが結果に現れない。2ペイン比較ツールとしては
  片側のみのディレクトリを表示すべきと判断。
- **C++版の隠しディレクトリ除外バグ(B1)は踏襲しない**: `QDirIterator` + `it.next()` の誤用で
  サブツリーが除外されない挙動は仕様ではなく不具合と判断(SPEC.md §1.4)。
- **配置場所**: 既存の `SwiftUI/` ディレクトリ(30行のプレースホルダースケルトン)を
  本実装で置き換える。リポジトリの `Python/` と並ぶ構成とする。

### 未解決事項
- 大文字小文字のみ異なるファイル名の扱い(現状: 別エントリ。case-insensitive な APFS では
  同一ディレクトリ内に共存しないため実害は限定的)。
- mtime 高速モードのオプション化(v2 検討)。

## フェーズ1: プロジェクト骨格と実装(2026-06-12)

### やったこと
- `DiffLoupe.xcodeproj`(pbxproj 手書き・共有スキーム `DiffLoupe`)を作成。
  アプリターゲット+ユニットテストターゲットの2構成。`xcodebuild -scheme DiffLoupe build` 成功を確認。
- エンジン実装: `FolderComparator`(走査+段階判定+TaskGroup並列ハッシュ+進捗+キャンセル)、
  `LineDiff`(CollectionDifference による行差分と横並び整列)、`TextFileLoader`(UTF-8/バイナリ判別)。
- GUI実装: フォルダ選択バー(NSOpenPanel)、分類ツリー(OutlineGroup)、横並び差分ビュー、
  「差分のみ表示」フィルタ、設定画面(除外トグル)、ステータスバー(進捗・サマリ)、
  `frameAutosaveName` によるウィンドウ記憶。
- アプリアイコンは既存 `resources/icons/icon.png`(256px)から `Scripts/make_app_icon.sh` で
  icns 化(ブランド統一のため新規描画はしない)。
- 旧 `SwiftUI/Package.swift` スケルトン(プレースホルダー30行)を削除し本実装で置換。

### 判断とその理由
- **テストターゲットは TEST_HOST を使わず、エンジン/モデルのソースを両ターゲットに含める方式**:
  GUIアプリの起動なしで高速にテストでき、pbxproj も単純になるため。
- **エンジンの言語モードは Swift 5**(コンパイラは 6.2)。Strict Concurrency 移行は v2 検討。
- **巨大ファイル保護を追加**(SPEC §2.6 に追記): 行数上限 40,000 行に加え、
  32MiB 超のファイルは差分表示しない。メモリ保護のための実装上の追加。

### テストで発見・修正した問題
1. **リンク切れシンボリックリンクの誤判定**: `URLResourceValues.fileSize` はリンク自体の
   サイズ(ターゲットパス文字列長)を返すため、左右でパス長が異なると `different` に
   なってしまっていた。リンク切れはサイズ -1 に統一し、ハッシュ段階の読込失敗で
   `error` になるよう修正。
2. **CRLF の行分割**: Swift の `String` では `"\r\n"` が1つの `Character` になるため、
   `split(separator: "\n")` では分割されない。先に `\r\n` → `\n` 正規化してから
   分割する方式に修正(LineDiff.splitLines)。

## フェーズ2: 受け入れ確認(2026-06-12)

### やったこと
- XCTest 19件すべて合格(`xcodebuild test`)。分類・除外トグル・空ディレクトリ・
  シンボリックリンク・キャンセル・進捗・行差分・バイナリ判別をカバー。
- `Scripts/make_stress_fixtures.sh` で1万ファイルペアを生成し、エンジン単体(release最適化)で
  **0.66秒**で完走(分類: 一致9800 / 相違100 / 左のみ100、期待どおり)。
- GUI実機確認: 実フォルダ(Hearts of Iron IV、**59,764ファイル**)の比較が完走し、
  走査・比較中もUI操作可能なことを確認。ツリーの集約表示・「差分のみ表示」・
  差分ビュー(変更行の黄ハイライト)・サマリ表示の動作を目視確認。
  スクリーンショットを `docs/screenshot.png` として README に掲載。

### 未解決事項
- 差分ビューの長い行の折返し表示は左右セルの高さが揃う一方、視覚的な行対応が
  把握しにくい場合がある(横スクロール表示への切替は v2 検討)。
- ウィンドウ最小幅 900pt は 13インチ画面では大きめかもしれない(要フィードバック)。

## フェーズ3: 比較エンジン拡張(内容比較モード・除外glob・リネーム検出)のUI接続と仕上げ(2026-08-11)

### やったこと
- エンジン拡張(前フェーズ実装)をUIへ接続:
  - `SettingsView` にユーザー除外glob(1行1パターン)・内容比較モード(改行コード/行末空白/
    連続空白/大文字小文字)・リネーム検出の設定を追加。既存の既定除外トグルとFree/Pro条件は維持。
  - 設定キーを `AppModel` に集約し、`AppModel.makeOptions(defaults:maxEntries:)` で
    `FolderComparator.Options` へ一括伝播。初期値は厳密比較・リネーム検出オフ・既定除外オン(後方互換)。
  - `DiffDetailView` が同じ `ContentComparison` を `LineDiff.rows` に渡す。
    再読み込みトリガーに `settingsSignature` を含め、設定変更後の再表示が反映される設計。
  - 詳細ビューのヘッダーに左右のサイズ・変更日時・POSIXパーミッションと、
    片側のみファイルのリネーム/移動候補(確定/候補・相手パス)を表示。
- エンジンのGLM指摘を修正:
  - `LineDiff` は差分計算のみ正規化済み行を使用し、`DiffRow.leftText/rightText` には
    元テキストを保持(行末CR含む)。ignoreCase等でも大文字・空白・改行差を確認できる。
    厳密モードは従来挙動を維持。
  - 正規化比較の `Data(contentsOf:)` 一括読込を `Options.maxNormalizationCompareBytes`
    (既定64MiB)で保護。超過ファイルはバイト厳密(ストリーミングハッシュ/サイズ)にフォールバック。
  - `detectRenames` の `try?` による CancellationError の飲み込みを修正し、即時再送出。
    `ComparisonProgress.renames(done:total:)` を追加し、AppModel/StatusBar のフェーズ表示に対応。
- テストを56件→68件へ追加(glob空パターン・Unicode・設定読み込みの純粋関数・正規化時の元表示保持・
  リネーム検出の進捗/キャンセル・巨大ファイル保護)。`xcodebuild test` 全件成功を確認。

### 判断とその理由
- **設定の純粋関数は `AppModel` に置き `nonisolated` 宣言**(テストから直接検証可能に)。
  テストターゲットはソース直接コンパイル方式のため、AppModel.swift をテストターゲットへ追加した。
- **巨大ファイルの正規化比較はバイト厳密フォールバックを採用**: 内容相違の見逃しを避けつつ
  OOM を防ぐ安全側の選択(サイズ不一致→相違、サイズ一致→ハッシュ判定)。
- **CRLFの行末CRは表示に残す**: Swiftの`"\r\n"`は1Characterになるため、文字走査で
  CRLF合字を分解して行末CRを保持する専用の `splitLinesKeepingCR` を新設(置換ベースは
  合字に戻るため不可)。表示専用で、差分計算は従来の CR 除去のまま。

### 未解決事項
- 設定変更の差分ビューへの即時反映は「次回比較・再選択から」。設定画面を開いたままの
  ライブ反映は未対応(SPEC §2.9 のとおり「次回の比較から反映」)。
- RELEASE_NOTES の全面改訂はフェーズ11で実施する。

## フェーズ4: テキスト詳細差分UX拡張(2026-08-11)

### やったこと
- `InlineDiff` を追加し、LCS動的計画法による文字単位の変更範囲ハイライトを実装。
  左右合計1,200文字の上限、`Character` 単位のUnicode安全性、nil/空文字の安全な扱いを追加。
- `DiffDisplay` を追加し、検索一致行抽出、連続5行以上のコンテキスト折りたたみ、
  差分ジャンプ対象の抽出、検索ヒットを含むグループの自動展開をUIから分離。
- `DiffDetailView` に前後差分ジャンプ、`⌘↑`/`⌘↓`、検索と大文字小文字切替、
  `⌘G`/`⌘⇧G`、折りたたみ/全行表示、長い行の水平スクロールを接続。
- GLM 5.2 の監査で発見した「初期状態のジャンプ無効」「検索ヒットの折りたたみ内隠蔽」を修正。
- `InlineDiffTests` / `DiffDisplayTests` を追加し、`xcodebuild test` **107件全件成功**を確認。

### 確認済み
- 行内差分の境界条件、Unicode・nil・長い行の上限保護。
- 検索、折りたたみ閾値5、複数グループの展開判定、未選択時ジャンプの純粋ロジック。
- Xcodeプロジェクト登録と既存機能を含む全107テスト。

### 未検証・残リスク
- macOS実機でのGUI見映え、`⌘↑`/`⌘↓`/`⌘G`のフォーカス時挙動、
  LazyVStack遠方スクロール、水平スクロール位置の目視確認は未実施。

## フェーズ5: 左右交換・Finder D&D・比較履歴・レポート出力(2026-08-11)

### やったこと
- `AppModel.swapSides()` と `⌘⇧S` で左右フォルダ・Git repository/refを一括交換し、結果・選択・一時スナップショットを安全に破棄する経路を追加。
- `FolderPicker.onDrop` と `AppModel.acceptDroppedFolder` でFinderからの単一フォルダD&Dを接続。複数項目・ファイル・存在しないパス・比較実行中は拒否し、既存のGit検出/ref初期化経路を共有。
- `RecentComparisonStore` を追加し、左右パス・Git ref・日時をUserDefaultsへ保存。最大10件、同一ペアの先頭移動、壊れたデータの安全な無視、存在チェック付き復元を実装。
- `ComparisonReport` とレポートCommandMenuを追加し、比較結果をHTML/CSV/JSON/Markdownで保存。UTC ISO8601、安定JSONキー、CSV/HTML/Markdownのescaping、上書き確認を実装。

### 検証
- `cd SwiftUI && xcodebuild -scheme DiffLoupe -project DiffLoupe.xcodeproj test -destination 'platform=macOS'` を実行し、**138テスト成功・0失敗**を確認。
- PRO分岐は、gitignore対象のProソースを一時コピーへ追加した検証環境でビルド成功を確認。リポジトリの通常Free構成は変更していない。

### 未検証・残リスク
- FinderからのD&D視覚フィードバック、`⌘⇧S`、NSSavePanel/NSAlertのGUI実機操作は未確認。
- PROフラグの有効化方式は既存の配布構成に依存し、通常Freeプロジェクト設定にはProソースを登録しない。

## フェーズ6: CLI(diffloupe)実装(2026-08-11)

### やったこと

- `SwiftUI/Package.swift`にmacOS 13+のSwiftPM構成を追加し、既存の比較エンジンを`DiffLoupeCore`としてCLIから再利用した。
- `DiffLoupeCLI`と薄い`CLI/main.swift`を追加し、引数解析、標準出力/標準エラー分離、HTML/CSV/JSON/Markdownレポート、
  レポートファイルの親ディレクトリ検証と上書き拒否、終了コード0/1/2/3、SIGINT/SIGTERMキャンセルを実装した。
- CI用の`Scripts/build_cli.sh`と`Scripts/verify_cli.sh`を追加した。

### 検証

- `swift build`成功。
- `swift test`は57件0失敗(後続のWinMerge互換テストを含む)。
- `Scripts/verify_cli.sh`は`ALL PASSED`。
- `xcodebuild test -project DiffLoupe.xcodeproj -scheme DiffLoupe -destination 'platform=macOS'`は138件0失敗。

## フェーズ7: WinMerge互換CLIの再帰制御・スラッシュスイッチ(2026-08-11)

### やったこと

- Agy/GeminiでWinMerge公式コマンドライン仕様を確認し、WinMergeの既定非再帰、`/r`再帰、`/r-`非再帰に合わせた。
- GUIアプリの既存再帰動作を保つため`FolderComparator.Options.recursive`の既定はtrueとし、CLIだけ既定falseを渡すようにした。
- `/r`、`/r-`、`/or`、`/enableexitcode`と、GUI専用の`/u`系、`/wl`系、`/minimize`系、`/dl`系を受理した。
  GUI専用スイッチはno-op、`/x`/`/xq`は進捗抑止へマップし、`/or`の未知拡張子はHTMLへフォールバックする。
- CLI統合テスト8件とCore再帰テスト5件を追加し、`verify_cli.sh`にもCLI運用チェックを維持した。
- GLM 5.2の読み取り専用監査で指摘されたデッドな再帰オプションと統合テスト不足を解消した。

### 判断と制限

- `/enableexitcode`は既存の詳細な終了コードを壊さず、指定時だけ0/1/2へ写像する。完全なWinMergeドロップイン互換ではない。
- `/dl`、`/dm`、`/dr`のラベルは引数互換のため受理するが、CLIの表示・レポートには反映しない。
- GUIの3-wayマージ、ウィンドウ状態、Finder/NSSavePanel操作はこのCLIフェーズでは実装対象外または未検証である。

### 検証

- 独立再検証で`swift build`成功、`swift test` **57件0失敗**、`Scripts/verify_cli.sh` **ALL PASSED**、
  `xcodebuild test` **138件0失敗**、`git diff --check`成功を確認した。

## フェーズ8: 画像比較ビューアーのズーム・パン・閾値操作(2026-08-11)

### やったこと

- `ImageComparisonView`に共有`ImageViewport`を追加し、左右/重ね合わせ/ブリンク/差分/スライスの5モードで
  ズームとパンを共有した。
- 0.25x〜4.0xのズーム、倍率スライダー、拡大/縮小、リセット、ドラッグパン、ピンチ操作を追加した。
- 異寸法画像では共通キャンバスを左右ペインとブリンクで使い、モード切替時にパン位置をリセットした。
- 閾値変更を400msデバウンスし、閾値スライダーの各ステップで画像比較を起動しないようにした。
- ツールバーを横スクロール可能にし、画像メタデータ警告をGrid下の行へ移動した。
- 画像エンジンに微小なRGB差分が閾値3では隠れ、閾値2では検出される境界テストを追加した。

### 判断と制限

- 表示操作では既存のCGImageを再利用し、閾値変更時だけ差分計算を再実行する方針とした。
- 画像寸法が異なる場合の表示は共通キャンバス内のアスペクト比維持とし、エンジンの比較判定ロジックは変更していない。
- UIのピンチ、ドラッグパン、スライス境界ドラッグ、狭幅ツールバーの横スクロールは実機目視未確認である。

### 検証

- 独立再検証で`xcodebuild test` **139件0失敗**、`swift build`成功、`swift test` **57件0失敗**、
  `git diff --check`成功を確認した。

## フェーズ9: 構造化比較・Hex比較・フィルタ・WinMerge互換CLI拡張(2026-08-11)

### やったこと

- `StructuredNormalizer`でJSON/XML/CSV/TSVを比較用に正規化。JSONキー順・XML属性順・表記上の空白を
  安定化し、配列順と浮動小数点の精度を保持する。8MiB上限、UTF-8 BOM除去、左右片側失敗時の
  両側rawフォールバックも実装した。
- `HexDiff`と`HexComparisonView`を追加し、バイナリを16バイト単位のHex/ASCII表示で比較する。
  8MiB超過時は未比較範囲を明示し、同一サイズの共通プレフィックスだけで一致と誤判定しないようにした。
- `FilterSet`でinclude/exclude globとソース管理・ビルド・Web/Node.js・macOSメタデータのプリセットを追加。
  NFC正規化、`*`/`?`/`**`、exclude優先をテストした。
- CLIに構造化比較の別名オプションと、既存のスラッシュスイッチ・終了コード・レポート出力を接続した。

### 判断と制限

- 構造化正規化はフォルダ分類とCLIに適用し、詳細ビューの表示原文は変更しない。3-wayも行ベースの
  `ContentComparison`だけを判定に使う。
- `.flt`ファイル、正規表現行フィルタ、WinMerge DLLプラグインは互換サブセットの対象外とした。

## フェーズ10: 自動監視・安全なコピー計画・3-way差分(2026-08-11)

### やったこと

- `DirectoryWatcher`を追加し、設定で有効化した作業ツリーだけをポーリング監視する。比較中の変更をpendingに
  集約し、連続自動再比較を3回で一時停止する。Git refの一時スナップショット側は監視しない。
- `SyncPlan`を純粋関数として追加し、左→右/右→左のcopyFile、createDirectory、conflict、保持、blockedを
  読み取り専用でプレビューする。実際のコピー・削除・上書き・権限変更は行わない。
- `ThreeWayDiff`とビューを追加し、base/left/rightの挿入・削除・置換を整列して、片側変更・同一変更・
  競合を表示する。40,000行上限、原文表示、ContentComparison判定を実装した。
- GLM 5.2の反例監査で検出された削除対変更の競合欠落を修正し、編集単位をbase行軸で再構成した。
  右→左の計画表示時に方向Pickerが左→右へ戻るUI不整合も修正した。

### 最終検証

- `swift test`: **187テスト成功・0失敗**。
- `xcodebuild test -project DiffLoupe.xcodeproj -scheme DiffLoupe -configuration Debug -destination 'platform=macOS'`:
  **212テスト成功・0失敗**。
- `Scripts/verify_cli.sh`: 終了コード19項目+レポート内容2項目、**ALL PASSED**。
- `xcodebuild build`: **BUILD SUCCEEDED**。`git diff --check`:問題なし。
- GLM 5.2の一時検証ではCE1/CE2の回帰例、ランダム2,000件、長め500件、ignoreCase 500件、
  行末空白500件を確認し、全て成功した。

### 未検証・残リスク

- WinMerge本体との同一入力による直接比較、Windows GUI、macOS GUIの実機目視操作は未実施。
- 3-wayのマージ適用・保存、フォルダ同期の実適用、ファイル編集は未実装。
- DirectoryWatcherはFSEventsではなくポーリングで、同一サイズ・同一mtimeの内容変化やsymlink先追跡は対象外。

## フェーズ11: リリース候補ドキュメント整備(2026-08-12)

### やったこと

- `README.md`と`SPEC.md`の現行機能・互換範囲・テスト件数を確認した。
- 旧1.1.0配布物の記録と、現行ワークツリーの追加実装を混同しないよう、
  `RELEASE_NOTES.md`に構造化比較、Hex、Git、CLI、監視、コピー計画、3-way差分の状態を追記した。
- 配布物をまだ再生成していないため、既存DMG/ZIPのチェックサムは変更していない。

### 最終確認

- `swift test`: 187テスト成功・0失敗。
- `xcodebuild test`: 212テスト成功・0失敗。
- `Scripts/verify_cli.sh`: `ALL PASSED`。
- `xcodebuild build`: `BUILD SUCCEEDED`。
- `git diff --check`: 問題なし。
- XcodeのSwift 5言語モードではビルド成功。`AppModel`の非同期処理にはSwift 6言語モードで
  エラー化する`Task.detached`内の弱参照警告が5件残るため、Swift 6移行時に別途整理する。
