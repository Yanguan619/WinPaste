$ErrorActionPreference = 'Stop'
$out = 'artifacts/portable'

New-Item -ItemType Directory -Force -Path $out | Out-Null
Get-ChildItem 'src-tauri/target/release' -Filter *.exe | Copy-Item -Destination $out -Force

$exe = Join-Path $out 'winpaste-app.exe'
if (Get-Command upx -ErrorAction SilentlyContinue) {
    upx -1 $exe
    Write-Host "UPX compressed: $exe"
} else {
    Write-Host "UPX not found, skipping compression. Install from https://upx.github.io/"
}

Write-Host "Portable build complete. Output: $out"
