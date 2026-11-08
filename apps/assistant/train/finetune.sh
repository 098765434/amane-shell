#!/usr/bin/env bash
# Fine-tunes Needle 3 on the assistant's tools, on this machine's CPU, and
# scores the result. Everything stays here: the data is made by gen.py, not
# Cactus's OpenRouter generator, and telemetry is off.
#
#   apps/assistant/train/finetune.sh WORK_DIR [EXAMPLES] [EPOCHS]
#
# WORK_DIR gets a Python venv with cactus-compute[train] (JAX on the CPU),
# the base checkpoint and tokenizer at the commit the shipped needle3.cact
# comes from, the data, the adapter and `needle3.cact`, the tuned archive.
# Put that next to whistle.cact in a folder and point EMAKI_ASSISTANT_MODELS
# at it (or copy both to ~/.local/share/emaki-assistant/models).
#
# Run from the shell repo, in `nix develop` (for the daemon's build). On
# NixOS the venv's wheels need libstdc++ and zlib, from the flake's nixpkgs.
set -euo pipefail

work=${1:?usage: finetune.sh WORK_DIR [EXAMPLES] [EPOCHS]}
examples=${2:-2000}
epochs=${3:-3}
repo=$(git rev-parse --show-toplevel)
needle=https://huggingface.co/Cactus-Compute/needle3/resolve/c95d25b4efa1e9601e785d92438e4ade0dbddbf8
whistle=https://huggingface.co/Cactus-Compute/whistle/resolve/ad73f1468dd5203791e0cc81d31544c553563645

export DO_NOT_TRACK=1 CACTUS_TELEMETRY=0 NEEDLE_TELEMETRY=0 HF_HUB_OFFLINE=1 PYTHONUNBUFFERED=1
mkdir -p "$work"
work=$(cd "$work" && pwd)

if [ ! -x "$work/venv/bin/cactus" ]; then
    nix shell --inputs-from "$repo" nixpkgs#python312 -c python3 -m venv "$work/venv"
    "$work/venv/bin/pip" install -q --disable-pip-version-check "cactus-compute[train]==4.0.1"
fi
LD_LIBRARY_PATH=$(nix build --inputs-from "$repo" nixpkgs#stdenv.cc.cc.lib nixpkgs#zlib \
    --no-link --print-out-paths | sed 's|$|/lib|' | paste -sd:)
export LD_LIBRARY_PATH

# the tokenizer goes where the package looks for it
model=$("$work/venv/bin/python" -c 'import cactus.model, os; print(os.path.dirname(cactus.model.__file__))')
for file in tokenizer.model tokenizer.vocab; do
    [ -f "$model/$file" ] || curl -sfL -o "$model/$file" "$needle/tokenizer/$file"
done
[ -f "$work/needle3.safetensors" ] || curl -sfL -o "$work/needle3.safetensors" "$needle/checkpoints/needle3.safetensors"
[ -f "$work/whistle.cact" ] || curl -sfL -o "$work/whistle.cact" "$whistle/whistle.cact"

cargo build --release -p emaki-assistant-daemon
daemon="$repo/target/release/emaki-assistant"
python3 -I "$repo/apps/assistant/train/gen.py" "$daemon" "$work/data.jsonl" "$examples"

cd "$work"
# 512 tokens a prompt at most (gen.py keeps them under), which trains about
# twice as fast as the 1024 the bucketing would round up to
"$work/venv/bin/cactus" finetune data.jsonl --checkpoint needle3.safetensors \
    --epochs "$epochs" --batch-size 16 --max-len 512 --out adapter.safetensors \
    --checkpoint-dir checkpoints
"$work/venv/bin/cactus" build needle3.safetensors --lora adapter.safetensors --out needle3.cact

echo "--- tuned, Needle alone"
EMAKI_ASSISTANT_NO_RULES=1 EMAKI_ASSISTANT_MODELS="$work" XDG_CACHE_HOME="$work/cache" \
    "$daemon" eval "$repo/apps/assistant/eval/cases.jsonl" "$repo/apps/assistant/eval/laptop.json" | tail -1
echo "--- tuned, with the rules"
EMAKI_ASSISTANT_MODELS="$work" XDG_CACHE_HOME="$work/cache" \
    "$daemon" eval "$repo/apps/assistant/eval/cases.jsonl" "$repo/apps/assistant/eval/laptop.json" | tail -1
