#!/usr/bin/env bash
# Merges the newest official ZapFast release into ZapFast Plus.
# Exit 0 with "updated=true|false" in $GITHUB_OUTPUT; exit 1 on a conflict
# in ZapFast's own code that needs a person (or Claude) to resolve.
set -euo pipefail
cd "$(dirname "$0")/.."
latest=${1:-$(gh api repos/crmne/zapfast/releases/latest -q .tag_name)}
current=$(tr -d ' \n' < plus/UPSTREAM)
if [ "$latest" = "$current" ]; then
  echo "Already on $current."
  echo "updated=false" >> "${GITHUB_OUTPUT:-/dev/null}"
  exit 0
fi
git remote get-url upstream >/dev/null 2>&1 || git remote add upstream https://github.com/crmne/zapfast
git fetch --quiet upstream "refs/tags/$latest:refs/tags/$latest"
if ! git merge --no-edit --no-ff -m "Merge ZapFast $latest into ZapFast Plus" "$latest"; then
  # The version lines always clash: take ZapFast's and set ours again.
  for file in Cargo.toml Cargo.lock; do
    if git diff --name-only --diff-filter=U | grep -qx "$file"; then
      git checkout --theirs -- "$file" && git add "$file"
    fi
  done
  # ZapFast's own workflows are not run here.
  git diff --name-only --diff-filter=U | grep '^\.github/workflows/' | grep -v '/plus-' | xargs -r git rm -q --
  if [ -n "$(git diff --name-only --diff-filter=U)" ]; then
    echo "Conflicts that need resolving:"
    git diff --name-only --diff-filter=U
    git merge --abort
    exit 1
  fi
  git commit --no-edit -q
fi
# Workflows ZapFast added in this release stay out too.
for file in .github/workflows/*.yml; do
  case "$(basename "$file")" in plus-*) ;; *) git rm -q -- "$file" ;; esac
done
echo "$latest" > plus/UPSTREAM
echo 1 > plus/REVISION
version=$(plus/set-version.sh)
git add -A plus/UPSTREAM plus/REVISION Cargo.toml Cargo.lock .github/workflows
git commit -q -m "ZapFast Plus $version, on ZapFast $latest"
echo "Now ZapFast Plus $version on ZapFast $latest."
echo "updated=true" >> "${GITHUB_OUTPUT:-/dev/null}"
