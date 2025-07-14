# 作業メモ

## icon.icns の作成方法

1. `resources/icons/icon.png` を元に各サイズの PNG を生成します。
   macOS 付属の `sips` コマンドで次のように作成できます。
   ```bash
   mkdir icon.iconset
   sips -z 16 16     resources/icons/icon.png --out icon.iconset/icon_16x16.png
   sips -z 32 32     resources/icons/icon.png --out icon.iconset/icon_16x16@2x.png
   sips -z 32 32     resources/icons/icon.png --out icon.iconset/icon_32x32.png
   sips -z 64 64     resources/icons/icon.png --out icon.iconset/icon_32x32@2x.png
   sips -z 128 128   resources/icons/icon.png --out icon.iconset/icon_128x128.png
   sips -z 256 256   resources/icons/icon.png --out icon.iconset/icon_128x128@2x.png
   sips -z 256 256   resources/icons/icon.png --out icon.iconset/icon_256x256.png
   sips -z 512 512   resources/icons/icon.png --out icon.iconset/icon_256x256@2x.png
   sips -z 512 512   resources/icons/icon.png --out icon.iconset/icon_512x512.png
   cp resources/icons/icon.png icon.iconset/icon_512x512@2x.png
   iconutil -c icns icon.iconset -o resources/icons/icon.icns
   rm -r icon.iconset
   ```
2. 生成した `icon.icns` を `resources/icons/` に配置してから `cmake` を実行します。
   `icon.icns` が存在する場合は自動的にバンドルへ含まれます。
