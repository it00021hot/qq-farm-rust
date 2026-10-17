$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Profile = if ($args.Count -gt 0) { $args[0] } else { "release" }
$Target = $env:TARGET
$TargetDirectory = Join-Path $Root "target"
if (-not [string]::IsNullOrWhiteSpace($Target)) { $TargetDirectory = Join-Path $TargetDirectory $Target }
$Binary = Join-Path $TargetDirectory "$Profile\qq-farm-gpui.exe"
$Architecture = if ($Target -like "aarch64*") { "aarch64" } else { "x86_64" }
$Stage = Join-Path ([System.IO.Path]::GetTempPath()) ("qq-farm-gpui-package-" + [guid]::NewGuid())
$Package = Join-Path $Stage "qq-farm-gpui"
$Dist = Join-Path $Root "dist"
if (-not (Test-Path $Binary)) { throw "Missing $Binary. Build with cargo build --$Profile -p qq-farm-gpui first." }
New-Item -ItemType Directory -Force -Path $Package, $Dist | Out-Null
try {
  Copy-Item $Binary (Join-Path $Package "qq-farm-gpui.exe")
  New-Item -ItemType Directory -Force -Path (Join-Path $Package "assets") | Out-Null
  Copy-Item (Join-Path $Root "assets\game_config") (Join-Path $Package "assets\game_config") -Recurse
  Copy-Item (Join-Path $Root "desktop-ui\public\activity-assets") (Join-Path $Package "activity-assets") -Recurse
  Copy-Item (Join-Path $Root "desktop-ui\public\land-assets") (Join-Path $Package "land-assets") -Recurse
  Copy-Item (Join-Path $Root "assets\tsdk.wasm") (Join-Path $Package "tsdk.wasm")
  Copy-Item (Join-Path $Root "assets\tsdk-wx.wasm") (Join-Path $Package "tsdk-wx.wasm")
  Copy-Item (Join-Path $Root "README.md") (Join-Path $Package "README.md")
  Get-ChildItem -Force -Path $Package -Recurse -File -Filter ".DS_Store" | Remove-Item -Force
  Compress-Archive -Path $Package -DestinationPath (Join-Path $Dist "qq-farm-gpui-windows-$Architecture.zip") -Force
}
finally {
  Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
}
