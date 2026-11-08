#!/bin/sh
# Build Rust + Xcode CityGame host and optionally install to a connected iPad.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
"$root/scripts/build_app_icons.sh"
export IPHONEOS_DEPLOYMENT_TARGET=15.0

device_udid=${CITYGAME_IOS_DEVICE:-00008020-000874EE1A60802E}
dest=${1:-}
if [ -z "$dest" ]; then
    if xcrun devicectl list devices 2>/dev/null | rg -q "$device_udid"; then
        dest="platform=iOS,id=$device_udid"
        rust_target=aarch64-apple-ios
    else
        dest='platform=iOS Simulator,name=iPad Pro 13-inch (M5)'
        rust_target=aarch64-apple-ios-sim
    fi
else
    case "$dest" in
        *Simulator*) rust_target=aarch64-apple-ios-sim ;;
        *) rust_target=aarch64-apple-ios ;;
    esac
fi

"$root/scripts/build_ios_rust.sh" "$rust_target"

# Stage seed assets for the next install when a local pack exists.
if [ -n "${CITYGAME_MOBILE_ASSET_PACK:-}" ]; then
    mobile_asset_pack=$CITYGAME_MOBILE_ASSET_PACK
elif [ -d "$HOME/Library/Application Support/CityGame" ]; then
    mobile_asset_pack="$HOME/Library/Application Support/CityGame"
else
    mobile_asset_pack="$HOME/Library/Application Support/SC2K Rust Viewer"
fi
if [ -d "$mobile_asset_pack" ]; then
    "$root/scripts/seed_ios_assets.sh" "$device_udid" || true
fi

cd "$root/packaging/ios"
command -v xcodegen >/dev/null || {
    echo "xcodegen is required (brew install xcodegen)" >&2
    exit 1
}
xcodegen generate

case "$dest" in
    platform=iOS,id=*)
        xcodebuild -project CityGame.xcodeproj -scheme CityGame \
            -destination "$dest" -configuration Debug \
            -allowProvisioningUpdates \
            DEVELOPMENT_TEAM=AD7493G5S3 \
            build
        app=$(ls -d ~/Library/Developer/Xcode/DerivedData/CityGame-*/Build/Products/Debug-iphoneos/CityGame.app | head -1)
        xcrun devicectl device install app --device "$device_udid" "$app"
        xcrun devicectl device process launch --device "$device_udid" com.citygame.CityGame || true
        ;;
    *)
        xcodebuild -project CityGame.xcodeproj -scheme CityGame \
            -destination "$dest" -configuration Debug build
        ;;
esac
