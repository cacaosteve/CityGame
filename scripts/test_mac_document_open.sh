#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/citygame-document-open.XXXXXX")
trap 'rm -rf "$tmpdir"' EXIT

xcrun clang -fobjc-arc -framework Foundation \
    native/mac_document_open.m scripts/test_mac_document_open.m \
    -o "$tmpdir/test-mac-document-open"
"$tmpdir/test-mac-document-open"
