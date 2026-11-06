#!/usr/bin/env bash
# Fine-tunes Needle 3 on another machine's NVIDIA GPU over Tailscale SSH, and
# watches it. The data is made here (gen.py) and sent; the tuned archive comes
# back and is scored here, by the daemon's own engine. Nothing goes to
# Cactus's or anyone else's servers: telemetry is off and the run is offline.
#
#   remote.sh setup                      venv (JAX on CUDA), base model, tokenizer
#   remote.sh run NAME DATA [FLAGS...]   send DATA, train, build NAME's archive
#   remote.sh watch NAME [--plain]       the run in the bar (or the terminal)
#   remote.sh status NAME                running, done or failed
#   remote.sh stop NAME                  stops it
#   remote.sh fetch NAME DIR             the tuned needle3.cact and log, into DIR
#   remote.sh score NAME CASES           fetch, score it on CASES (the dev set) with
#                                        the daemon's engine, and add that to the log
#   remote.sh paraphrase BANKS OUT [FLAGS...]
#                                        grow gen.py's phrase lists (gen.py --banks)
#                                        with a local LLM on the GPU, into OUT
#
# FLAGS go to `cactus finetune` (--epochs, --lr, --lora-rank, --lora-alpha,
# --seed, ...). NEEDLE_HOST picks the machine (default: debian); runs live in
# ~/needle-ft/runs/NAME there, each a systemd user unit (needle-ft-NAME), so
# a dropped connection doesn't stop it.
#
# Scoring runs here: NEEDLE_WORK keeps the fetched runs (default
# ~/.cache/needle-ft), AMANE_ASSISTANT_BIN is a daemon built with Cactus
# (default: the repo's target/release), WHISTLE the whistle.cact to load
# beside the tuned model (default: the installed one, else one fetched into
# NEEDLE_WORK at the commit flake.nix pins).
#
# Paraphrasing runs vLLM in a venv of its own (~/needle-ft/para) on
# PARAPHRASE_MODEL (default Qwen/Qwen3-8B-AWQ), downloaded once; it's a batch
# job, no server and no port, offline once the weights are there.
set -euo pipefail

host=${NEEDLE_HOST:-debian}
here=$(cd "$(dirname "$0")" && pwd)
# the commit the shipped needle3.cact comes from (flake.nix)
needle=https://huggingface.co/Cactus-Compute/needle3/resolve/c95d25b4efa1e9601e785d92438e4ade0dbddbf8
whistle_url=https://huggingface.co/Cactus-Compute/whistle/resolve/ad73f1468dd5203791e0cc81d31544c553563645/whistle.cact
cactus=4.0.1
jax=0.12.0.1

remote() { tailscale ssh "$host" -- "$@"; }
# copies over the same SSH: rsync runs `tailscale ssh HOST rsync --server ...`
send() { rsync -az -e "tailscale ssh" "$@"; }

# the watcher, as a user unit here, when a bar is there to show it
start_watcher() {
    busctl --user status org.amane.StatusBar1 >/dev/null 2>&1 || return 0
    local repo installed; repo=$(cd "$here/../../.." && pwd)
    local -a command
    if installed=$(command -v amane-train-watch); then
        command=("$installed")
    elif [ -x "$repo/target/debug/amane-train-watch" ]; then
        # a build in the repo needs the dev shell's libraries
        command=(nix develop "$repo" -c "$repo/target/debug/amane-train-watch")
    else
        return 0
    fi
    systemctl --user stop "train-watch-$1" 2>/dev/null || true
    systemd-run --user --quiet --collect --unit "train-watch-$1" --setenv=PATH="$PATH" \
        "${command[@]}" "$host" "$1" && echo "watching it in the bar (unit train-watch-$1)"
}

usage() { sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
name_ok() { [[ $1 =~ ^[a-z0-9][a-z0-9._-]*$ ]] || { echo "run names are lowercase letters, digits, . _ -" >&2; exit 2; }; }

case ${1:-} in
setup)
    remote bash -s -- "$needle" "$cactus" "$jax" <<'EOF'
set -euo pipefail
needle=$1 cactus=$2 jax=$3
mkdir -p ~/needle-ft/runs && cd ~/needle-ft
# the pinned JAX needs Python 3.12; uv brings one into ~/needle-ft, nothing system-wide
if [ ! -x venv/bin/python3.12 ]; then
    rm -rf venv
    [ -x uv/bin/uv ] || { python3 -m venv uv && uv/bin/pip install -q --disable-pip-version-check uv; }
    UV_PYTHON_INSTALL_DIR=~/needle-ft/python uv/bin/uv venv -q --seed -p 3.12 venv
fi
venv/bin/pip install -q --disable-pip-version-check "cactus-compute[train]==$cactus" "jax[cuda12]==$jax"
model=$(venv/bin/python -c 'import cactus.model, os; print(os.path.dirname(cactus.model.__file__))')
for file in tokenizer.model tokenizer.vocab; do
    [ -s "$model/$file" ] || curl -sfL -o "$model/$file" "$needle/tokenizer/$file"
done
[ -s needle3.safetensors ] || curl -sfL -o needle3.safetensors "$needle/checkpoints/needle3.safetensors"
[ -s needle3.cact ] || curl -sfL -o needle3.cact "$needle/needle3.cact"
venv/bin/python -c 'import jax; print("jax", jax.__version__, jax.default_backend(), jax.devices())'
EOF
    ;;
run)
    name=${2:?usage: remote.sh run NAME DATA [FLAGS...]}; data=${3:?usage: remote.sh run NAME DATA [FLAGS...]}
    name_ok "$name"; shift 3
    remote mkdir -p "needle-ft/runs/$name"
    send "$data" "$host:needle-ft/runs/$name/data.jsonl"
    # the flags, quoted for the remote shell
    flags=$([ $# -eq 0 ] || printf ' %q' "$@")
    # ssh joins its arguments into one line for the remote shell: quote them for it
    remote "bash -s -- $(printf %q "$name") $(printf %q "$flags")" <<'EOF'
set -euo pipefail
name=$1 flags=$2
dir=~/needle-ft/runs/$name
rm -f "$dir/done" "$dir/train.log"
cat >"$dir/job.sh" <<JOB
#!/usr/bin/env bash
set -o pipefail
cd "$dir"
export DO_NOT_TRACK=1 CACTUS_TELEMETRY=0 NEEDLE_TELEMETRY=0 HF_HUB_OFFLINE=1 PYTHONUNBUFFERED=1
export CACTUS_NEEDLE_WEIGHTS=\$HOME/needle-ft/needle3.cact
# JAX takes 75% of the card up front otherwise
export XLA_PYTHON_CLIENT_PREALLOCATE=false
stamp() { while IFS= read -r line; do echo "\$(date +%H:%M:%S) \$line"; done; }
{
    echo "  run       $name $flags"
    ~/needle-ft/venv/bin/cactus finetune data.jsonl --checkpoint ~/needle-ft/needle3.safetensors \
        --max-len 512 --out adapter.safetensors --checkpoint-dir checkpoints $flags &&
    ~/needle-ft/venv/bin/cactus build ~/needle-ft/needle3.safetensors --lora adapter.safetensors --out needle3.cact
} 2>&1 | stamp >>train.log
status=\$?
echo "\$(date +%H:%M:%S)   finished  \$status" >>train.log
echo \$status >done
JOB
chmod +x "$dir/job.sh"
systemctl --user reset-failed "needle-ft-$name" 2>/dev/null || true
systemd-run --user --quiet --unit "needle-ft-$name" --collect "$dir/job.sh"
echo "started needle-ft-$name"
EOF
    start_watcher "$name"
    ;;
watch)
    name=${2:?usage: remote.sh watch NAME [--plain]}; name_ok "$name"
    watcher=$(command -v amane-train-watch || echo "$here/../../../target/debug/amane-train-watch")
    [ -x "$watcher" ] || { echo "no amane-train-watch: cargo build -p amane-train-watch" >&2; exit 1; }
    exec "$watcher" "$host" "$name" ${3:+"$3"}
    ;;
status)
    name=${2:?usage: remote.sh status NAME}; name_ok "$name"
    # one line, as ssh hands it to the remote shell
    remote "cat needle-ft/runs/$name/done 2>/dev/null || systemctl --user is-active needle-ft-$name"
    ;;
stop)
    name=${2:?usage: remote.sh stop NAME}; name_ok "$name"
    remote systemctl --user stop "needle-ft-$name"
    ;;
fetch)
    name=${2:?usage: remote.sh fetch NAME DIR}; dir=${3:?usage: remote.sh fetch NAME DIR}; name_ok "$name"
    mkdir -p "$dir"
    send "$host:needle-ft/runs/$name/needle3.cact" "$host:needle-ft/runs/$name/train.log" "$dir/"
    ;;
score)
    name=${2:?usage: remote.sh score NAME CASES}; cases=${3:?usage: remote.sh score NAME CASES}; name_ok "$name"
    repo=$(cd "$here/../../.." && pwd)
    bin=${AMANE_ASSISTANT_BIN:-$repo/target/release/amane-assistant}
    work=${NEEDLE_WORK:-$HOME/.cache/needle-ft}
    dir=$work/$name
    whistle=${WHISTLE:-}
    if [ -z "$whistle" ] && installed=$(command -v amane-assistant); then
        whistle=$(dirname "$(readlink -f "$installed")")/../share/amane-assistant/models/whistle.cact
    fi
    if [ ! -s "${whistle:-/nonexistent}" ]; then
        whistle=$work/whistle.cact
        [ -s "$whistle" ] || { mkdir -p "$work"; curl -sfL -o "$whistle" "$whistle_url"; }
    fi
    "$0" fetch "$name" "$dir"
    [ -e "$dir/whistle.cact" ] || ln -s "$(readlink -f "$whistle")" "$dir/whistle.cact"
    # offline, as the daemon runs: no network in its namespace
    score=$(AMANE_ASSISTANT_MODELS="$dir" XDG_CACHE_HOME="$dir/cache" CACTUS_TELEMETRY=0 DO_NOT_TRACK=1 \
        unshare -rn "$bin" eval --json "$cases" "$repo/apps/assistant/eval/laptop.json")
    echo "$score" >"$dir/score.json"
    line=$(python3 -c 'import json, sys; print("{right}/{commands} right, {wrong} wrong ({destructive_wrong} destructive), negatives {clean}/{negatives}".format(**json.loads(sys.argv[1])))' "$score")
    echo "$name: $line"
    remote "echo \"\$(date +%H:%M:%S)   dev       $line\" >>needle-ft/runs/$name/train.log"
    ;;
paraphrase)
    banks=${2:?usage: remote.sh paraphrase BANKS OUT [FLAGS...]}; out=${3:?usage: remote.sh paraphrase BANKS OUT [FLAGS...]}
    shift 3
    model=${PARAPHRASE_MODEL:-Qwen/Qwen3-8B-AWQ}
    remote mkdir -p needle-ft/para needle-ft/models
    send "$banks" "$host:needle-ft/para/banks.json"
    send "$here/paraphrase.py" "$host:needle-ft/para/paraphrase.py"
    flags=$([ $# -eq 0 ] || printf ' %q' "$@")
    remote "bash -s -- $(printf %q "$model") $(printf %q "$flags")" <<'EOF'
set -euo pipefail
model=$1 flags=$2
cd ~/needle-ft
[ -x para/venv/bin/python ] || UV_PYTHON_INSTALL_DIR=~/needle-ft/python uv/bin/uv venv -q --seed -p 3.12 para/venv
# vLLM builds a few kernels with ninja when it starts
[ -x para/venv/bin/ninja ] || VIRTUAL_ENV=~/needle-ft/para/venv uv/bin/uv pip install -q vllm ninja
dir=models/${model##*/}
[ -s "$dir/config.json" ] || DO_NOT_TRACK=1 HF_HUB_DISABLE_TELEMETRY=1 para/venv/bin/hf download "$model" --local-dir "$dir" >/dev/null
rm -f para/out.json para/done
systemctl --user reset-failed needle-para 2>/dev/null || true
# shellcheck disable=SC2086 # the flags are words
systemd-run --user --quiet --collect --unit needle-para --working-directory="$HOME/needle-ft/para" \
    --setenv=PATH="$HOME/needle-ft/para/venv/bin:$PATH" --setenv=HF_HUB_OFFLINE=1 --setenv=DO_NOT_TRACK=1 --setenv=VLLM_NO_USAGE_STATS=1 \
    bash -c "venv/bin/python -I paraphrase.py ../$dir banks.json out.json $flags >log 2>&1; echo \$? >done"
EOF
    echo "paraphrasing on $host (unit needle-para, log ~/needle-ft/para/log)"
    until status=$(remote "cat needle-ft/para/done 2>/dev/null") && [[ $status =~ ^[0-9]+$ ]]; do
        sleep 30
    done
    remote "grep -v '^INFO\|^WARNING\|^(\|^\[' needle-ft/para/log | tail -n 5"
    [ "$status" = 0 ] || { echo "paraphrasing failed ($status)" >&2; exit 1; }
    send "$host:needle-ft/para/out.json" "$out"
    echo "-> $out"
    ;;
*) usage ;;
esac
