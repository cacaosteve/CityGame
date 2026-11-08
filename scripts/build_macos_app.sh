#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

if [ ! -f "$root/assets/CityGame.icns" ] ||
    [ "$root/assets/citygame-icon.png" -nt "$root/assets/CityGame.icns" ] ||
    [ ! -f "$root/assets/CityGameCity.icns" ] ||
    [ "$root/assets/citygame-cityfile-icon.png" -nt "$root/assets/CityGameCity.icns" ] ||
    [ "$root/scripts/build_app_icons.sh" -nt "$root/assets/CityGame.icns" ]; then
    sh "$root/scripts/build_app_icons.sh" --macos-only
fi

cargo build --release

bundle="$root/target/CityGame.app"
contents="$bundle/Contents"
rm -rf "$bundle"
mkdir -p "$contents/MacOS" "$contents/Resources"
cp "$root/target/release/citygame" "$contents/MacOS/CityGame"
cp "$root/assets/CityGame.icns" "$contents/Resources/CityGame.icns"
cp "$root/assets/CityGameCity.icns" "$contents/Resources/CityGameCity.icns"
cp "$root/assets/ui/newspaper/OFL.txt" "$contents/Resources/NewspaperTypeface-OFL.txt"
replacement_pack=${CITYGAME_REPLACEMENT_PACK:-$root/assets/replacements/our-graphics}
if [ -f "$replacement_pack/asset-pack.json" ]; then
    mkdir -p "$contents/Resources/replacements"
    cp -R "$replacement_pack" "$contents/Resources/replacements/our-graphics"
fi

cat > "$contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>CityGame</string>
    <key>CFBundleIconFile</key>
    <string>CityGame.icns</string>
    <key>CFBundleIdentifier</key>
    <string>org.citygame.desktop</string>
    <key>CFBundleName</key>
    <string>CityGame</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key>
            <string>org.citygame.sc2-city</string>
            <key>UTTypeDescription</key>
            <string>CityGame City</string>
            <key>UTTypeConformsTo</key>
            <array>
                <string>public.data</string>
            </array>
            <key>UTTypeTagSpecification</key>
            <dict>
                <key>public.filename-extension</key>
                <array>
                    <string>sc2</string>
                </array>
            </dict>
        </dict>
    </array>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>CityGame City</string>
            <key>CFBundleTypeIconFile</key>
            <string>CityGameCity.icns</string>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
            <key>LSHandlerRank</key>
            <string>Owner</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>org.citygame.sc2-city</string>
            </array>
        </dict>
    </array>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.games</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

chmod +x "$contents/MacOS/CityGame"
codesign --force --sign - "$bundle"
printf 'Built %s\n' "$bundle"
