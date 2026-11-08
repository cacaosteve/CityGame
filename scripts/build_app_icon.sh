#!/bin/sh
# Back-compat entry point — generates icons for every supported platform.
exec "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/build_app_icons.sh"
