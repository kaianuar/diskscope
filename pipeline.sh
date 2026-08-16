#!/usr/bin/env bash
# =============================================================================
# pipeline.sh — phased, fully-automated build orchestrator for omp-agent.
#
# DESIGN (v3, 2026-08-16):
#   The single-shot "plan + build everything" loop caused two prob lems:
#   • the plan critic kept escalating (severity inflation) because the plan
#     tried to specify the WHOLE app at once; and
#   • the builder scaffolded code before the plan was approved, so the review
#     judged a plan against code that didn't match it.
#
#   v3 fixes that with Kai's two refinements:
#   1. PLAN-ONLY PHASE — during planning the builder is allowed to touch ONLY
#      plan.md. No source files, no manifests, no builds. The plan is approved
#      on its own merit before any code exists.
#   2. PHASED BUILD — after plan approval, the build is split into phases (taken
#      from the plan's own build-order table). EACH phase is built + reviewed on
#      its own: builder writes that phase's code -> Gate 1 (tests) + Gate 2
#      (critic reviews just that phase's diff) -> pass -> next phase.
#
#   Result: each review is small and bounded, so the reviewer has far fewer
#   things to object to and the loop can genuinely converge.
#
# FLOW:
#   PLAN   : read requirements.md -> builder writes plan.md ONLY
#          -> Gate 0 reviews plan -> loop until PASS (revise plan only) or cap
#   PHASES : extract phase names from plan -> for each phase:
#          -> builder implements that phase (code + tests, from the approved plan)
#          -> run-gates.sh (Gate 1 tests + Gate 2 critic on that phase's diff)
#          -> loop until PASS (fix that phase's code) or cap
#   DONE   : all phases green -> steer/commit
#
# NOTE: omp must be on PATH (e.g. export PATH="$HOME/.bun/bin:$PATH").
# Env:
#   BUILDER_MODEL   OpenRouter model id (default xiaomi mimo-v2.5-pro)
#   MAX_PLAN_ROUNDS  plan-revision cap (default 5)
#   MAX_PHASE_ROUNDS phase-impl cap (default 5)
#   PLAN_ONLY        set 1 to stop after the plan is approved (no build)
#   PHASE_FROM       skip phases before this index (resume)
# =============================================================================
set -uo pipefail

BUILDER_MODEL="${BUILDER_MODEL:-xiaomi-token-plan-sgp/mimo-v2.5-pro}"
MAX_PLAN_ROUNDS="${MAX_PLAN_ROUNDS:-5}"
MAX_PHASE_ROUNDS="${MAX_PHASE_ROUNDS:-5}"
FINDINGS_FILE="${FINDINGS_FILE:-/tmp/review_verdict.txt}"
GATE0_VERDICT="/tmp/gate0_verdict.txt"
RUNLOG="/tmp/last_gate_failure.txt"

PROJ_ROOT="$(pwd)"

# ---- safe runner: bounded omp call so the pipeline never hangs on --print ----
run_omp() { # prompt...  (extra positional args are the task text)
  local task="${1:-}"
  local req="${REQUIREMENTS:-requirements.md}"
  local omp_call=(omp --print --model "$BUILDER_MODEL" --append-system-prompt=PIPELINE.md --append-system-prompt="$req")
  if [ -n "$task" ]; then
    omp_call+=( "$task" )
  fi
  timeout "${OMP_TIMEOUT:-900}" "${omp_call[@]}"
  local rc=$?
  if [ "$rc" -eq 124 ]; then
    echo "xx [pipeline] omp timed out (${OMP_TIMEOUT:-900}s). Marking as incomplete." >&2
  fi
  return "$rc"
}

# Extract the ordered list of implementation phases from plan.md.
# Matches ONLY top-level phase headings like "## Phase 1: Domain Core"
# (not table mentions, risk rows, or dependency references), dedupes, and sorts
# numerically so phases build in order 1..N.
extract_phases() {
  local phases
  phases=$(grep -iE '^#+[[:space:]]+phase[[:space:]]+[0-9]+' plan.md 2>/dev/null \
    | sed -E 's/^#+[[:space:]]*//I; s/[[:space:]]*:.*$//I; s/[[:space:]]+$//' \
    | tr '[:upper:]' '[:lower:]' \
    | sort -t' ' -k2 -n \
    | sort -u -k1,1 -k2,2n)
  if [ -z "$phases" ]; then
    echo "impl"
    return
  fi
  # Rebuild as single-word tokens "PhaseN" (no space) so array indexing works
  # without word-splitting "Phase 1" into ["Phase","1"].
  echo "$phases" | while read -r line; do
    n=$(echo "$line" | sed -E 's/^phase[[:space:]]+//')
    [ -n "$n" ] && echo "Phase${n}"
  done
}

# ---------------- argument dispatch ----------------
# `build-skeleton` stays a standalone, NON-RECURSIVE action (run-gates.sh invokes
# it after Gate 0). It must NOT start the phased loop, or we self-recurse.
CMD="${1:-build}"
if [ "$CMD" = "build-skeleton" ]; then
  echo "==> pipeline.sh: build-skeleton (standalone, non-recursive)"
  if [ ! -f plan.md ]; then
    echo "xx build-skeleton: no plan.md found. Run './pipeline.sh build' first." >&2; exit 1
  fi
  # Remove the build-skeleton from the inner plan-only concern: it implements a
  # single approved phase (or the whole plan if no phase given).
  PHASE="${2:-}"
  if [ -n "$PHASE" ]; then
    TASK="Implement this phase ONLY per the approved plan.md. Do not edit plan.md. Phase: ${PHASE}"
  else
    TASK="Implement the approved plan (all phases) in plan.md. Do not edit plan.md."
  fi
  run_omp "$TASK"
  echo "==> pipeline.sh: build-skeleton done (exit $?)."
  exit $?
fi

# Resolve requirements path.
if [ "$CMD" != "build" ]; then
  REQUIREMENTS="$CMD"
else
  REQUIREMENTS="${2:-requirements.md}"
fi

echo "============================================================"
echo " omp-agent PHASED autonomous build"
echo "  project: $PROJ_ROOT"
echo "  requirements: $REQUIREMENTS"
echo "  builder: $BUILDER_MODEL"
echo "============================================================"

if [ ! -f "$REQUIREMENTS" ]; then
  echo "xx requirements file not found: $REQUIREMENTS" >&2; exit 1
fi

# ============================================================================
# PHASE 0 — PLAN (planning only: builder may touch ONLY plan.md)
# ============================================================================
echo ""
echo "============================="
echo " PHASE 0: PLAN"
echo "============================="

plan_round=0
while [ "$plan_round" -lt "$MAX_PLAN_ROUNDS" ]; do
  plan_round=$((plan_round+1))
  echo ""
  echo "  ---- plan round ${plan_round}/${MAX_PLAN_ROUNDS} ----"

  # Build step: create/revise plan.md ONLY.
  if [ ! -f plan.md ]; then
    echo "==> [plan] no plan.md - builder writes the plan (planning only, NO code)..."
    run_omp "Write plan.md from the requirements. THIS IS PLANNING ONLY: you may create/Edit ONLY plan.md. Do NOT create any source files, Cargo.toml, package.json, or run any build — the plan must be approved before any code exists. Structure it with discrete PHASES (e.g. Phase 1: domain core, Phase 2: adapters, Phase 3: CLI, Phase 4: GUI) so each phase can be built and reviewed independently. Every phase must list: concrete deliverables, its tests in 'should <behavior> when <condition>' form, and which gates it satisfies. Keep it concrete but NOT bloated."
  else
    echo "==> [plan] revising plan.md per reviewer findings (planning only)..."
    run_omp "Revise plan.md ONLY to resolve the architecture review findings below. Do NOT create or edit any other file, and do NOT write any code — planning only. Keep the phase structure and do not let the plan balloon.\n\nFINDINGS:\n$(cat "${GATE0_VERDICT:-}" 2>/dev/null)"
  fi

  # Clear stale verdict, then run the plan gate ONLY (not the full run-gates,
  # which would try to build code before the plan is approved).
  rm -f "${GATE0_VERDICT}" "${FINDINGS_FILE}"
  set +e
  bash "$PROJ_ROOT/tests/gate0_plan_review.sh"
  PLAN_EC=$?
  set -e

  if [ "$PLAN_EC" -eq 0 ]; then
    echo "==> [plan] PLAN APPROVED. Proceeding to phased build."
    break
  fi
  echo "==> [plan] plan rejected (round ${plan_round}); feeding findings back."
done

if [ "$PLAN_EC" -ne 0 ]; then
  echo ""
  echo "!! Plan could not be approved within ${MAX_PLAN_ROUNDS} rounds."
  echo "   Latest plan-review findings: ${GATE0_VERDICT}"
  echo "   Review the plan manually, or the plan is over-specified for the critic."
  exit 1
fi

# PLAN_ONLY mode stops here.
if [ "${PLAN_ONLY:-0}" = "1" ]; then
  echo ""
  echo "==> PLAN_ONLY set - plan approved; stopping before build."
  exit 0
fi

# Establish a clean diff baseline: an empty initial commit (if the repo has no
# commits yet) so each phase's `git diff --cached HEAD` isolates only that phase.
if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
  git add -A 2>/dev/null || true
  git -c user.name=omp-agent -c user.email=omp-agent@local commit -q -m "chore: baseline (plan + scaffold)" 2>/dev/null || true
  echo "==> [pipeline] created baseline commit for per-phase diff isolation."
fi

# ============================================================================
# PHASES — build each identified phase independently (build + gates per phase)
# ============================================================================
echo ""
echo "============================="
echo " PHASES: building per approved plan"
echo "============================="

PHASES=( $(extract_phases) )
if [ "${#PHASES[@]}" -eq 0 ]; then
  PHASES=( "impl" )
fi
echo "Detected ${#PHASES[@]} phase(s): ${PHASES[*]}"

phase_idx=0
all_green=1
for PHASE in "${PHASES[@]}"; do
  phase_idx=$((phase_idx+1))
  if [ -n "${PHASE_FROM:-}" ] && [ "$phase_idx" -lt "$PHASE_FROM" ]; then
    echo "==> skipping phase ${phase_idx} (${PHASE}) via PHASE_FROM=${PHASE_FROM}"
    continue
  fi
  echo ""
  echo "=============================================================="
  echo "  PHASE ${phase_idx}/${#PHASES[@]} : ${PHASE}"
  echo "=============================================================="

  phase_round=0
  phase_done=0
  while [ "$phase_round" -lt "$MAX_PHASE_ROUNDS" ] && [ "$phase_done" -eq 0 ]; do
    phase_round=$((phase_round+1))
    echo ""
    echo "  ---- phase build round ${phase_round}/${MAX_PHASE_ROUNDS} ----"

    # IMPLEMENT this phase only (from approved plan).
    if [ "$phase_round" -eq 1 ]; then
      echo "==> [phase] implementing: ${PHASE}"
      run_omp "Implement THIS PHASE ONLY per the approved plan.md: ${PHASE}. Build only the code and tests this phase requires. Do NOT edit plan.md. Do not implement future phases."
    else
      FIX_SRC="$(cat "${FINDINGS_FILE:-}" 2>/dev/null)"
      [ -z "$FIX_SRC" ] && FIX_SRC="$(cat "${RUNLOG:-}" 2>/dev/null)"
      echo "==> [phase] revising code for reviewer/test findings..."
      run_omp "Your code for phase '${PHASE}' was reviewed and needs fixing. Fix ONLY the failing code/tests for THIS phase; do not touch later phases or edit plan.md. Findings:\n${FIX_SRC}"
    fi

    # GATE 1: tests for this phase.
    rm -f "${FINDINGS_FILE}"
    set +e
    bash "$PROJ_ROOT/tests/gate.sh" 2>&1 | tee "${RUNLOG}"
    T_EC=${PIPESTATUS[0]}
    set -e
    if [ "$T_EC" -ne 0 ]; then
      echo "  xx Gate 1 (tests) failed for phase ${PHASE} (round ${phase_round})."
      if [ "$phase_round" -ge "$MAX_PHASE_ROUNDS" ]; then
        echo "  xx Phase ${PHASE} reached its fix-round cap on tests."
        all_green=0
      fi
      continue
    fi

    # GATE 2: critic reviews ONLY this phase's diff (clean baseline = commit
    # after every passing phase, so uncommitted = this phase only).
    git add -A 2>/dev/null || true
    git diff --cached HEAD -- . ':!**/package-lock.json' ':!**/pnpm-lock.yaml' ':!**/yarn.lock' ':!**/Cargo.lock' ':!**/target/**' ':!**/node_modules/**' ':!**/tests/e2e/**' > /tmp/phase.diff
    if [ ! -s /tmp/phase.diff ]; then
      echo "  xx no diff for phase ${PHASE} (nothing staged since last commit). Halting." >&2
      all_green=0
      break
    fi
    set +e
    CRITIC_MODEL="${CRITIC_MODEL:-z-ai/glm-5.2}" bash "$PROJ_ROOT/tests/review_gate.sh" /tmp/phase.diff 2>&1 | tee -a "${RUNLOG}"
    C_EC=${PIPESTATUS[0]}
    set -e
    if [ "$C_EC" -eq 0 ]; then
      # Phase green: commit it so the NEXT phase has a clean diff baseline.
      git add -A 2>/dev/null || true
      git commit -q --no-verify -m "phase: ${PHASE}" 2>/dev/null || true
      echo "==> PHASE ${PHASE} PASSED + committed after ${phase_round} round(s)."
      phase_done=1
    else
      echo "  xx Gate 2 (critic) rejected phase ${PHASE} (round ${phase_round})."
      if [ "$phase_round" -ge "$MAX_PHASE_ROUNDS" ]; then
        echo "  xx Phase ${PHASE} reached its fix-round cap."
        all_green=0
      fi
    fi
  done
  if [ "$phase_done" -eq 0 ]; then
    all_green=0
  fi
done

if [ "$all_green" -eq 1 ]; then
  echo ""
  echo "================================================================"
  echo " ALL PHASES PASSED. Proceed to steer/commit."
  echo "================================================================"
  exit 0
fi

echo ""
echo "!! Build did not fully pass. See ${FINDINGS_FILE} for the latest critic findings."
echo "   Rerun with PHASE_FROM=<n> to resume from a given phase, or fix manually."
exit 1