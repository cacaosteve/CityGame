#!/bin/sh
# Stage SeedAssets.zip for the next iOS install (never commit SC2K media).
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
python3 "$root/scripts/make_mobile_seed_zip.py" "$root/packaging/ios/CityGame/SeedAssets.zip"
printf 'Staged %s\n' "$root/packaging/ios/CityGame/SeedAssets.zip"
