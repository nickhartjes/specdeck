#!/usr/bin/env bash
# Records a real run of the Docker Compose guide for the self-hosting video (design decision 3):
# in a temporary copy of deploy/compose, on this machine's Docker, with Caddy's local certificate
# for localhost. Saves each step's command and output to run/, with the TURN secret and every
# address other than 127.0.0.1 and the container network left out, then removes it all again.
# Run by hand, not by `just video`: DOCKER_CONTEXT=<a Docker with 80, 443 and 3478 free> ./record.sh
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)
secret=$(openssl rand -hex 32)
# Under the home folder, which Colima and Docker Desktop share with their VM; /tmp they do not.
work=$(mktemp -d "$here/.work.XXXXXX")
trap 'cd "$work" && docker compose down -v >/dev/null 2>&1; rm -rf "$work"' EXIT

cp "$repo/deploy/compose/compose.yaml" "$repo/deploy/compose/Caddyfile" "$work/"
cd "$work"
# The settings compose.yaml needs; a local run, so both domains are this machine.
cat > .env <<ENV
SPECDECK_VERSION=$version
SPECDECK_DOMAIN=localhost
TURN_DOMAIN=127.0.0.1
TURN_EXTERNAL_IP=127.0.0.1
TURN_SECRET=$secret
ENV

rm -rf "$here/run"
mkdir -p "$here/run"
# Without colours; the secret as •••, and any IPv4 address but 0.0.0.0, 127.0.0.1 and Docker's
# 172.16/12 as •.•.•.•.
redact() {
  sed -E -e $'s/\x1b\\[[0-9;]*m//g' -e "s/$secret/•••/g" \
    -e '/(^|[^0-9.])(0\.0\.0\.0|127\.0\.0\.1|172\.(1[6-9]|2[0-9]|3[01])\.[0-9]+\.[0-9]+)([^0-9.]|$)/!s/(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}([^0-9.]|$)/\1•.•.•.•\3/g'
}
# step <file> <command>: runs the command as shown, and saves it with its output.
step() {
  local file=$1
  shift
  { printf '$ %s\n' "$*"; bash -c "$*" 2>&1 | redact; } > "$here/run/$file.txt"
  # A command such as curl ends without a newline.
  [ -z "$(tail -c 1 "$here/run/$file.txt")" ] || echo >> "$here/run/$file.txt"
  cat "$here/run/$file.txt"
}

step 1-env 'cat .env'
step 2-up 'docker compose up -d'
# Caddy makes its local certificate authority on its first request; trust it, as a browser would.
sleep 5
curl -ksS https://localhost/health >/dev/null || true
docker compose exec -T caddy cat /data/caddy/pki/authorities/local/root.crt > caddy-root.crt
step 3-ps 'docker compose ps --format "table {{.Service}}\t{{.Image}}\t{{.Status}}"'
step 4-health 'curl -sS --cacert caddy-root.crt https://localhost/health'
step 5-version 'curl -sS --cacert caddy-root.crt https://localhost/web-viewer.json'
step 6-logs 'docker compose logs server --no-log-prefix --tail 6'
