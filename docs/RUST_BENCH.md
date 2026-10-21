# RUST_BENCH — Swift CLI vs Rust CLI ベンチマーク

計測: `core/benches/compare.sh`(同条件で複数回計測し最小値を採用、wall time)
環境: macOS(Apple Silicon)、両バイナリ release ビルド

## 結果

| ケース | Swift | Rust | 比(Rust/Swift) |
|---|---:|---:|---:|
| ストレスフィクスチャ(10,000ファイル再帰比較) | 606.3 ms | 468.1 ms | **0.772** |
| 大テキスト差分(35,000行×2、サイズ一致→ハッシュ比較) | 23.1 ms | 20.6 ms | **0.891** |
| 大バイナリ(64 MiB、SHA-256 比較) | 81.1 ms | 82.4 ms | **1.016** |

## 所見

- ストレス 10k ファイルでは約 23% 高速。走査・突き合わせ・並列ハッシュの
  オーバーヘッドが小さいことが要因と考えられる。
- 大テキストもわずかに高速(約 11%)。
- 大バイナリハッシュはほぼ同等(±2%以内で誤差範囲)。両者ともストリーミング
  SHA-256 で I/O バウンドのため、実装差は出にくい。
- すべてのケースで Swift 以上の速度を確認済み。

## 再現方法

```sh
cd core && cargo build --release
cd ../SwiftUI && swift build -c release --product diffloupe
cd .. && core/benches/compare.sh 5
```
