# DiffLoupe Qt版

超軽量・超高速なフォルダ差分表示ツール「ディフルーペ」のC++/Qt実装版

## 特徴

- **超軽量**: 数MB級の実行ファイル
- **超高速**: C++/Qtによる高速処理、並列化対応
- **クロスプラットフォーム**: Windows (Me以降)、macOS、Linux対応
- **レガシー環境対応**: 古いOSでも動作
- **豊富な機能**: テキスト差分、画像比較、バイナリ表示

## ビルド方法

### 必要環境

- Qt 5.9以上（推奨: Qt 5.15 LTS または Qt 6.x）
- CMake 3.16以上
- C++17対応コンパイラ（GCC 7+、Clang 5+、MSVC 2017+）

### ビルド手順

```bash
# リポジトリのクローン
git clone https://github.com/yourname/diffloupe-qt.git
cd diffloupe-qt

# ビルドディレクトリ作成
mkdir build && cd build

# CMake設定
cmake ..

# macOS環境では "WrapVulkanHeaders" が見つからないという警告が表示される場合がありますが、
# 本アプリのビルドには影響しません。
# そのまま続行して問題ありません。

# ビルド
cmake --build .

# macOSの場合はビルド後に macdeployqt を実行してFrameworkをバンドルします
# (Qt6では自動で行われます)
macdeployqt DiffLoupe.app

# Gatekeeperにより"破損している"と表示される場合は以下を実行してください
xattr -cr DiffLoupe.app

# 実行
./DiffLoupe
```

### スタティックビルド（配布用）

```bash
# Qt静的ライブラリを使用してビルド
cmake .. -DCMAKE_PREFIX_PATH=/path/to/qt-static
cmake --build . --config Release
```

## プロジェクト構造

```
DiffLoupe-Qt/
├── CMakeLists.txt          # CMakeビルド設定
├── Python                  # Pythonファイル
├── README.md               # このファイル
├── src/                    # ソースコード
│   ├── main.cpp           # エントリーポイント
│   ├── mainwindow.cpp/h   # メインウィンドウ
│   ├── core/              # コア機能
│   │   ├── comparator.cpp/h    # 比較エンジン
│   │   ├── fileinfo.cpp/h      # ファイル情報
│   │   └── diffresult.cpp/h    # 差分結果
│   ├── ui/                # UIコンポーネント
│   │   ├── diffviewer.cpp/h    # テキスト差分表示
│   │   ├── imageviewer.cpp/h   # 画像比較表示
│   │   └── hexviewer.cpp/h     # バイナリ表示
│   └── workers/           # 非同期処理
│       ├── compareworker.cpp/h  # 比較ワーカー
│       └── fileloadworker.cpp/h # ファイル読込ワーカー
├── forms/                 # UIフォーム
│   └── mainwindow.ui     # メインウィンドウUI
├── resources/            # リソース
│   ├── resources.qrc     # リソースファイル
│   ├── icons/           # アイコン
│   └── translations/    # 翻訳ファイル
└── tests/               # テスト
```

## 実装状況

### 完了
- [x] プロジェクト構造
- [x] CMake設定
- [x] 基本的なヘッダーファイル
- [x] UI設計（Qt Designer）

### 実装中
- [ ] コア比較エンジン
- [ ] メインウィンドウ実装
- [ ] 非同期処理

### 未実装
- [ ] テキスト差分表示
- [ ] 画像比較機能
- [ ] バイナリ表示
- [ ] フィルター/ソート機能
- [ ] テスト
- [ ] 配布パッケージング

## 機能一覧

### コア機能
- フォルダ間の差分比較
- メタデータ比較（サイズ、更新日時）
- ハッシュベース比較（SHA256）
- 並列処理対応

### UI機能
- ツリービューでの結果表示
- 3つの表示モード（テキスト/画像/バイナリ）
- フィルター機能（すべて/欠落・新規/差分のみ）
- ソート機能（名前/更新日/サイズ）
- 隠しファイル表示切り替え

### 差分表示
- テキスト差分（行番号付き、省略機能）
- 画像比較（左右/切り替え/スライス）
- バイナリ表示（16進ダンプ）

## ライセンス

（ライセンスを記載）

## 貢献

プルリクエスト歓迎！

## 作者

（作者情報を記載）