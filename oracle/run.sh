#!/bin/sh
# Cross-check a list of .x_t files: native digests, Parasolid digests (under
# Wine, resumed after any crash), then the comparison.
#
#   oracle/run.sh LIST RUN_DIR
#
# Writes RUN_DIR/{native,oracle}.ndjson, RUN_DIR/result.tsv and prints the summary.
# A file that crashes the Parasolid process is recorded as an oracle failure
# at stage "crash" (oracle silent, never a pass).
set -eu
list=$1
run=$2
here=$(cd "$(dirname "$0")" && pwd)
lock="$here/../../cadabra3/scripts/cargo-locked"
mkdir -p "$run"
native="$run/native.ndjson"
oracle="$run/oracle.ndjson"

"$here/target/release/xt-native-dump" "$list" "$native" --threads "${THREADS:-4}"

: >>"$oracle"
while :; do
  python3 -I - "$list" "$oracle" "$run/remaining.list" <<'PY'
import json, sys
done = set()
for line in open(sys.argv[2]):
    if line.strip():
        done.add(json.loads(line)["path"])
rest = [p.rstrip("\n") for p in open(sys.argv[1]) if p.strip() and p.rstrip("\n") not in done]
open(sys.argv[3], "w").write("".join(p + "\n" for p in rest))
print(len(rest))
PY
  [ -s "$run/remaining.list" ] || break
  before=$(wc -l <"$oracle")
  (cd "$here" && "$lock" run --release --target x86_64-pc-windows-gnu -p xt-pk-dump -- \
    "$run/remaining.list" "$oracle") || true
  after=$(wc -l <"$oracle")
  if [ "$after" -eq "$before" ] || [ "$(wc -l <"$run/remaining.list")" -gt $((after - before)) ]; then
    # The process died on the first file it had not yet recorded.
    first=$(sed -n "$((after - before + 1))p" "$run/remaining.list")
    python3 -I - "$first" >>"$oracle" <<'PY'
import json, sys
print(json.dumps({"path": sys.argv[1], "outcome": {"status": "failed", "stage": "crash", "message": "Parasolid process exited without finishing this file"}}))
PY
  fi
done
"$here/target/release/xt-xcheck-compare" "$native" "$oracle" "$run/result.tsv"
