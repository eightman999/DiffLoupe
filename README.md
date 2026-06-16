# DiffLoupe

フォルダ・ファイルを比較する差分ツール「ディフルーペ」。
「WinMerge難民のためのMac差分ツール」をコンセプトに、現在は **macOSネイティブ(Swift)版** を主軸に開発しています。

## 実装一覧

| 版 | 場所 | 位置づけ |
|---|---|---|
| **macOSネイティブ(Swift)版** | [SwiftUI/](SwiftUI/) | **現行の開発主軸**。SwiftUI製・外部依存ゼロ・macOS 13+ |
| C++/Qt版(レガシー) | [legacy/cpp/](legacy/cpp/) | クロスプラットフォームの旧実装 |
| Python/PySide6版(レガシー) | [legacy/Python/](legacy/Python/) | 旧実装 |

新規に使う・ビルドする場合は **[SwiftUI/README.md](SwiftUI/README.md)** を参照してください。
過去の実装(C++/Qt版・Python版)はすべて [legacy/](legacy/) にまとめてあります。

## ディレクトリ構成

```
DiffLoupe/
├── SwiftUI/        # macOSネイティブ(Swift)版 ← 現行の開発主軸
│   ├── DiffLoupe/          アプリ本体(Models / Engine / UI)
│   ├── DiffLoupeTests/     XCTest
│   ├── SPEC.md             比較アルゴリズム仕様書
│   └── README.md           ビルド方法・スクリーンショット
├── legacy/         # 旧実装(Swift版より前)をまとめた場所
│   ├── cpp/                C++/Qt版(src/, forms/, resources/, CMakeLists.txt ...)
│   ├── Python/             Python/PySide6版
│   ├── STRUCTURE.md        旧構成メモ
│   └── SAGYOU.md           作業メモ
├── README.md       # このファイル(リポジトリの入口)
├── CLAUDE.md / AGENTS.md / GEMINI.md   # AIエージェント向け共通指示
└── ...
```

## ライセンス / オープンコア

本プロジェクトは「オープンコア」構成を採用しています。

- **Free (OSS)**: コア機能（無制限のフォルダ比較、テキスト差分、画像比較など）は **MIT License** で完全自由にご利用いただけます。ファイル数の制限もありません。
- **Pro (商用)**: 商用ライセンス（Pro版）向けの機能やコンポーネントは分離して提供されます。以下のファイル/ディレクトリは MIT License の対象外であり、商用コンポーネントとして扱われます:
  - `SwiftUI/DiffLoupe/Pro/LicenseManager.swift`
  - `SwiftUI/tools/generate_license.swift`

Copyright (c) 2026 eightman999 (Furin-lab)
