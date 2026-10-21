#!/usr/bin/env bash
# Generate the amane layer-rules into the niri config from the single source of
# truth, niri-layer-rule.kdl. Replaces only the region between the markers
#
#     // >>> amane layer-rules (generated from shell/niri-layer-rule.kdl ...)
#     // <<< amane layer-rules
#
# in the target config and leaves everything else untouched, so it is safe to
# re-run after editing niri-layer-rule.kdl. Reload niri afterwards:
#
#     ./gen-layer-rules.sh ~/.config/niri/config.kdl
#     niri validate -c ~/.config/niri/config.kdl
#     niri msg action reload-config
#
# Usage: ./gen-layer-rules.sh [path-to-config.kdl]
#        (default target: ~/.config/niri/config.kdl)
set -euo pipefail
cd "$(dirname "$0")"

SRC="niri-layer-rule.kdl"
TARGET="${1:-$HOME/.config/niri/config.kdl}"
SENTINEL="//<<<GENERATED-RULES-BELOW>>>"
BEGIN="// >>> amane layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)"
END="// <<< amane layer-rules"

[ -f "$SRC" ]    || { echo "error: source $SRC not found" >&2; exit 1; }
[ -f "$TARGET" ] || { echo "error: target $TARGET not found" >&2; exit 1; }

# Everything in the source after the sentinel line, with leading blanks trimmed.
rules="$(awk -v s="$SENTINEL" 'f{print} index($0,s){f=1}' "$SRC" | sed '/./,$!d')"
[ -n "$rules" ] || { echo "error: sentinel '$SENTINEL' not found in $SRC" >&2; exit 1; }

if ! grep -qF "$BEGIN" "$TARGET" || ! grep -qF "$END" "$TARGET"; then
  cat >&2 <<EOF
error: generation markers not found in $TARGET.

One-time setup: replace the hand-written amane layer-rule section of the niri
config with these two marker lines, then re-run this script to fill them:

$BEGIN
$END
EOF
  exit 1
fi

# Replacement block: markers wrapping the generated rules.
block="$(mktemp)"; trap 'rm -f "$block"' EXIT
{ printf '%s\n' "$BEGIN"; printf '%s\n' "$rules"; printf '%s\n' "$END"; } > "$block"

# Splice: replace everything from BEGIN through END (inclusive) with the block.
out="$(mktemp)"
awk -v b="$BEGIN" -v e="$END" -v bf="$block" '
  BEGIN { while ((getline l < bf) > 0) blk = blk l "\n" }
  index($0, b) { printf "%s", blk; inblock = 1; next }
  inblock      { if (index($0, e)) inblock = 0; next }
  { print }
' "$TARGET" > "$out"
mv "$out" "$TARGET"
echo "updated $TARGET (amane layer-rules regenerated from $SRC)"
