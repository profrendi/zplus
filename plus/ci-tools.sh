#!/usr/bin/env bash
# Installs what the ZFPlus workflows need, from ~/apt-debs when an earlier
# run kept the packages there (actions/cache), so only the package lists are
# fetched; whatever had to be downloaded is added to ~/apt-debs for next time.
set -euo pipefail
packages=(mingw-w64 wixl msitools zip nasm cmake perl
  libxkbcommon-dev libwayland-dev libgl1-mesa-dev libasound2-dev libpipewire-0.3-dev clang libclang-dev)
debs="$HOME/apt-debs"
mkdir -p "$debs"
if compgen -G "$debs/*.deb" >/dev/null; then
  sudo cp "$debs"/*.deb /var/cache/apt/archives/
fi
sudo apt-get update -q -o Acquire::Retries=3
sudo apt-get install -y -q --no-install-recommends -o APT::Keep-Downloaded-Packages=true \
  -o Acquire::Retries=3 "${packages[@]}"
cp -n /var/cache/apt/archives/*.deb "$debs"/ 2>/dev/null || true
rustup show active-toolchain || rustup toolchain install
rustup target add x86_64-pc-windows-gnu
