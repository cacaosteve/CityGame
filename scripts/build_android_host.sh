#!/bin/sh
# Build Rust jniLibs + debug APK; optionally adb install.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
"$root/scripts/build_app_icons.sh"
chmod +x "$root/scripts/build_android_rust.sh"
"$root/scripts/build_android_rust.sh"

cd "$root/packaging/android"
if [ ! -f local.properties ]; then
    echo "sdk.dir=${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}" > local.properties
fi
./gradlew :app:assembleDebug
apk="$root/packaging/android/app/build/outputs/apk/debug/app-debug.apk"
printf 'APK: %s\n' "$apk"

if command -v adb >/dev/null && adb get-state 2>/dev/null | rg -q device; then
    adb install -r "$apk"
    adb shell am start -n com.citygame.CityGame.debug/com.citygame.CityGame.MainActivity || true
fi
