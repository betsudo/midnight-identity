#!/usr/bin/env bash
# Read-only compatibility preflight for the designated shared Ledger 9 stack.
set -euo pipefail

node_rpc=http://127.0.0.1:9944
indexer_http=http://127.0.0.1:8088/api/v4/graphql
indexer_ws=ws://127.0.0.1:8088/api/v4/graphql/ws
prover=http://127.0.0.1:6300

fail() {
  printf 'Midnight attach: %s\n' "$1" >&2
  exit 1
}

for tool in curl jq; do
  command -v "$tool" >/dev/null || fail "$tool is required"
done

request() {
  curl --noproxy '*' --fail --silent --show-error --max-time 4 "$@"
}

rpc() {
  local payload
  payload=$(jq -cn --arg method "$1" --argjson params "$2" \
    '{jsonrpc:"2.0",id:1,method:$method,params:$params}')
  request -H 'Content-Type: application/json' --data "$payload" "$node_rpc" \
    | jq -er 'select(.error == null) | .result'
}

version=$(rpc system_version '[]') || fail 'node RPC is unavailable'
[[ $version =~ ^2\.1\.101(-[[:alnum:]._-]+)?$ ]] || fail "unexpected node version: $version"
genesis=$(rpc chain_getBlockHash '[0]') || fail 'node genesis is unavailable'
[[ $genesis =~ ^0x[[:xdigit:]]{64}$ ]] || fail 'invalid genesis hash'

indexed=$(request -H 'Content-Type: application/json' \
  --data '{"query":"{ block { height hash protocolVersion } }"}' "$indexer_http" \
  | jq -ce 'select(.errors == null) | .data.block
    | select(.protocolVersion == 2001100 and (.height | type == "number" and . > 0 and . == floor)
    and (.hash | test("^[0-9a-f]{64}$")))') || fail 'indexer has no current Ledger 9 block'
height=$(jq -r '.height' <<<"$indexed")
hash=$(jq -r '.hash' <<<"$indexed")
node_hash=$(rpc chain_getBlockHash "[$height]") || fail 'node could not resolve the indexer block'
[[ $node_hash == "0x$hash" ]] || fail 'node and indexer disagree at the indexed height'

proof_version=$(request "$prover/version") || fail 'proof server is unavailable'
[[ $proof_version == 9.0.2 ]] || fail "unexpected proof server version: $proof_version"

jq -n --arg node "$node_rpc" --arg indexer "$indexer_http" --arg ws "$indexer_ws" \
  --arg prover "$prover" --arg genesis "$genesis" --arg hash "$node_hash" --argjson height "$height" \
  '{networkId:"undeployed",urls:{nodeRpcUrl:$node,indexerHttpUrl:$indexer,indexerWsUrl:$ws,proofServerUrl:$prover},
    genesisHash:$genesis,indexedBlock:{height:$height,hash:$hash}}'
