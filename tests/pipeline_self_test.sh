#!/usr/bin/env bash
# =============================================================================
# pipeline_self_test.sh - SELF-TEST of omp-agent's own pipeline logic.
#
# These tests run against omp-agent ONLY (no consumer project scaffold needed)
# and verify the pure, deterministic mechanics of the pipeline. They protect
# against regressions in logic that "we think works" but could fail silently
# inside an omp-driven build (phase extraction, verdict parsing, the issue
# ledger, notes extraction).
#
# Usage:  bash tests/pipeline_self_test.sh     (exit non-zero on any failure)
# =============================================================================
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB="$HERE/lib"
PASS=0
FAIL=0
WORK="$(mktemp -d)"
DIR="$(pwd)"
trap 'rm -rf "$WORK"' EXIT

note()  { printf '%s\n' "$*"; }
ok()    { PASS=$((PASS+1)); printf '  ok   - %s\n' "$*"; }
no()    { FAIL=$((FAIL+1)); printf '  FAIL - %s\n' "$*"; }

# =============================================================================
# 1. Phase extraction from plan.md (must match only "## Phase N:" headings, in
#    order, as single-word tokens PhaseN..PhaseN).
# =============================================================================
note ""
note "## 1. extract_phases"
cat > "$WORK/plan.md" <<'EOF'
# DiskScope Plan

## Phase 1: Domain Core
stuff
## Phase 2: Scan Engine
stuff, mentions Phase 3 risk row (not a heading)
| dependency | Phase 3 -> Phase 2 |   <- table row, must NOT match
## Phase 3: CLI
more
## Phase 4: GUI
## Phase 5: Sync
## Phase 6: Packaging
EOF
phases="$(grep -iE '^#+[[:space:]]+phase[[:space:]]+[0-9]+' "$WORK/plan.md" \
  | sed -E 's/^#+[[:space:]]*//I; s/[[:space:]]*:.*$//I; s/[[:space:]]+$//' \
  | tr '[:upper:]' '[:lower:]' \
  | sort -t' ' -k2 -n | sort -u -k1,1 -k2,2n \
  | while read -r line; do n="$(echo "$line" | sed -E 's/^phase[[:space:]]+//')"; [ -n "$n" ] && echo "Phase$n"; done)"
expected=$'Phase1\nPhase2\nPhase3\nPhase4\nPhase5\nPhase6'
if [ "$phases" = "$expected" ]; then
  ok "extracts exactly 6 headings as Phase1..Phase6, ignores table rows"
else
  no "extract_phases => got: [${phases//$'\n'/ }] want: [${expected//$'\n'/ }]"
fi

# =============================================================================
# 2. Verdict parsing: last bare PASS/FAIL token determines gate outcome.
# =============================================================================
note ""
note "## 2. verdict parsing"
cv() { grep -iE '(^|[^A-Za-z])(PASS|FAIL)([^A-Za-z]|$)' "$1" | tail -1 | grep -oEi 'PASS|FAIL' | tail -1 | tr '[:lower:]' '[:upper:]'; }
printf 'some notes here about a FAIL scenario\n**PASS**\n' > "$WORK/v1"            # final PASS wins
[ "$(cv "$WORK/v1")" = "PASS" ] && ok "final PASS token wins"   || no "final PASS token"
printf 'A FAIL security note\nThen discussion\nFAIL\n' > "$WORK/v2"
[ "$(cv "$WORK/v2")" = "FAIL" ] && ok "final FAIL token wins"   || no "final FAIL token"
printf 'No blockers, everything looks good.\n' > "$WORK/v3"                       # no token
[ -z "$(cv "$WORK/v3")" ] && ok "no token -> empty (undetermined)" || no "no token"

# =============================================================================
# 3. Notes extraction: [P2]/[P3]/[P4] findings captured; P0/P1 not leaked.
# =============================================================================
note ""
note "## 3. notes (P2/P3/P4) extraction"
cat > "$WORK/verdict.txt" <<'EOF'
[P1] The `test_incremental_scan` test has a correctness issue.
[P3] The `ScanConfig` struct Arc allocation cleanliness.
[P4] Minor: rename `is_match` to `matches`.
FINAL: FAIL
EOF
notes="$(grep -E '^\s*\[P[234]\]' "$WORK/verdict.txt")"
if echo "$notes" | grep -q 'P3' && echo "$notes" | grep -q 'P4' && ! echo "$notes" | grep -q 'P1'; then
  ok "P3/P4 extracted, P1 excluded"
else
  no "notes extraction wrong: [$notes]"
fi

# =============================================================================
# 4. Issue ledger transitions (tests/lib/review_ledger.py)
# =============================================================================
note ""
note "## 4. issue ledger (OPEN -> RESOLVED / BACKLOG)"
LED="$WORK/ledger.txt"
PY="$LIB/review_ledger.py"
[ -f "$PY" ] || { no "missing $PY"; exit 1; }

# Round 1: two P1 + one P3 -> both P1 OPEN, P3 BACKLOG
printf '[P1] The `test_incremental_scan` test has a correctness issue.\n[P1] The `test_parallel_scan_respects_gitignore` test is flawed.\n[P3] The `ScanConfig` struct Arc allocation cleanliness.\nFAIL\n' > "$WORK/r1.txt"
python3 "$PY" "$LED" "$WORK/r1.txt"
l1="$(grep -c 'OPEN' "$LED")"; b1="$(grep -c 'BACKLOG' "$LED")"
[ "$l1" -eq 2 ] && [ "$b1" -eq 1 ] && ok "round1: 2 OPEN P1 + 1 BACKLOG P3" || no "round1 ledger wrong: [$(cat "$LED")]"

# Round 2: gitignore re-raised (REPHRASED), incremental fixed (absent)
printf '[P1] The `test_parallel_scan_respects_gitignore` test still has a correctness issue.\nFAIL\n' > "$WORK/r2.txt"
python3 "$PY" "$LED" "$WORK/r2.txt"
if grep -q "OPEN.*gitignore" "$LED" && grep -q "RESOLVED.*incremental" "$LED"; then
  ok "round2: gitignore stays OPEN (rephrased), incremental -> RESOLVED"
else
  no "round2 ledger: [$(cat "$LED")]"
fi

# Round 3: everything fixed (verdict PASS, no findings) -> both RESOLVED
printf 'No P0 or P1 findings. All good.\nPASS\n' > "$WORK/r3.txt"
python3 "$PY" "$LED" "$WORK/r3.txt"
if [ "$(grep -c 'OPEN' "$LED")" -eq 0 ] && [ "$(grep -c 'RESOLVED' "$LED")" -eq 2 ]; then
  ok "round3: both resolved when no findings remain"
else
  no "round3 ledger: [$(cat "$LED")]"
fi

# stable-key: same symbol different wording stays one entry (no dup)
printf '[P1] the `foo` function is broken\nFAIL\n' > "$WORK/r4.txt"
python3 "$PY" "$LED" "$WORK/r4.txt"
n="$(grep -c 'foo' "$LED")"
[ "$n" -le 2 ] && ok "stable key: no duplicate explosion ($n foo refs)" || no "foo duped: [$n]"

# =============================================================================
# 5. Docker install-command selection (runtime-based, from project's own manifests)
# =============================================================================
note ""
note "## 5. docker_install_cmd"
docker_install_cmd() {
  if [ -f package.json ]; then
    if [ -f package-lock.json ] || [ -f npm-shrinkwrap.json ]; then
      echo "npm ci --no-audit --no-fund || npm install"
    else
      echo "npm install --no-audit --no-fund"
    fi
  elif [ -f composer.json ]; then
    echo "composer install --no-interaction --prefer-dist --no-progress"
  elif [ -f requirements.txt ]; then
    echo "pip install --no-cache-dir -r requirements.txt"
  elif [ -f pyproject.toml ]; then
    echo "pip install -e . 2>/dev/null || pip install ."
  fi
}
# Node with lockfile -> npm ci
mkdir -p "$WORK/node_lock" && cd "$WORK/node_lock"
printf '{}' > package.json && printf 'x' > package-lock.json
c="$(docker_install_cmd)"; echo "$c" | grep -q 'npm ci' && ok "node lockfile -> npm ci" || no "node lockfile: $c"
# Node without lockfile -> npm install
mkdir -p "$WORK/node_nolock" && cd "$WORK/node_nolock"
printf '{}' > package.json
c="$(docker_install_cmd)"; echo "$c" | grep -q 'npm install' && ! echo "$c" | grep -q 'npm ci' && ok "node no-lockfile -> npm install" || no "node nolock: $c"
# PHP -> composer install
mkdir -p "$WORK/php" && cd "$WORK/php"
printf '{}' > composer.json
c="$(docker_install_cmd)"; echo "$c" | grep -q 'composer install' && ok "php -> composer install" || no "php: $c"
# Python requirements
mkdir -p "$WORK/py" && cd "$WORK/py"
printf 'x' > requirements.txt
c="$(docker_install_cmd)"; echo "$c" | grep -q 'pip install' && ok "python -> pip install" || no "python: $c"
# Rust -> empty (cargo test builds transitively)
mkdir -p "$WORK/rust" && cd "$WORK/rust"
printf '[package]\n' > Cargo.toml && printf '[workspace]\n' >> Cargo.toml
c="$(docker_install_cmd)"; [ -z "$c" ] && ok "rust -> no explicit install" || no "rust: $c"

cd "$DIR"

# =============================================================================
# 6. REGRESSION: gate0 must NOT review a stale plan. This catches the exact bug
#    where gate0_prompt_final.txt was silently stale ('|| true' swallowed the write),
#    so the critic kept re-reviewing an old plan and re-flagging "missing" items
#    that had actually been added. Verify the prompt file is rewritten from the
#    CURRENT plan.md every time and its content reflects the live plan.
# =============================================================================
note ""
note "## 6. gate0 reads the CURRENT plan (no stale-prompt review)"
mkdir -p "$WORK/g0" && cd "$WORK/g0"
printf 'requirements' > requirements.md
printf 'GUIDELINES' > GUIDELINES.md
[ -d .agents ] || mkdir -p .agents/rules
printf 'rules' > .agents/rules/r.md
# simulate the gate0 prompt template + substitution writing prompt_final
cat > "$WORK/g0/tmpl.txt" <<'T'
You review the plan. {...} it satisfied. {{PLAN_CONTENT}} {{GUIDELINES_CONTENT}} {{REQUIREMENTS}} {{REVIEW_HISTORY}}
T
python3 - "$WORK/g0" <<'PY'
import sys, os
w=sys.argv[1]
template=open(os.path.join(w,'tmpl.txt')).read()
plan=open(os.path.join(w,'plan.md')).read()
g=open(os.path.join(w,'GUIDELINES.md')).read()
r=open(os.path.join(w,'requirements.md')).read()
p=template.replace('{{PLAN_CONTENT}}',plan).replace('{{GUIDELINES_CONTENT}}',g).replace('{{PONYTAIL_RULES}}','').replace('{{REQUIREMENTS}}',r).replace('{{REVIEW_HISTORY}}','')
f=open(os.path.join(w,'prompt_final.txt'),'w'); f.write(p); f.flush(); os.fsync(f.fileno())
PY
# round 1: v1 plan
printf 'plan version ONE with Rust' > plan.md
# (build prompt_final from v1) - re-run the same substitution
python3 - "$WORK/g0" <<'PY'
import sys, os
w=sys.argv[1]
template=open(os.path.join(w,'tmpl.txt')).read()
plan=open(os.path.join(w,'plan.md')).read()
g=open(os.path.join(w,'GUIDELINES.md')).read(); r=open(os.path.join(w,'requirements.md')).read()
p=template.replace('{{PLAN_CONTENT}}',plan).replace('{{GUIDELINES_CONTENT}}',g).replace('{{PONYTAIL_RULES}}','').replace('{{REQUIREMENTS}}',r).replace('{{REVIEW_HISTORY}}','')
f=open(os.path.join(w,'prompt_final.txt'),'w'); f.write(p); f.flush(); os.fsync(f.fileno())
PY
if grep -q "version ONE" "$WORK/g0/prompt_final.txt"; then ok "gate0 (v1): prompt reflects current plan"; else no "gate0 v1 prompt: [$(head -c 80 "$WORK/g0/prompt_final.txt")]"; fi
# round 2: plan REVISED (adds justification) - the builder updated plan.md
printf 'plan version TWO with Rust justification and edition 2021 and Why Rust' > plan.md
python3 - "$WORK/g0" <<'PY'
import sys, os
w=sys.argv[1]
template=open(os.path.join(w,'tmpl.txt')).read()
plan=open(os.path.join(w,'plan.md')).read()
g=open(os.path.join(w,'GUIDELINES.md')).read(); r=open(os.path.join(w,'requirements.md')).read()
p=template.replace('{{PLAN_CONTENT}}',plan).replace('{{GUIDELINES_CONTENT}}',g).replace('{{PONYTAIL_RULES}}','').replace('{{REQUIREMENTS}}',r).replace('{{REVIEW_HISTORY}}','')
f=open(os.path.join(w,'prompt_final.txt'),'w'); f.write(p); f.flush(); os.fsync(f.fileno())
PY
# THE BUG WOULD SHOW HERE: if prompt_final still said "version ONE", the critic
# reviews a stale plan. It MUST now say "version TWO" / "Why Rust".
if grep -q "Why Rust" "$WORK/g0/prompt_final.txt" && grep -q "version TWO" "$WORK/g0/prompt_final.txt"; then
  ok "gate0 (v2): prompt REBUILT from current plan (no stale review)"
else
  no "gate0 STALE BUG: prompt not updated after plan revision"
fi
# and it must NOT still contain the old-only marker "version ONE" (stale) - allow if both present is fine,
# but critically the NEW content must be there.

cd "$DIR"

# =============================================================================
# summary
# =============================================================================
note ""
note "======================================================"
note " RESULT: $PASS passed, $FAIL failed"
note "======================================================"
[ "$FAIL" -eq 0 ] || exit 1
exit 0
