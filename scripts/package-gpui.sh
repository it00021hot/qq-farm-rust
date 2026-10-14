#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${1:-release}"
TARGET="${TARGET:-}"
TARGET_DIR="$ROOT/target"
if [[ -n "$TARGET" ]]; then
  TARGET_DIR="$TARGET_DIR/$TARGET"
fi
BIN="$TARGET_DIR/$PROFILE/qq-farm-gpui"
DIST="$ROOT/dist"
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/qq-farm-gpui-package.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

if [[ ! -x "$BIN" ]]; then
  echo "Missing $BIN. Build with cargo build --$PROFILE -p qq-farm-gpui first." >&2
  exit 1
fi

copy_resources() {
  local destination="$1"
  mkdir -p "$destination/assets" "$destination/activity-assets"
  cp -R "$ROOT/assets/game_config" "$destination/assets/game_config"
  cp -R "$ROOT/desktop-ui/public/activity-assets/." "$destination/activity-assets/"
  cp "$ROOT/assets/tsdk.wasm" "$destination/tsdk.wasm"
  cp "$ROOT/assets/tsdk-wx.wasm" "$destination/tsdk-wx.wasm"
  find "$destination" -name '.DS_Store' -type f -delete
}

mkdir -p "$DIST"
case "$(uname -s)" in
  Darwin)
    ARCH="$(uname -m)"
    case "$TARGET" in
      aarch64-apple-darwin) ARCH="aarch64" ;;
      x86_64-apple-darwin) ARCH="x86_64" ;;
      "")
        if [[ "$ARCH" == "arm64" ]]; then ARCH="aarch64"; fi
        ;;
    esac
    APP="$STAGE/QQ Farm.app"
    mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    cp "$BIN" "$APP/Contents/MacOS/qq-farm-gpui"
    cp "$ROOT/crates/qq-farm-desktop/icons/icon.icns" "$APP/Contents/Resources/AppIcon.icns"
    copy_resources "$APP/Contents/Resources"
    cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>qq-farm-gpui</string>
  <key>CFBundleIdentifier</key><string>com.qqfarm.gpui</string>
  <key>CFBundleName</key><string>QQ Farm</string>
  <key>CFBundleDisplayName</key><string>QQ Farm</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
    APP_DIST="$STAGE/QQ Farm-${ARCH}.app"
    ditto "$APP" "$APP_DIST"
    rm -rf "$DIST/QQ Farm-${ARCH}.app"
    mv "$APP_DIST" "$DIST/QQ Farm-${ARCH}.app"
    ditto -c -k --norsrc --noextattr --keepParent "$APP" "$DIST/qq-farm-gpui-macos-${ARCH}.app.zip"
    ;;
  Linux)
    ARCH="$(uname -m)"
    PACKAGE="$STAGE/qq-farm-gpui"
    mkdir -p "$PACKAGE"
    cp "$BIN" "$PACKAGE/qq-farm-gpui"
    copy_resources "$PACKAGE"
    cp "$ROOT/README.md" "$PACKAGE/README.md"
    tar -C "$STAGE" -czf "$DIST/qq-farm-gpui-linux-${ARCH}.tar.gz" qq-farm-gpui
    ;;
  *)
    echo "Run scripts/package-gpui.ps1 on Windows." >&2
    exit 2
    ;;
esac
