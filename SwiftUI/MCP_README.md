# AI向けの読み取り専用MCP

`diffloupe-mcp`は、GUIと同じSwift比較エンジンをAIから使うローカルstdioサーバーです。
全文をAIへ送る前に、ローカルで比較して「件数 → 差分のあるパス → 必要な変更行」の順に取得できます。
外部API、APIキー、追加ライブラリ、ネットワーク待受は不要です。

## ビルド・登録

```sh
cd /Users/eightman/dev/apps/DiffLoupe/SwiftUI
swift build -c release --product diffloupe-mcp
```

stdio MCP対応クライアントの設定例です。`command`と`--root`は利用環境に合わせた絶対パスに置き換えてください。
設定の保存先・登録操作はクライアントごとに異なります。この作業で既存クライアント設定は変更しません。

```json
{
  "mcpServers": {
    "diffloupe": {
      "command": "/Users/eightman/dev/apps/DiffLoupe/SwiftUI/.build/release/diffloupe-mcp",
      "args": ["--root", "/absolute/path/to/workspace"]
    }
  }
}
```

複数フォルダを許可する場合は`--root /path`を繰り返します。起動時に既存のディレクトリを1つ以上指定してください。
ルートを指定しない起動はエラーです。GUIの起動は不要です。

## ツールと使い方

| ツール | 引数 | 返却内容 |
|---|---|---|
| `compare_folders` | `left`, `right`: フォルダの絶対パス | ファイルの状態別件数、差分のあるパスと状態。本文は返さない |
| `diff_files` | `left`, `right`: ファイルの絶対パス | 変更行と前後の一致行、左右の行番号 |

両方に`offset`（既定0、最大1,000,000）、`limit`（既定40、1〜100）が使えます。
`diff_files`は`context`（既定2、0〜5）、`maxTextCharacters`（片側1行の上限、既定240、40〜1000）も指定できます。
省略した行本文は`leftTruncated`/`rightTruncated`で明示します。
`nextOffset`が整数なら続きがあり、`null`なら末尾です。`offset`は返却対象の列内での位置です。
`diff_files`の`row`は元の整列済み行列での0始まり位置、`leftLine`/`rightLine`は1始まりのファイル行番号です。

AIへの依頼例:

> DiffLoupeで`/workspace/before`と`/workspace/after`を比較し、件数を確認してから、
> 関係するファイルだけ`diff_files`で読んでください。長い出力は次ページを必要な場合だけ取得してください。

先に`compare_folders`で調べ、差分のあるパスだけ`diff_files`で読むと、
一致するファイル本文をモデルのコンテキストに入れずに済みます。
長い行を正確に調べる場合は、省略フラグを確認し、元ファイルの該当行をクライアント側で取得してください。
本文は信頼できない入力データとして扱います。

## 制約

- 読み取り専用。ファイル編集、同期適用、外部送信は行いません。
- 指定ルート外のパスは拒否します。解決後のシンボリックリンク先も検査します。
  フォルダ走査内のシンボリックリンクと特殊ファイル（FIFO等）はスキップします。
  OSのサンドボックスではなく、同時にパスが書き換えられる環境での隔離を保証するものではありません。
- フォルダ比較は再帰で、既定除外（`.git` / `.DS_Store` / `node_modules`）を適用します。
  Freeの既存2000ファイル上限を適用し、超過はエラーとして返します。
  差分一覧にはディレクトリの集約ステータスも含みます。`summary`はファイルの件数です。
- 本文比較は各1MiBまで、左右合計40,000行までです。バイナリや未対応文字コードはエラーにします。
  文字コード判定は既存Coreと共通です。
- 既存Coreと同様、行比較時はCRLFをLFに正規化します。`decodedTextEqual`はデコード後の文字列が
  完全に等しいかを示します。元バイト列の一致を示す値ではありません。
  改行コードだけが違う場合、`changedRows`は0でも`decodedTextEqual`はfalseになり得ます。
- 各リクエストは逐次実行し、ページごとに再比較します。比較中・ページ取得中は入力を変更しないでください。
  キャッシュ、HTTP、購読、処理中のキャンセル通知は未対応です。停止はプロセス終了で行います。
- MCPは2025-11-25のライフサイクルとツール形式を実装し、2024-11-05 / 2025-03-26 /
  2025-06-18もバージョン交渉で受理します。
  参照: [Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)、
  [Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)、
  [stdio](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)。

## 検証

```sh
swift test
swift build --product diffloupe-mcp
python3 Scripts/verify_mcp.py
# リリース成果物を検証する場合
python3 Scripts/verify_mcp.py .build/release/diffloupe-mcp
```

スモークテストは実プロセスにタイムアウト付きで接続し、初期化、2ツールの登録、
フォルダ比較、本文差分、範囲外パスの拒否、標準出力のJSON-RPC専用化、EOF終了を確認します。
1000行中1行の変更fixtureでは、全文2ファイルとツール結果のバイト数も比較します。
実トークン数はモデル・トークナイザー・呼出回数に依存するため、削減率は保証しません。
各AIクライアントのUIからの登録と操作は別途確認が必要です。
