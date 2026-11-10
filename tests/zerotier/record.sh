#!/bin/bash
# Records the zerotier-cli captures the ZeroTier specification and the e2e fake
# read, from two real ZeroTier nodes in containers. No ZeroTier account: one
# node's built-in network controller hosts the networks the other joins.
#
#   tests/zerotier/record.sh            record into the core's fixtures
#   tests/zerotier/record.sh --keep     and leave the containers running
#
# Needs docker (rootless works) with /dev/net/tun, and python3. The member's
# public address, which ZeroTier reports in `info`, is replaced with a
# documentation address before `info` reaches the fixtures: the raw answer
# only ever sits in a private temporary file.

set -euo pipefail

here="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
fixtures="$here/../../crates/tailgauge-core/tests/fixtures"
image="zerotier/zerotier:1.14.2"
net="tailgauge-zt"
controller="tailgauge-zt-controller"
member="tailgauge-zt-member"
keep=false
[[ ${1:-} == --keep ]] && keep=true

raw_info=$(mktemp)
teardown() {
  rm -f "$raw_info"
  docker rm -f "$controller" "$member" >/dev/null 2>&1 || true
  docker network rm "$net" >/dev/null 2>&1 || true
}
if $keep; then
  trap 'rm -f "$raw_info"' EXIT
else
  trap teardown EXIT
fi
docker rm -f "$controller" "$member" >/dev/null 2>&1 || true
docker network rm "$net" >/dev/null 2>&1 || true

docker network create "$net" >/dev/null
for node in "$controller" "$member"; do
  docker run -d --name "$node" --network "$net" \
    --cap-add NET_ADMIN --device /dev/net/tun "$image" >/dev/null
done

cli() {
  local node=$1
  shift
  docker exec "$node" zerotier-cli "$@"
}

wait_for() {
  local what=$1
  shift
  for _ in $(seq 60); do
    if "$@"; then
      return 0
    fi
    sleep 1
  done
  echo "record: timed out waiting for $what" >&2
  exit 1
}

online() { cli "$1" info 2>/dev/null | grep -q ONLINE; }
wait_for "the controller" online "$controller"
wait_for "the member" online "$member"

token=$(docker exec "$controller" cat /var/lib/zerotier-one/authtoken.secret)
address=$(cli "$controller" info | awk '{print $3}')
api() {
  docker exec "$controller" curl -fsS -X POST -H "X-ZT1-AUTH: $token" \
    -d "$2" "http://localhost:9993/controller/$1"
}
network_id() { python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])'; }

homelab=$(api "network/${address}______" '{"name":"homelab","private":true,
  "ipAssignmentPools":[{"ipRangeStart":"10.147.20.1","ipRangeEnd":"10.147.20.254"}],
  "routes":[{"target":"10.147.20.0/24"}],"v4AssignMode":{"zt":true}}' | network_id)
office=$(api "network/${address}______" '{"name":"office","private":true,
  "ipAssignmentPools":[{"ipRangeStart":"10.244.0.1","ipRangeEnd":"10.244.0.254"}],
  "routes":[{"target":"10.244.0.0/16"}],"v4AssignMode":{"zt":true},
  "v6AssignMode":{"rfc4193":true}}' | network_id)

cli "$controller" join "$homelab" >/dev/null
cli "$member" join "$homelab" >/dev/null

status_of() {
  cli "$member" -j listnetworks |
    python3 -c 'import json, sys; print(" ".join(n["status"] for n in json.load(sys.stdin)))'
}
denied() { [[ $(status_of) == ACCESS_DENIED ]]; }
wait_for "the member to be refused" denied

capture() { cli "$member" -j "$1" >"$fixtures/$2"; }
capture listnetworks zerotier-listnetworks-denied.json
capture peers zerotier-peers-denied.json

member_address=$(cli "$member" info | awk '{print $3}')
api "network/$homelab/member/$address" '{"authorized":true}' >/dev/null
api "network/$homelab/member/$member_address" '{"authorized":true}' >/dev/null
cli "$member" join "$office" >/dev/null
api "network/$office/member/$member_address" '{"authorized":true}' >/dev/null

both_up() { [[ $(status_of) == "OK OK" ]]; }
wait_for "both networks" both_up
direct() { cli "$member" peers | awk -v a="$address" '$1 == a && $5 == "DIRECT"' | grep -q .; }
wait_for "a direct path to the controller" direct

cli "$member" -j info >"$raw_info"
capture listnetworks zerotier-listnetworks.json
capture peers zerotier-peers.json

python3 - "$raw_info" "$fixtures/zerotier-info.json" <<'EOF'
import json, sys
raw, out = sys.argv[1], sys.argv[2]
info = json.load(open(raw))
settings = info.get("config", {}).get("settings", {})
settings["surfaceAddresses"] = [
    "203.0.113.10/" + a.rsplit("/", 1)[-1] for a in settings.get("surfaceAddresses", [])
]
open(out, "w").write(json.dumps(info, indent=1) + "\n")
EOF

echo "record: wrote the ZeroTier captures into $fixtures"
