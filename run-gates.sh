#!/usr/bin/env bash
# run-gates.sh — HARD, non-skippable quality gate for the pipeline.
#
# Runs BOTH gates in sequence. THIS IS THE ONLY way the loop proceeds past "build".
# GATE 1 (tests)  -> must pass (exit 0)
# GATE 2 (adversarial review) -> must pass (critic exits 0)
#
# If either fails, this exits non-zero and the pipeline MUST halt (builder must
# fix, then re-run run-gates.sh). Nothing proceeds to the steer/commit step
# without this passing.
#
# PIPELINE MUST treat this as a mandatory step after BUILD. Do not skip it.
# run-gates.sh is run from the PROJECT ROOT (./run-gates.sh). Do NOT cd — the
# caller runs it from the repo root so `tests/gate.sh` etc. resolve correctly.
set -uo pipefail
PROJ_ROOT="$(pwd)"

# Load the project's .env (the ONLY one the pipeline reads). This exposes
# DATABASE_URL / ABLY_KEY / OPENROUTER_API_KEY etc. so the gates run against real
# infra instead of silently skipping (e.g. the 8 Neon tests skip without it).
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; # shellcheck disable=SC1091
  . "$PROJ_ROOT/.env"
  set +a
  [ "${REVIEW_DEBUG:-0}" = "1" ] && echo ">> loaded project .env ($PROJ_ROOT/.env)"
else
  echo "!! no .env found at $PROJ_ROOT/.env (gates may skip infra-dependent tests)"
fi

echo ""
echo "=============================================="
echo "  HARD QUALITY GATE — run-gates.sh"
echo "=============================================="

# ---- GATE 1: tests ----
echo ""
echo "==> [GATE 1 / 2] Running tests (auto-detect stack)..."
if ! ./tests/gate.sh; then
  echo ""
  echo "xx GATE 1 FAILED — tests are red. Pipeline HALTS. Fix tests, re-run run-gates.sh."
  exit 1
fi
echo "==> [GATE 1 / 2] Tests passed."

# ---- GATE 2: adversarial review (a DIFFERENT model criticizes the diff) ----
# Generate the diff of uncommitted work + staged changes to review.
echo ""
echo "==> [GATE 2 / 2] Building diff to review..."
git add -N . 2>/dev/null || true   # track new/untracked files in the diff
# Exclude lockfiles at ANY depth (use **/ so client/ and server/ lockfiles are skipped):
git diff --cached -- . ':!**/package-lock.json' ':!**/pnpm-lock.yaml' ':!**/yarn-lock.yaml' > /tmp/review.diff
git diff -- . ':!**/package-lock.json' ':!**/pnpm-lock.yaml' ':!**/yarn-lock.yaml' >> /tmp/review.diff

if [ ! -s /tmp/review.diff ]; then
  echo "xx GATE 2 — no diff to review. Pipeline HALTS (nothing built?)."
  exit 1
fi

if ! CRITIC_MODEL="${CRITIC_MODEL:-z-ai/glm-5.2}" ./tests/review_gate.sh /tmp/review.diff; then
  echo ""
  echo "xx GATE 2 FAILED — adversarial review found problems. Pipeline HALTS."
  echo "   Fix the findings, then re-run run-gates.sh."
  exit 1
fi
echo "==> [GATE 2 / 2] Adversarial review passed."

echo ""
echo "=============================================="
echo "  HARD QUALITY GATE PASSED — proceed to steer/commit"
echo "=============================================="
exit 0
