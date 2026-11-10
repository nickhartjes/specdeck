#!/usr/bin/env bash
# Tests scripts/render-cask.sh: the cask of 1.8.0 as published in nickhartjes/homebrew-tap
# (scripts/testdata/specdeck-1.8.0.rb), and a refusal with no output for each bad argument.
# Usage: scripts/render-cask.test.sh
set -euo pipefail
cd "$(dirname "$0")"
arm=a096be01ae7c282042181477cb176edb7a94c00913d466d323fd2416a4edd8d1
intel=f43235566d7e5ebeeaa0c70b218a8c755251cb06b320d91cd6500a05f282b62c
status=0

if ./render-cask.sh 1.8.0 "$arm" "$intel" nickhartjes/specdeck unsigned | diff -u testdata/specdeck-1.8.0.rb -; then
  echo "ok       1.8.0 renders the published cask"
else
  echo "FAILED   1.8.0 does not render the published cask" >&2
  status=1
fi

refuses() {
  local what=$1 out
  shift
  if out=$(./render-cask.sh "$@" 2> /dev/null); then
    echo "FAILED   accepted $what" >&2
    status=1
  elif [ -n "$out" ]; then
    echo "FAILED   printed output for $what" >&2
    status=1
  else
    echo "ok       refuses $what"
  fi
}

refuses 'a version with a quote' '1.8.0";system "x' "$arm" "$intel"
refuses 'a version with a v' v1.8.0 "$arm" "$intel"
refuses 'a prerelease version' 1.8.0-rc.1 "$arm" "$intel"
refuses 'a version with a space' '1.8.0 ' "$arm" "$intel"
refuses 'a version over two lines' $'1.8.0\n1.8.0' "$arm" "$intel"
refuses 'an empty version' '' "$arm" "$intel"
refuses 'a short arm64 hash' 1.8.0 "${arm:1}" "$intel"
refuses 'an uppercase arm64 hash' 1.8.0 "$(tr a-f A-F <<< "$arm")" "$intel"
refuses 'an x64 hash with a quote' 1.8.0 "$arm" "${intel:1}\""
refuses 'a long x64 hash' 1.8.0 "$arm" "${intel}0"
refuses 'a repository with a quote' 1.8.0 "$arm" "$intel" 'nickhartjes/spec"deck'
refuses 'a repository without an owner' 1.8.0 "$arm" "$intel" specdeck
refuses 'a repository with a space' 1.8.0 "$arm" "$intel" 'nickhartjes/spec deck'
refuses 'a repository with a #{}' 1.8.0 "$arm" "$intel" 'nickhartjes/#{x}'
refuses 'missing hashes' 1.8.0

exit "$status"
