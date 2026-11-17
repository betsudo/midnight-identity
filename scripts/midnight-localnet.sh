#!/usr/bin/env bash
# Repo-owned lifecycle wrapper. Mutating commands are reserved for the
# designated midnight-network stack owner; all other sessions use `attach`.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
compose_file="$repo_root/infra/standalone.yml"
credentials="$repo_root/infra/.midnight-indexer.env"
project=midnight-network

usage() {
  printf 'usage: %s {attach|discover|config|up|status|down}\n' "${0##*/}" >&2
  exit 2
}

require_credentials() {
  [[ -f $credentials ]] || {
    printf 'copy infra/standalone.env.example to infra/.midnight-indexer.env and replace every value\n' >&2
    exit 1
  }
}

require_owner() {
  [[ ${MIDNIGHT_LOCALNET_OWNER:-} == 1 ]] || {
    printf 'set MIDNIGHT_LOCALNET_OWNER=1 only in the designated stack-owner session\n' >&2
    exit 1
  }
}

action=${1:-attach}
case "$action" in
  attach|discover)
    exec "$repo_root/scripts/midnight-attach.sh"
    ;;
  config)
    require_credentials
    exec docker compose -p "$project" -f "$compose_file" config --quiet
    ;;
  up)
    require_owner
    if "$repo_root/scripts/midnight-attach.sh"; then
      printf 'compatible midnight-network stack already running; Compose was not invoked\n' >&2
      exit 0
    fi
    for url in \
      http://127.0.0.1:9944 \
      http://127.0.0.1:8088/api/v4/graphql \
      http://127.0.0.1:6300/version; do
      if curl --noproxy '*' --silent --show-error --max-time 1 "$url" >/dev/null 2>&1; then
        printf 'an incompatible or not-ready Midnight service already occupies the shared endpoints; refusing Compose up\n' >&2
        exit 1
      fi
    done
    require_credentials
    exec docker compose -p "$project" -f "$compose_file" up -d
    ;;
  status)
    exec docker compose -p "$project" -f "$compose_file" ps
    ;;
  down)
    require_owner
    exec docker compose -p "$project" -f "$compose_file" down
    ;;
  *) usage ;;
esac
