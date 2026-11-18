#!/usr/bin/env bash
# Assemble a double-clickable Wildlands.app from the release binary and zip it.
# Usage: bash packaging/make-app.sh   (after: cargo build --release)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${ROOT}/target/release/wildlands"
OUT="${ROOT}/dist"
APP="${OUT}/Wildlands.app"
ARCH="$(uname -m)"

if [ ! -x "$BIN" ]; then
  echo "missing $BIN - run: cargo build --release" >&2
  exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "${ROOT}/packaging/Info.plist" "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"
cp "$BIN" "$APP/Contents/MacOS/Wildlands"
chmod +x "$APP/Contents/MacOS/Wildlands"

# Ad-hoc signature so Gatekeeper reports an unidentified developer rather than a
# damaged bundle; the app still needs "Open Anyway" on first launch after download.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 \
  || echo "warning: ad-hoc codesign failed, bundle is unsigned" >&2

ZIP="${OUT}/Wildlands-macos-${ARCH}.zip"
rm -f "$ZIP"
# zip -X keeps AppleDouble "._*" sidecar files out of the distributed archive.
( cd "$OUT" && zip -qryX "$(basename "$ZIP")" Wildlands.app )
echo "$ZIP"
