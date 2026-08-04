#!/usr/bin/env bash
# Smoke-test edgectl against a cluster that already has inputs and outputs.
#
# Runs via cargo run by default. Set EDGECTL to test a prebuilt binary instead,
# e.g. EDGECTL=./target/debug/edgectl or EDGECTL=edgectl
set -u

if [ -n "${EDGECTL:-}" ]; then
    edgectl=("$EDGECTL")
else
    cd "$(dirname "$0")" || exit 1 # cargo needs to run from the manifest directory
    edgectl=(cargo run --quiet --)
    cargo build --quiet # build once, so build output does not interleave with the test
fi

name="smoke-$$"

set -x
"${edgectl[@]}" input list
"${edgectl[@]}" output list
"${edgectl[@]}" appliance list
"${edgectl[@]}" group list
"${edgectl[@]}" region list
"${edgectl[@]}" output-list list
"${edgectl[@]}" alarm list
"${edgectl[@]}" health
"${edgectl[@]}" build-info
set +x

# Pick the first appliance, and its first interface that is not loopback or cilium plumbing.
appliance=$("${edgectl[@]}" appliance list | awk 'NR > 1 && NF { print $1; exit }')
interface=$("${edgectl[@]}" appliance show "$appliance" |
    awk '/^  - Name: / && $3 != "lo" && $3 !~ /^cilium/ { print $3; exit }')
echo
echo "### using appliance '$appliance' interface '$interface'"

# Clean up whatever we managed to create, even if a step below fails.
trap '
    "${edgectl[@]}" output delete "$name-out" || true
    "${edgectl[@]}" input delete "$name-in" || true
' EXIT

set -x

"${edgectl[@]}" input create "$name-in" --mode generator --appliance "$appliance" --bitrate 5000000
"${edgectl[@]}" input show "$name-in"

"${edgectl[@]}" output create "$name-out" \
    --appliance "$appliance" \
    --mode udp \
    --interface "$interface" \
    --input "$name-in" \
    --dest 198.51.100.10:5000
"${edgectl[@]}" output show "$name-out"
