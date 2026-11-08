#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
home_dir=${HOME:?HOME must be set}
prefix=${PREFIX:-"$home_dir/.local"}
bindir="$prefix/bin"
datadir="$prefix/share"
desktop_id=com.citygame.CityGame

command -v cargo >/dev/null 2>&1 || {
    echo "cargo is required to build CityGame" >&2
    exit 1
}

cd "$repo_root"
cargo build --release

install -d "$bindir" \
    "$datadir/applications" \
    "$datadir/icons/hicolor/scalable/apps"
install -m 755 target/release/citygame "$bindir/citygame"
# Prefer generated multi-size icons (same art as macOS .icns); fall back to assets/.
if [ -d packaging/linux/icons/hicolor ]; then
    for dir in packaging/linux/icons/hicolor/*/apps; do
        [ -d "$dir" ] || continue
        size=$(basename "$(dirname "$dir")")
        install -d "$datadir/icons/hicolor/$size/apps"
        install -m 644 "$dir/$desktop_id.png" \
            "$datadir/icons/hicolor/$size/apps/$desktop_id.png" 2>/dev/null || true
    done
    if [ -f packaging/linux/icons/hicolor/scalable/apps/$desktop_id.svg ]; then
        install -m 644 packaging/linux/icons/hicolor/scalable/apps/$desktop_id.svg \
            "$datadir/icons/hicolor/scalable/apps/$desktop_id.svg"
    fi
else
    install -d "$datadir/icons/hicolor/1024x1024/apps"
    install -m 644 assets/citygame-icon.png \
        "$datadir/icons/hicolor/1024x1024/apps/$desktop_id.png"
    install -m 644 assets/citygame-icon.svg \
        "$datadir/icons/hicolor/scalable/apps/$desktop_id.svg"
fi

desktop_exec_path=$(printf '%s' "$bindir/citygame" | sed 's/\\/\\\\/g; s/"/\\"/g')
desktop_exec="\"$desktop_exec_path\""
sed "s|@CITYGAME_EXEC@|$desktop_exec|g" \
    packaging/linux/com.citygame.CityGame.desktop.in \
    > "$datadir/applications/$desktop_id.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$datadir/applications" || true
fi

printf 'Installed CityGame to %s\n' "$bindir/citygame"
printf 'Desktop launcher: %s\n' "$datadir/applications/$desktop_id.desktop"
