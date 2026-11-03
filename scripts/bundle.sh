#!/bin/sh
# Build dist/PersonalOEE.app (ad-hoc signed). Release by default; pass "debug"
# for a debug build, which includes practice mode (D-45).
set -e
cd "$(dirname "$0")/.."
PROFILE="${1:-release}"
if [ "$PROFILE" = "debug" ]; then
  cargo build -p oee-app
else
  cargo build --release -p oee-app
fi
APP=dist/PersonalOEE.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# App icon: build an .icns from the 1024px master in assets/.
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size assets/AppIcon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) assets/AppIcon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
cp "target/$PROFILE/PersonalOEE" "$APP/Contents/MacOS/"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Personal OEE</string>
<key>CFBundleDisplayName</key><string>Personal OEE</string>
<key>CFBundleIdentifier</key><string>com.tristanbailey.personaloee</string>
<key>CFBundleExecutable</key><string>PersonalOEE</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundleVersion</key><string>1.0.3</string>
<key>CFBundleShortVersionString</key><string>1.0.3</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
echo "Built $APP"
