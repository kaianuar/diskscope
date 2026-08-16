#!/usr/bin/env bash
# GATE 1 — hard test gate. Runs the project's OWN test suite(s) by auto-detecting
# the stack. There is NO manual test-command here — omp leaves the choice to this
# script, which detects the stack and runs the matching command(s).
# For a full-stack project this runs BOTH frontend and backend test commands.
#
# Exit code 0 = all detected suites passed (proceed).
# Non-zero    = at least one suite failed (halt; builder must fix, then re-run).
set -uo pipefail
cd "$(dirname "$0")/.."

# Optional explicit override — if the project sets this outside AUTO, use it.
# Otherwise auto-detect below. LEAVE UNSET for auto-detect.
#   GATE_TEST_CMD="npm test && cd client && npm test"

RUN=0          # number of suites actually run
FAILED=0       # number that failed

run_cmd() {
  local label="$1"; shift
  echo "==> [gate] ${label}: $*"
  ( eval "$*" ) || { echo "xx [gate] FAILED -> ${label}: $*"; FAILED=$((FAILED+1)); }
  RUN=$((RUN+1))
}

# --- Auto-detect: Node (often has workspaces / client+server) ---
if [ -f package.json ]; then
  # If this is a monorepo/workspace, run the test script at the root (covers all workspaces)
  if command -v node >/dev/null 2>&1 && grep -q '"workspaces"' package.json 2>/dev/null; then
    run_cmd "npm-workspaces" "npm test --workspaces --if-present"
  elif grep -q '"test"' package.json 2>/dev/null; then
    run_cmd "npm-test" "npm test --if-present"
  fi
fi

# --- Sub-package.json under client/ and/or server/ (full-stack monorepo) ---
for sub in client server front backend api; do
  if [ -f "$sub/package.json" ] && grep -q '"test"' "$sub/package.json" 2>/dev/null; then
    run_cmd "$sub-test" "cd '$sub' && npm test --if-present && cd .."
  fi
done

# --- Python ---
if [ -f pyproject.toml ]; then
  run_cmd "pytest" "python -m pytest -q"
elif [ -f requirements.txt ] || [ -f setup.py ] || [ -f setup.cfg ]; then
  run_cmd "pytest" "python -m pytest -q"
fi

# --- Go ---
if [ -f go.mod ]; then
  run_cmd "go-test" "go test ./..."
fi

# --- Rust ---
if [ -f Cargo.toml ]; then
  run_cmd "cargo-test" "cargo test"
fi

echo ""
echo "==> Gate result: ${RUN} suite(s) run, ${FAILED} failed."

if [ "$FAILED" -gt 0 ]; then
  echo "xx GATE 1 FAILED — halt, fix tests, re-run."
  exit 1
fi
if [ "$RUN" -eq 0 ]; then
  echo "!! No tests detected/configured. Add tests, or this gate is vacuous."
  exit 1   # fail-closed: an empty test gate is not acceptable
fi
echo "==> GATE 1 passed (${RUN} suite(s) green)."
exit 0
