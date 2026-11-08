#!/usr/bin/env bash
# Sets ZFPlus's version from the official release it is built on
# (plus/UPSTREAM, such as v0.21.1) and its own revision (plus/REVISION):
# major.minor.(patch * 100 + revision), so 0.21.1 revision 1 is 0.21.101.
# Every official release and every Plus revision then counts as newer, which
# the updater needs. Prints the version.
set -euo pipefail
cd "$(dirname "$0")/.."
upstream=$(tr -d ' \n' < plus/UPSTREAM)
revision=$(tr -d ' \n' < plus/REVISION)
IFS=. read -r major minor patch <<< "${upstream#v}"
version="$major.$minor.$((patch * 100 + revision))"
# The package's own version, in the manifest and the lock file.
python3 - "$version" <<'PY'
import re, sys
version = sys.argv[1]
with open("Cargo.toml") as f:
    text = f.read()
text = re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', text, count=1)
with open("Cargo.toml", "w") as f:
    f.write(text)
with open("Cargo.lock") as f:
    text = f.read()
text = re.sub(r'(name = "zapfast"\nversion = )"[^"]+"', rf'\1"{version}"', text, count=1)
with open("Cargo.lock", "w") as f:
    f.write(text)
PY
echo "$version"
