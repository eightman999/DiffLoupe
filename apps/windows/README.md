# DiffLoupe for Windows (diffloupe-win)

Rust コア(`diffloupe-core`)を直接使う Win32 ネイティブ GUI。Free 版。
WinUI/XAML・WebView・ランタイム同梱なし。実行ファイル単体で動作する。

## ビルド

```
cargo build --release --target x86_64-pc-windows-msvc
# 出力: target/x86_64-pc-windows-msvc/release/diffloupe-win.exe
```

Per-monitor DPI v2 / Common Controls v6 のマニフェストは `build.rs` が
`diffloupe-win.manifest` を link.exe に埋め込む。

## スモークテスト(ヘッドレス検証)

```
diffloupe-win.exe --smoke-test <左フォルダ> <右フォルダ> <出力先>
```

メインウィンドウを生成し、フォルダ比較を実行、テキスト差分エントリを選択して
メイン/結果/テキスト差分/Hex/画像比較/設定画面を BMP(PrintWindow)として出力し、
終了コード 0 で終了する。

## パッケージング

```
powershell -File package.ps1
```

`target/package/DiffLoupe-<ver>-windows-<arch>.zip`(exe + README)を生成する。

## コード署名について

本リポジトリでは署名しない。配布時に署名する場合は
`signtool sign /fd sha256 /a /t http://timestamp.digicert.com diffloupe-win.exe`
等で Authenticode 署名を行う。未署名 exe は SmartScreen の警告が出る点に注意。

## 設定の保存先

`%APPDATA%\DiffLoupe\settings.json`(設定・ウィンドウ位置・最近使った比較)。
