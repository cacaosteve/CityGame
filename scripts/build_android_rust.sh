#!/bin/sh
# Build libcitygame.so (+ SDL) for Android arm64 and stage into jniLibs.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

# Minimal PATH when invoked from IDEs / Gradle.
export PATH="${HOME}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"
if [ -f "${HOME}/.cargo/env" ]; then
    # shellcheck disable=SC1091
    . "${HOME}/.cargo/env"
fi

api=${ANDROID_API_LEVEL:-24}
abi=arm64-v8a
triple=aarch64-linux-android

if ! command -v cargo-ndk >/dev/null; then
    echo "cargo-ndk is required (cargo install cargo-ndk)" >&2
    exit 1
fi

echo "==> cargo ndk -t $abi -P $api build --release --no-default-features"
export ANDROID_NDK="${ANDROID_NDK:-${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/29.0.14206865}}"
export ANDROID_NDK_HOME="$ANDROID_NDK"
export CMAKE_TOOLCHAIN_FILE="$ANDROID_NDK/build/cmake/android.toolchain.cmake"
export CMAKE_MAKE_PROGRAM="${CMAKE_MAKE_PROGRAM:-$(command -v make)}"
cargo ndk -t "$abi" -P "$api" -o "$root/packaging/android/app/src/main/jniLibs" \
    build --release --no-default-features

# cargo-ndk -o places libcitygame.so under jniLibs/<abi>/
jni="$root/packaging/android/app/src/main/jniLibs/$abi"
so="$jni/libcitygame.so"
if [ ! -f "$so" ]; then
    # Fallback: copy from target/
    built="$root/target/$triple/release/libcitygame.so"
    mkdir -p "$jni"
    cp -f "$built" "$so"
fi

out="$root/target/$triple/release"
sdl_so=$(find "$out/build" -name 'libSDL3.so' 2>/dev/null | head -1 || true)
if [ -z "$sdl_so" ]; then
    sdl_so=$(find "$root/target/$triple" -name 'libSDL3.so' 2>/dev/null | head -1 || true)
fi
if [ -n "$sdl_so" ]; then
    cp -f "$sdl_so" "$jni/libSDL3.so"
    printf 'SDL: %s\n' "$sdl_so"
else
    echo "warning: libSDL3.so not found — Activity load may fail" >&2
fi

python3 "$root/scripts/make_mobile_seed_zip.py" \
    "$root/packaging/android/app/src/main/assets/SeedAssets.zip"

printf 'jniLibs %s:\n' "$jni"
ls -lah "$jni"
