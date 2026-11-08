#!/bin/sh
# Build libcitygame.a (+ SDL from source) for iOS device or simulator.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

# Xcode Run Script phases use a minimal PATH (no ~/.cargo/bin). Prefer rustup.
export PATH="${HOME}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"
if [ -f "${HOME}/.cargo/env" ]; then
    # shellcheck disable=SC1091
    . "${HOME}/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found (install rustup and ensure ~/.cargo/bin is on PATH)" >&2
    echo "PATH=${PATH}" >&2
    exit 1
fi

profile=${CITYGAME_IOS_PROFILE:-release}
target=${1:-aarch64-apple-ios}

case "$target" in
    aarch64-apple-ios|aarch64-apple-ios-sim|x86_64-apple-ios) ;;
    *)
        echo "usage: $0 [aarch64-apple-ios|aarch64-apple-ios-sim]" >&2
        exit 1
        ;;
esac

export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"

echo "==> cargo build --target $target --no-default-features ($profile)"
if [ "$profile" = release ]; then
    cargo build --target "$target" --no-default-features --release
    out="$root/target/$target/release"
else
    cargo build --target "$target" --no-default-features
    out="$root/target/$target/debug"
fi

lib="$out/libcitygame.a"
if [ ! -f "$lib" ]; then
    echo "missing $lib" >&2
    exit 1
fi
printf 'Built %s (%s bytes)\n' "$lib" "$(wc -c < "$lib" | tr -d ' ')"

dest="$root/packaging/ios/RustLibs"
mkdir -p "$dest/lib" "$dest/include"
cp -f "$lib" "$dest/lib/libcitygame.a"

sdl_lib=$(find "$out/build" \( -name 'libSDL3.a' -o -name 'libSDL3.dylib' \) 2>/dev/null | head -1 || true)
if [ -z "$sdl_lib" ]; then
    sdl_lib=$(find "$out/build" -name 'libSDL3*.a' 2>/dev/null | head -1 || true)
fi
if [ -n "$sdl_lib" ]; then
    printf 'SDL: %s\n' "$sdl_lib"
    cp -f "$sdl_lib" "$dest/lib/"
fi

hdr_src=$(find "$HOME/.cargo/registry/src" -type d -path '*/sdl3-src-*/SDL/include/SDL3' 2>/dev/null | head -1 || true)
if [ -n "$hdr_src" ]; then
    rm -rf "$dest/include/SDL3"
    cp -R "$hdr_src" "$dest/include/SDL3"
fi

printf '%s\n' "$target" > "$dest/CURRENT_TARGET"
