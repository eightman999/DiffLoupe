# diffloupe-win ポータブル ZIP 作成スクリプト。
# 使い方: powershell -File package.ps1 [-Arch x86_64-pc-windows-msvc]
# 署名は行わない(README の「コード署名について」を参照)。

param(
    [string]$Arch = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $root "target/$Arch/release/diffloupe-win.exe"
if (-not (Test-Path $exe)) {
    Write-Error "release ビルドがありません: cargo build --release --target $Arch"
}

$ver = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "(.+)"').Matches.Groups[1].Value
$dir = "DiffLoupe-$ver-windows-$Arch"
$stage = Join-Path $root "target/package/$dir"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item $exe (Join-Path $stage "DiffLoupe.exe")
Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.txt")

$zip = Join-Path $root "target/package/$dir.zip"
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip

$size = (Get-Item $exe).Length / 1KB
Write-Output ("作成: {0} (exe {1:N1} KB)" -f $zip, $size)
