#!/usr/bin/env bash
# Trains each configuration in turn on the GPU machine (remote.sh), scores
# each on the dev set, and ranks them: right − 3 × wrong, and a run that got a
# Destructive command wrong is out. The winner is then scored on the eval
# set, the test, once, by hand.
#
#   sweep.sh DATA DEV [PREFIX] [CONFIG...]
#
# DATA and DEV are gen.py's (OUT.jsonl and --dev). A CONFIG is "NAME FLAGS",
# FLAGS for `cactus finetune`; without any, the default grid below. Runs are
# PREFIX-NAME (default prefix: sw). Each shows in the bar while it trains.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
data=${1:?usage: sweep.sh DATA DEV [PREFIX] [CONFIG...]}
dev=${2:?usage: sweep.sh DATA DEV [PREFIX] [CONFIG...]}
prefix=${3:-sw}
shift $(($# < 3 ? $# : 3))
configs=("$@")
if [ ${#configs[@]} -eq 0 ]; then
    configs=(
        "r16-lr1e4 --lora-rank 16 --lora-alpha 32 --lr 1e-4 --epochs 3"
        "r16-lr3e4 --lora-rank 16 --lora-alpha 32 --lr 3e-4 --epochs 3"
        "r64-lr1e4 --lora-rank 64 --lora-alpha 128 --lr 1e-4 --epochs 3"
        "r64-lr3e4 --lora-rank 64 --lora-alpha 128 --lr 3e-4 --epochs 3"
    )
fi
work=${NEEDLE_WORK:-$HOME/.cache/needle-ft}

runs=()
for config in "${configs[@]}"; do
    read -r name flags <<<"$config"
    run=$prefix-$name
    runs+=("$run")
    # shellcheck disable=SC2086 # the flags are words
    "$here/remote.sh" run "$run" "$data" $flags --batch-size 16 --val-split 0.02
    until status=$("$here/remote.sh" status "$run" 2>/dev/null) && [[ $status =~ ^[0-9]+$ ]]; do
        sleep 60
    done
    if [ "$status" != 0 ]; then
        echo "$run failed ($status)"
        continue
    fi
    "$here/remote.sh" score "$run" "$dev"
done

echo
python3 -I - "$work" "${runs[@]}" <<'EOF'
import json, os, sys
work, runs = sys.argv[1], sys.argv[2:]
rows = []
for run in runs:
    path = os.path.join(work, run, "score.json")
    if not os.path.exists(path):
        continue
    s = json.load(open(path))
    out = s["destructive_wrong"] > 0
    rows.append((-1e9 if out else s["right"] - 3 * s["wrong"], run, s, out))
for value, run, s, out in sorted(rows, reverse=True):
    print(f"{run:24} {'OUT' if out else int(value):>5}  right {s['right']}/{s['commands']}  "
          f"wrong {s['wrong']}  negatives {s['clean']}/{s['negatives']}  p95 {s['p95_ms']:.0f} ms")
EOF
