#!/usr/bin/env bash
# Fails when the pnpm the sharing service image installs by default differs from mise.toml's, which
# CI and the release pass to it.
# Usage: check-pnpm-version.sh [mise.toml] [Dockerfile]
set -euo pipefail
mise=${1:-mise.toml}
dockerfile=${2:-docker/server.Dockerfile}
want=$(sed -n 's/^pnpm = "\(.*\)"$/\1/p' "$mise")
have=$(sed -n 's/^ARG PNPM_VERSION=\(.*\)$/\1/p' "$dockerfile")
if [ -z "$want" ] || [ "$want" != "$have" ]; then
  echo "$dockerfile installs pnpm ${have:-(no ARG PNPM_VERSION)}, $mise pins ${want:-nothing}" >&2
  exit 1
fi
echo "ok       pnpm $want in $mise and $dockerfile"
