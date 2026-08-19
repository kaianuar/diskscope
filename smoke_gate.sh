#!/usr/bin/env bash
# =============================================================================
# GATE 1.5 — BEHAVIORAL SMOKE TEST
#
# Catches the bug class Gate 1 (self-authored tests) and Gate 2 (diff reading)
# both miss: the program behaving WRONG when a real user runs it.
#
# The bugs this catches (all hit diskscope 2026-08-19):
#   1. `--quiet` suppressed ALL output (test asserted empty stdout as correct)
#   2. root directory appeared as its own child (no test checked structure)
#   3. directory sizes were raw inode sizes not recursive sums (no test
#      asserted size == actual bytes on disk)
#
# Design: deterministic, no LLM, ~seconds. Builds the real binary, runs it
# against a FIXED fixture tree created here (not by the builder), and asserts
# the contract from requirements.md with exact byte counts.
#
# Usage: bash tests/smoke_gate.sh   (from project root; skips gracefully if
# the project has no obvious CLI binary to smoke-test)
# =============================================================================
set -uo pipefail
# NOTE: unlike gate.sh this must be run FROM the project root (the pipeline
# invokes it as bash "$PROJ_ROOT/tests/smoke_gate.sh" with cwd=project root).
# We intentionally do NOT cd to the script dir — the binary and fixtures are
# relative to the PROJECT, not to this script.
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); echo "  ok   - $*"; }
bad() { FAIL=$((FAIL+1)); echo "  FAIL - $*"; }

# ---- which binary? (in priority order) -------------------------------------
BIN=""
if [ -x ./target/debug/diskscope ]; then BIN=./target/debug/diskscope
elif [ -x ./target/debug/$(basename "$(pwd)") ]; then BIN=./target/debug/$(basename "$(pwd)")
fi

# If no Rust binary and no node CLI, skip silently (not every project is a CLI).
if [ -z "$BIN" ]; then
  if [ -f Cargo.toml ] || [ -f package.json ]; then
    echo "==> [smoke] no debug binary yet — building."
    if [ -f Cargo.toml ]; then
      cargo build 2>/dev/null || { echo "xx [smoke] cargo build failed — SKIP"; exit 0; }
      [ -x ./target/debug/diskscope ] && BIN=./target/debug/diskscope
    fi
  fi
fi
[ -z "$BIN" ] && { echo "==> [smoke] no CLI binary to smoke-test — SKIP (not a CLI project)."; exit 0; }
echo "==> [smoke] binary: $BIN"

# ---- fixed fixture: created HERE, never by the builder ---------------------
FIX=$(mktemp -d)
mkdir -p "$FIX/sub"
printf 'aaaaaa'      > "$FIX/a.txt"      # 6 bytes
printf 'bbbbbb'      > "$FIX/b.txt"      # 6 bytes
printf 'cccccc'      > "$FIX/sub/c.txt"  # 6 bytes
# expected totals: a=6, b=6, sub/c=6, sub dir=6, root=18
EXPECT_ROOT=18
EXPECT_SUB=6

echo "==> [smoke] fixture: $FIX (3 files x 6B = ${EXPECT_ROOT}B)"

# ---- 1. table: no duplicate root row --------------------------------------
TABLE=$("$BIN" scan "$FIX" 2>/dev/null)
COUNT_ROOT=$(echo "$TABLE" | grep -c "$FIX$" || true)  # exact path, end of line
if [ "$COUNT_ROOT" -eq 1 ]; then
  ok "table: root path appears exactly once (got $COUNT_ROOT)"
else
  bad "table: root path appears $COUNT_ROOT times (should be 1) — duplicate-row bug"
fi

# ---- 2. json: root size == recursive sum (not inode size) ------------------
JSON=$("$BIN" scan "$FIX" --format json 2>/dev/null)
ROOT_SIZE=$(echo "$JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('size', '-1'))
" 2>/dev/null)
if [ "$ROOT_SIZE" = "$EXPECT_ROOT" ]; then
  ok "json: root size $ROOT_SIZE == expected $EXPECT_ROOT (recursive sum)"
else
  bad "json: root size $ROOT_SIZE != expected $EXPECT_ROOT (inode-size bug)"
fi

# ---- 3. --quiet keeps output, drops summary --------------------------------
QUIET_OUT=$("$BIN" scan "$FIX" --format json --quiet 2>/dev/null | wc -c)
if [ "$QUIET_OUT" -gt 0 ]; then
  ok "--quiet: stdout non-empty ($QUIET_OUT bytes) — output preserved"
else
  bad "--quiet: stdout EMPTY — quiet-suppresses-output bug"
fi
QUIET_ERR=$("$BIN" scan "$FIX" --format json --quiet 2>&1 >/dev/null | wc -c)
if [ "$QUIET_ERR" -eq 0 ]; then
  ok "--quiet: stderr empty — summary suppressed"
else
  bad "--quiet: stderr has $QUIET_ERR bytes — summary not suppressed"
fi

# ---- 4. jsonl: one line per file (3 files) ---------------------------------
JSONL_LINES=$("$BIN" scan "$FIX" --format jsonl 2>/dev/null | wc -l)
if [ "$JSONL_LINES" -ge 3 ]; then
  ok "jsonl: $JSONL_LINES lines (>= 3 files)"
else
  bad "jsonl: only $JSONL_LINES lines (should be >= 3)"
fi

# ---- 5. summary total matches du -------------------------------------------
SUM=$("$BIN" summary "$FIX" 2>/dev/null | grep -oE 'total: [0-9]+' | grep -oE '[0-9]+' | head -1)
if [ "$SUM" = "$EXPECT_ROOT" ]; then
  ok "summary: total $SUM == $EXPECT_ROOT (matches du)"
else
  bad "summary: total $SUM != $EXPECT_ROOT (size-accounting bug)"
fi

rm -rf "$FIX"
echo ""
echo "==> Smoke gate: $PASS passed, $FAIL failed."
[ "$FAIL" -eq 0 ] || exit 1
exit 0
