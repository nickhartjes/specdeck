#!/usr/bin/env bash
# Fails when the shipped TURN relays drift apart: the Compose and Kubernetes manifests must run the
# same coturn image tag, and in each the per-credential quota must be lower than the total quota,
# the Compose defaults included (sharing-service-limits: Relay quotas).
# Usage: check-relay-manifests.sh [compose file] [Kubernetes coturn manifest]
set -euo pipefail
compose=${1:-deploy/compose/compose.yaml}
k8s=${2:-deploy/k8s/base/coturn.yaml}
status=0

# The coturn/coturn tag a file runs.
tag() {
  sed -n 's/^[[:space:]]*image:[[:space:]]*coturn\/coturn:\([^[:space:]]*\).*$/\1/p' "$1" | head -n 1
}

# The number an option is set to: `--name=10`, or the default of `--name=${VAR:-10}`.
option() {
  sed -n "s/^[[:space:]]*-[[:space:]]*--$2=\\(\\\${[A-Za-z_]*:-\\)\\{0,1\\}\\([0-9][0-9]*\\)}\\{0,1\\}[[:space:]]*\$/\\2/p" "$1" | head -n 1
}

compose_tag=$(tag "$compose")
k8s_tag=$(tag "$k8s")
if [ -z "$compose_tag" ] || [ -z "$k8s_tag" ]; then
  echo "no coturn/coturn image in $compose or $k8s" >&2
  status=1
elif [ "$compose_tag" != "$k8s_tag" ]; then
  echo "coturn versions differ: $compose runs $compose_tag, $k8s runs $k8s_tag" >&2
  status=1
fi

for file in "$compose" "$k8s"; do
  user=$(option "$file" user-quota)
  total=$(option "$file" total-quota)
  if [ -z "$user" ] || [ -z "$total" ]; then
    echo "$file: no numeric --user-quota and --total-quota" >&2
    status=1
  elif [ "$user" -ge "$total" ]; then
    echo "$file: --user-quota=$user is not below --total-quota=$total" >&2
    status=1
  fi
done

[ "$status" -ne 0 ] || echo "ok       coturn $k8s_tag in both; quotas below the total"
exit "$status"
