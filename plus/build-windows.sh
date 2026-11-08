#!/usr/bin/env bash
# Builds ZapFast Plus for 64-bit Windows on Linux, with MinGW, and packages
# it. Needs: rustup (the toolchain in rust-toolchain.toml with the
# x86_64-pc-windows-gnu target), mingw-w64, wixl, zip, openssl.
# Usage: plus/build-windows.sh <out dir> [signing key PEM]
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-dist}
key=${2:-}
target=x86_64-pc-windows-gnu
version=$(plus/set-version.sh)
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$PWD/plus/mingw-link.sh"
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
export RANLIB_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ranlib
export RC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-windres
export CARGO_PROFILE_RELEASE_LTO=off
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
# The update channel: this repository's releases.
export ZAPFAST_PLUS_REPO="${ZAPFAST_PLUS_REPO:-${GITHUB_REPOSITORY:-profrendi/zplus}}"
cargo build --locked --release --target "$target"
plus/package-windows.sh "target/$target/release/zapfast.exe" "$version" "$target" "$out" "$key"
echo "version=$version" >> "${GITHUB_OUTPUT:-/dev/null}"
