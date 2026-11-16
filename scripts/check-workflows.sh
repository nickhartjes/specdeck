#!/usr/bin/env bash
# Fails when a workflow uses a third-party action by anything but a full commit SHA with its
# version in a comment (`owner/action@<40 hex> # v1.2.3`), has no top-level `permissions:`, or
# grants `permissions: write-all` (release-integrity: Actions pinned by commit, Least privilege for
# workflow tokens). `uses:` counts in block style and in flow style (`- {uses: ...}`). Local
# actions and workflows (`./...`) are exempt. Prints each offending file and line.
# Tests: scripts/check-workflows.test.sh
# Usage: check-workflows.sh [workflows dir]
set -euo pipefail
dir=${1:-.github/workflows}
status=0
found=0
for file in "$dir"/*.yml "$dir"/*.yaml; do
  [ -f "$file" ] || continue
  found=1
  if ! grep -q '^permissions:' "$file"; then
    echo "$file: no top-level permissions:" >&2
    status=1
  fi
  line_no=0
  while IFS= read -r line || [ -n "$line" ]; do
    line_no=$((line_no + 1))
    # At the top level or in a job: the token may then do anything.
    if [[ $line =~ ^[[:space:]]*permissions:[[:space:]]*[\"\']?write-all ]]; then
      echo "$file:$line_no: permissions: write-all" >&2
      status=1
    fi
    # `uses:` as a key: at the start of the line after an optional list dash, or after `{` or `,`
    # in a flow mapping. Quotes around the key or the value are allowed.
    rest=$line
    while [[ $rest =~ (^|[{,])[[:space:]]*(-[[:space:]]+)?[\"\']?uses[\"\']?:[[:space:]]*[\"\']?([^\"\'[:space:]#,{}]+) ]]; do
      rest=${rest#*"${BASH_REMATCH[0]}"}
      ref=${BASH_REMATCH[3]}
      case "$ref" in ./*) continue ;; esac
      if ! [[ $ref =~ @[0-9a-f]{40}$ ]]; then
        echo "$file:$line_no: not pinned to a commit SHA: $ref" >&2
        status=1
      elif ! [[ $line =~ \#[[:space:]]*v[0-9] ]]; then
        echo "$file:$line_no: no version comment after the SHA: $ref" >&2
        status=1
      fi
    done
  done < "$file"
done
if [ "$found" -eq 0 ]; then
  echo "no workflows found in $dir" >&2
  exit 1
fi
exit "$status"
