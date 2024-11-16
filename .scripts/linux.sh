#!/bin/bash

if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

set -e

echo "Building Scope..."
cargo build --release

echo "Creating AppImage structure..."
mkdir -p AppDir/usr/share/applications
mkdir -p AppDir/usr/share/icons/hicolor/256x256/apps/
mkdir -p AppDir/usr/bin

cat > AppDir/usr/share/applications/scope.desktop << EOF
[Desktop Entry]
Name=Scope
Exec=scope
Icon=scope
Type=Application
Categories=Development;
EOF

echo "Copying files..."
cp target/release/scope AppDir/usr/bin/
cp .github/scope-round-200.png AppDir/usr/share/icons/hicolor/256x256/apps/scope.png

echo "Installing AppImage dependencies..."
sudo apt-get update
sudo apt-get install -y libfuse2

echo "Creating AppImage..."
wget -O linuxdeploy-x86_64.AppImage https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage
./linuxdeploy-x86_64.AppImage --appdir AppDir --output appimage

VERSION=$(grep '^version = ' src/ui/Cargo.toml | cut -d '"' -f2)
mv Scope*.AppImage ../Scope-${VERSION}.AppImage
rm -rf AppDir linuxdeploy-x86_64.AppImage

echo "Build complete! AppImage is ready: Scope-${VERSION}.AppImage"
