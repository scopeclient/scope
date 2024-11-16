#!/bin/bash

# APPLE_CERTIFICATE - Your Apple Developer certificate
# APPLE_CERTIFICATE_PASSWORD - Password for your certificate
# APPLE_ID - Your Apple ID email
# APPLE_ID_PASSWORD - Your Apple ID password
# KEYCHAIN_PASSWORD - Password for the keychain
# APP_BUNDLE_ID - Your app bundle identifier (e.g., "com.scope.app")

if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

set -e

required_vars=("APPLE_CERTIFICATE" "APPLE_CERTIFICATE_PASSWORD" "APPLE_ID" "APPLE_ID_PASSWORD" "KEYCHAIN_PASSWORD" "APP_BUNDLE_ID")
for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "Error: Missing required environment variable: $var"
        exit 1
    fi
done

echo "Building Scope..."
cargo build --release

echo "Creating app bundle..."
mkdir -p Scope.app/Contents/{MacOS,Resources}
cp target/release/scope Scope.app/Contents/MacOS/
cp .github/scope-round-200.png Scope.app/Contents/Resources/scope.icns

cat > Scope.app/Contents/Info.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Scope</string>
    <key>CFBundleDisplayName</key>
    <string>Scope</string>
    <key>CFBundleIdentifier</key>
    <string>${APP_BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>$(grep '^version = ' src/ui/Cargo.toml | cut -d '"' -f2)</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleExecutable</key>
    <string>scope</string>
    <key>CFBundleIconFile</key>
    <string>scope.icns</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "Setting up certificate..."
rm -f certificate.p12
echo "$APPLE_CERTIFICATE" | base64 --decode > certificate.p12 2>/dev/null
security import certificate.p12 -P "$APPLE_CERTIFICATE_PASSWORD" -A 2>/dev/null

SIGNING_IDENTITY=$(security find-identity -v -p codesigning | grep "Apple Development" | head -1 | awk -F '"' '{print $2}')

if [ -z "$SIGNING_IDENTITY" ]; then
    echo "Error: No valid signing identity found"
    exit 1
fi

echo "Using signing identity: $SIGNING_IDENTITY"
echo "Signing application..."
codesign --force --options runtime --sign "$SIGNING_IDENTITY" Scope.app

rm -f certificate.p12

echo "Creating DMG..."
hdiutil create -volname "Scope" -srcfolder Scope.app -ov -format UDZO Scope.dmg

echo "Signing DMG..."
codesign --force --sign "$APPLE_CERTIFICATE" Scope.dmg

echo "Notarizing DMG..."
xcrun notarytool submit Scope.dmg --apple-id "$APPLE_ID" --password "$APPLE_ID_PASSWORD" --team-id "$APPLE_CERTIFICATE" --wait

echo "Stapling notarization..."
xcrun stapler staple Scope.dmg

echo "Build complete! Signed and notarized DMG is ready: Scope.dmg"
