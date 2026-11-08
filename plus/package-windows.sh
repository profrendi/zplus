#!/usr/bin/env bash
# Packages a Windows build of ZFPlus the way the updater expects:
#   zapfast-v<version>-<target>.zip   portable: a folder with zapfast.exe and
#                                     the portable marker
#   zapfast-v<version>-<target>.msi   per-user installer, no admin rights
#   checksums.txt (+ .sig)            SHA-256 of both, signed with Ed25519
# Usage: plus/package-windows.sh <zapfast.exe> <version> <target> <out dir> [signing key PEM]
set -euo pipefail
exe=$1 version=$2 target=$3 out=$4 key=${5:-}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
stem="zapfast-v$version-$target"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out" "$work/$stem" "$work/msi"
# Absolute, since the archive is made from inside the work folder.
out=$(cd "$out" && pwd)

# Portable archive.
cp "$exe" "$work/$stem/zapfast.exe"
cp "$root/packaging/zapfast-portable.txt" "$work/$stem/zapfast-portable.txt"
cp "$here/README-ZFPlus.txt" "$root/LICENSE" "$work/$stem/"
(cd "$work" && rm -f "$out/$stem.zip" && zip -qr9 "$out/$stem.zip" "$stem")

# Installer.
cp "$exe" "$work/msi/zapfast.exe"
cp "$root/packaging/windows/zapfast-installer.txt" "$work/msi/"
cp "$here/README-ZFPlus.txt" "$work/msi/"
IFS=. read -r major minor patch <<< "$version"
# Windows Installer versions stop at 255.255.65535.
wixl -a x64 \
  -D Version="$major.$minor.$patch" \
  -D SourceDir="$work/msi" \
  -D IconFile="$root/packaging/windows/zfplus.ico" \
  -o "$out/$stem.msi" "$here/zfplus.wxs"

# Signed checksums.
(cd "$out" && sha256sum "$stem.zip" "$stem.msi" > checksums.txt)
if [ -n "$key" ]; then
  openssl pkeyutl -sign -inkey "$key" -rawin -in "$out/checksums.txt" -out "$out/checksums.txt.sig"
  test "$(wc -c < "$out/checksums.txt.sig")" -eq 64
fi
ls -la "$out"
