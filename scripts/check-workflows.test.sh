#!/usr/bin/env bash
# Tests scripts/check-workflows.sh: the repository's workflows pass, and each kind of unpinned
# action or broad permission fails, naming the file and line.
# Usage: scripts/check-workflows.test.sh
set -euo pipefail
cd "$(dirname "$0")/.."
check=$PWD/scripts/check-workflows.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
sha=3d3c42e5aac5ba805825da76410c181273ba90b1
status=0

if "$check" > /dev/null; then
  echo "ok       the repository's workflows pass"
else
  echo "FAILED   the repository's workflows fail" >&2
  status=1
fi

# A workflow with the given lines after a pinned checkout; `top` is its top-level permissions line.
workflow() {
  local top=$1
  shift
  rm -f "$tmp"/*.yml
  {
    echo 'on: push'
    [ -z "$top" ] || echo "$top"
    echo 'jobs:'
    echo '  build:'
    echo '    runs-on: ubuntu-latest'
    echo '    steps:'
    echo "      - uses: actions/checkout@$sha # v7.0.1"
    printf '%s\n' "$@"
  } > "$tmp/test.yml"
}

passes() {
  local what=$1
  shift
  workflow "$@"
  if out=$("$check" "$tmp" 2>&1); then
    echo "ok       passes $what"
  else
    echo "FAILED   rejected $what: $out" >&2
    status=1
  fi
}

# Fails and names the given line (with the two-line permissions, an added line is line 9), or the file.
fails() {
  local what=$1 where=$2
  shift 2
  workflow "$@"
  if out=$("$check" "$tmp" 2>&1); then
    echo "FAILED   accepted $what" >&2
    status=1
  elif [[ $out != *"$tmp/test.yml$where"* ]]; then
    echo "FAILED   did not name test.yml$where for $what: $out" >&2
    status=1
  else
    echo "ok       rejects $what"
  fi
}

read=$'permissions:\n  contents: read'
passes 'a pinned action with its version' "$read"
passes 'a pinned flow-style step' "$read" "      - {name: x, uses: actions/cache@$sha} # v6.1.0"
passes 'a local action' "$read" '      - uses: ./.github/actions/x'
passes 'read-all' 'permissions: read-all'
passes 'empty permissions' 'permissions: {}'
fails 'an action by tag' :9 "$read" '      - uses: actions/cache@v4'
fails 'a quoted action by tag' :9 "$read" '      - uses: "actions/cache@v4"'
fails 'a flow-style step by tag' :9 "$read" '      - {uses: owner/x@v1}'
fails 'a flow-style step by tag after a name' :9 "$read" '      - {name: x, uses: owner/x@v1}'
fails 'a quoted flow-style key' :9 "$read" '      - {"uses": owner/x@v1}'
fails 'a pin without a version comment' :9 "$read" "      - uses: actions/cache@$sha"
fails 'a short SHA' :9 "$read" '      - uses: actions/cache@3d3c42e'
fails 'a workflow without permissions' ': no top-level' ''
fails 'top-level write-all' :2 'permissions: write-all'
fails 'a job with write-all' :9 "$read" '    permissions: write-all'

exit "$status"
