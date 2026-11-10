#!/usr/bin/env bash
# Packages a Windows build of ZFPlus the way the updater expects:
#   <slug>-v<version>-<target>.zip   portable: a folder with <slug>.exe and
#                                    the portable marker
#   <slug>-v<version>-<target>.msi   per-user installer, no admin rights
#   checksums.txt (+ .sig)           SHA-256 of both, signed with Ed25519
# The slug is zapfast for ZFPlus and zfplus-nightly for ZFPlus Nightly
# (ZFPLUS_CHANNEL=nightly), each with its own installer.
# Usage: plus/package-windows.sh <zapfast.exe> <version> <target> <out dir> [signing key PEM]
set -euo pipefail
exe=$1 version=$2 target=$3 out=$4 key=${5:-}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
if [ "${ZFPLUS_CHANNEL:-}" = nightly ]; then
  slug=zfplus-nightly wxs="$here/zfplus-nightly.wxs"
else
  slug=zapfast wxs="$here/zfplus.wxs"
fi
stem="$slug-v$version-$target"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out" "$work/$stem" "$work/msi"
# Absolute, since the archive is made from inside the work folder.
out=$(cd "$out" && pwd)

# Portable archive.
cp "$exe" "$work/$stem/$slug.exe"
echo "$slug-portable-v1" > "$work/$stem/$slug-portable.txt"
cp "$here/README-ZFPlus.txt" "$root/LICENSE" "$work/$stem/"
(cd "$work" && rm -f "$out/$stem.zip" && zip -qr9 "$out/$stem.zip" "$stem")

# Installer.
cp "$exe" "$work/msi/$slug.exe"
echo "$slug-installer-v1" > "$work/msi/$slug-installer.txt"
cp "$here/README-ZFPlus.txt" "$work/msi/"
# Windows Installer versions stop at 255.255.65535. A stable version fits as
# it is; a nightly (<major>.<yyyymmdd>.<hhmm>-nightly) becomes yy.mm.ddhh,
# which still grows with every nightly.
core=${version%%-*}
IFS=. read -r major minor patch <<< "$core"
if [ "$slug" = zfplus-nightly ]; then
  msi_version="$((10#${minor:2:2})).$((10#${minor:4:2})).$((10#${minor:6:2} * 100 + 10#$patch / 100))"
else
  msi_version="$major.$minor.$patch"
fi
wixl -a x64 \
  -D Version="$msi_version" \
  -D SourceDir="$work/msi" \
  -D IconFile="$root/packaging/windows/zfplus.ico" \
  -o "$out/$stem.msi" "$wxs"

# Signed checksums.
(cd "$out" && sha256sum "$stem.zip" "$stem.msi" > checksums.txt)
if [ -n "$key" ]; then
  openssl pkeyutl -sign -inkey "$key" -rawin -in "$out/checksums.txt" -out "$out/checksums.txt.sig"
  test "$(wc -c < "$out/checksums.txt.sig")" -eq 64
fi
ls -la "$out"
