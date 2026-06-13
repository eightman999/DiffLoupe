# legacy — Swift版より前の実装

このディレクトリには、macOSネイティブ(Swift)版に作り直す前の **旧実装** をまとめています。
現在の開発主軸は [../SwiftUI/](../SwiftUI/) です。新規の利用・ビルドはそちらを参照してください。

ここの実装はリファレンス・履歴として残しています(Swift版の仕様
[../SwiftUI/SPEC.md](../SwiftUI/SPEC.md) は、これらのコードを読解して策定されました)。

## 内容

| ディレクトリ/ファイル | 内容 |
|---|---|
| [cpp/](cpp/) | C++/Qt版(`src/`, `forms/`, `resources/`, `CMakeLists.txt`, `CPP_GUIDE.md`, `Info.plist.in`, ビルド済み `install/`)。ビルド方法は [cpp/README.md](cpp/README.md) |
| [Python/](Python/) | Python/PySide6版。起動・依存は [Python/CLAUDE.md](Python/CLAUDE.md) |
| [STRUCTURE.md](STRUCTURE.md) | 旧2系統(C++/Python)構成の説明メモ |
| [SAGYOU.md](SAGYOU.md) | 作業メモ(icon.icns 生成手順など) |
| `test_file_a.txt` / `test_file_b.txt` | 差分動作確認用のテキスト素材 |

> 注: これらのメモ(STRUCTURE.md など)は移動前の構成を前提に書かれているため、
> パス表記が現在と一致しない箇所があります。履歴としてそのまま残しています。
