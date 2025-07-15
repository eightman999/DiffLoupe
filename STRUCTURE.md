# プロジェクト構造まとめ

このリポジトリは、C++/Qt 版と Python/PySide6 版の 2 系統で構成されたフォルダ比較ツール **DiffLoupe** を含みます。主なディレクトリ構成は以下の通りです。

```
DiffLoupe/
├── CMakeLists.txt          # CMake ビルド設定
├── Python/                 # Python/PySide6 実装
├── src/                    # C++ ソースコード
│   ├── core/               # コアロジック
│   ├── ui/                 # UI コンポーネント
│   └── workers/            # 非同期処理
├── forms/                  # Qt UI ファイル (.ui)
├── resources/              # リソース (アイコン、qrc など)
└── install/                # macOS 用バンドル例
```

その他、ビルド手順や機能の詳細は `README.md` や `CPP_GUIDE.md`、`Python/CLAUDE.md` などに記載されています。
