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

BUILDER_MODEL="${BUILDER_MODEL:-commandcode/MiniMaxAI/MiniMax-M3}"
MAX_PLAN_ROUNDS="${MAX_PLAN_ROUNDS:-5}"
MAX_PHASE_ROUNDS="${MAX_PHASE_ROUNDS:-8}"
FINDINGS_FILE="${FINDINGS_FILE:-/tmp/review_verdict.txt}"
GATE0_VERDICT="/tmp/gate0_verdict.txt"
RUNLOG="/tmp/last_gate_failure.txt"
REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE:-/tmp/review_notes.txt}"
# Builder<->critic interaction log: a chronological, scannable record of what the
# builder was asked, what the critic reviewer saw (incl. a content-hash of the file
# it reviewed so stale copies are immediately visible), and the verdict it returned.
# Set INTERACTION_LOG=/dev/null to disable. Default /tmp/omp_interaction.log.
INTERACTION_LOG="${INTERACTION_LOG:-/tmp/omp_interaction.log}"

log_int() {  # log_int <section> <role> <phase/round> <message>
  [ -n "$INTERACTION_LOG" ] || return 0
  local t
  t="$(date +%H:%M:%S)"
  printf '%s  [%s] [%s] %-6s %s\n' "$t" "$1" "$2" "$3" "$4" >> "$INTERACTION_LOG"
}
hash_of() {  # sha256 prefix of a file (or N/A if missing) - used to spot stale content
  [ -f "$1" ] && sha256sum "$1" | cut -c1-12 || echo "N/A(no-file)"
}

# ERR trap: catch unexpected failures under set -e, log them, and CONTINUE.
# Without this, any grep/sed/awk returning non-zero under set -e kills the
# pipeline with no error message. The trap returns 0 so bash continues.
trap 'echo "xx [pipeline] ERR at ${BASH_SOURCE[0]}:${LINENO}: exit=$? cmd=${BASH_COMMAND}" >&2; log_int "PIPELINE" "ERR" "$LINENO" "exit=$? cmd=${BASH_COMMAND}"; true' ERR

PROJ_ROOT="$(pwd)"

# Present any outstanding NON-BLOCKING (P2/P3/P4) review findings collected across
# the phases, so the HUMAN can decide how to handle them (fix now, defer, or accept).
present_outstanding_notes() {
  if [ -s "${REVIEW_NOTES_FILE}" ]; then
    echo ""
    echo "================================================================"
    echo " OUTSTANDING NON-BLOCKING REVIEW FINDINGS (P2/P3/P4)"
    echo " These did not block any phase. Review and decide how to handle them."
    echo "================================================================"
    cat "${REVIEW_NOTES_FILE}"
    echo "--------------------------------------------------------------"
  else
    echo ""
    echo "==> No outstanding non-blocking (P2/P3/P4) findings."
  fi
}

# ---- docker pre-flight: build a per-project toolchain image so tests run in a
# reproducible container with all native build deps (dbus, gtk, webkit2gtk...)
# installed via a Dockerfile. Kills the "missing system library" blocker class.
# Non-fatal if docker is unavailable or the project opts out (ENV_DOCKER=0).
docker_preflight() {
  local proj_img=""
  [ -f Cargo.toml ] || [ -f package.json ] || [ -f pyproject.toml ] || [ -f go.mod ] || return 0 # nothing to run
  if ! command -v docker >/dev/null 2>&1; then
    echo "==> [docker] 'docker' not found - running gates on host (system deps must be present)."
    return 0
  fi
  if docker info >/dev/null 2>&1; then :; else
    echo "==> [docker] docker daemon not reachable - running gates on host."
    return 0
  fi

  # Detect RUNTIME (a small, stable set) and write a Dockerfile from it. The
  # image's install steps come from the project's OWN manifests (npm ci, cargo
  # build, pip install, composer install), so Next.js/Nuxt/React all resolve to a
  # Node base and pull exactly what the project declares. For stacks needing
  # unusual native libs, the builder supplies/revises a Dockerfile via plan review
  # and this preflight builds THAT file instead.
  if [ ! -f Dockerfile ]; then
    if [ -f Cargo.toml ]; then
      base="rust:latest"; apt="pkg-config build-essential"
      # Tauri/GUI crates (+ other C libs) need system dev packages. Detect these
      # from the crate graphs so a Rust GUI project gets its native deps.
      local native=""
      if grep -rqE "tauri|egui|gtk|webkit|openssl|sqlite|postgres|mysql" Cargo.toml crates/*/Cargo.toml 2>/dev/null; then
        native="libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libglib2.0-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libssl-dev libsqlite3-dev"
      fi
      cat > Dockerfile <<EOF
FROM ${base}
RUN apt-get update && apt-get install -y --no-install-recommends ${apt} ${native} \\
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# deps are fetched at run time by the pipeline (cargo build) using the project's Cargo.lock
EOF
      echo "==> [docker] wrote Dockerfile (Rust runtime${native:+ +native deps})."
    elif [ -f package.json ]; then
      cat > Dockerfile <<'EOF'
FROM node:24-bookworm-slim
WORKDIR /app
ENV CI=true
# run: npm ci && npm test (project's own lockfile + scripts)
EOF
      echo "==> [docker] wrote Dockerfile (Node runtime - covers React/Next.js/Nuxt/Vue)."
    elif [ -f composer.json ]; then
      cat > Dockerfile <<'EOF'
FROM composer:2
WORKDIR /app
# run: composer install --no-interaction (project's own composer.lock)
EOF
      echo "==> [docker] wrote Dockerfile (PHP/Composer runtime - covers LAMP)."
    elif [ -f pyproject.toml ] || [ -f requirements.txt ]; then
      cat > Dockerfile <<'EOF'
FROM python:3.12-slim
WORKDIR /app
# run: pip install -r requirements.txt || pip install -e .  (project's own deps)
EOF
      echo "==> [docker] wrote Dockerfile (Python runtime)."
    elif [ -f go.mod ]; then
      cat > Dockerfile <<'EOF'
FROM golang:latest
WORKDIR /app
# run: go test ./...  (project's own go.mod)
EOF
      echo "==> [docker] wrote Dockerfile (Go runtime)."
    else
      echo "==> [docker] no recognized language manifest; running gates on host."
      return 0
    fi
  fi

  # Image name = omp-env:<project> (stable per project). Build if not present.
  proj_img="omp-env:$([ -f Dockerfile ] && basename "$(dirname "$(pwd)")")"
  echo "==> [docker] ensuring build image ${proj_img}... (use DOCKER_BUILDKIT=0 if BuildKit missing)"
  if docker image inspect "$proj_img" >/dev/null 2>&1; then
    : # cached
  else
    DOCKER_BUILDKIT=0 docker build -q -t "$proj_img" . 2>&1 | tail -2 || proj_img=""
  fi
  export OMP_DOCKER_IMAGE="$proj_img"
  [ -n "$proj_img" ] && echo "==> [docker] gates will run in ${proj_img}."
}

# ---- safe runner: bounded omp call so the pipeline never hangs on --print ----
run_omp() { # prompt...  (extra positional args are the task text)
  local task="${1:-}"
  local req="${REQUIREMENTS:-requirements.md}"
  local omp_call=(omp --print --model "$BUILDER_MODEL" --append-system-prompt=PIPELINE.md --append-system-prompt="$req")
  if [ -n "$task" ]; then
    omp_call+=( "$task" )
  fi
  local start_time
  start_time=$(date +%s)
  log_int "OMP" "START" "$(date +%H:%M:%S)" "omp call: $(echo "$task" | head -c 80)..."
  timeout "${OMP_TIMEOUT:-1200}" "${omp_call[@]}"
  local rc=$?
  local end_time
  end_time=$(date +%s)
  local elapsed=$((end_time - start_time))
  log_int "OMP" "EXIT" "$(date +%H:%M:%S)" "omp exit=${rc} elapsed=${elapsed}s"
  if [ "$rc" -eq 124 ]; then
    echo "xx [pipeline] omp timed out (${OMP_TIMEOUT:-1200}s, ${elapsed}s elapsed). Marking as incomplete." >&2
  elif [ "$rc" -ne 0 ]; then
    # Log diagnostic info on non-zero exit (signal, OOM, resource limits)
    local oom_score
    oom_score=$(cat /proc/self/oom_score 2>/dev/null || echo "?")
    local rss_kb
    rss_kb=$(awk '/VmRSS/{print $2}' /proc/self/status 2>/dev/null || echo "?")
    local procs
    procs=$(ps aux | wc -l)
    echo "xx [pipeline] omp exited rc=${rc} after ${elapsed}s (rss=${rss_kb}KB, oom_score=${oom_score}, procs=${procs})" >&2
    log_int "OMP" "FAIL" "$(date +%H:%M:%S)" "exit=${rc} elapsed=${elapsed}s rss=${rss_kb}KB oom=${oom_score} procs=${procs}"
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
  # Truncate after the number so full-line descriptions don't get split into extra phases.
  echo "$phases" | while read -r line; do
    n=$(echo "$line" | sed -E 's/^phase[[:space:]]+//; s/[^0-9].*$//')
    [ -n "$n" ] && echo "Phase${n}"
  done
}

# Extract deliverable lines (starting with "- ") for a specific phase from plan.md.
# Usage: extract_deliverables "Phase 2"
# Returns one deliverable per line (the "- `file` -- description" text).
extract_deliverables() {
  local target_phase="$1"
  # Allow optional space between "Phase" and the number (Phase1 vs Phase 1)
  local target_re
  target_re=$(echo "$target_phase" | sed 's/Phase/Phase[[:space:]]*/')
  local in_phase=0 line
  while IFS= read -r line; do
    # Match ## or ### headings (Phase N or PhaseN)
    if echo "$line" | grep -qiE "^#{2,3}[[:space:]]+phase[[:space:]]*[0-9]+"; then
      if echo "$line" | grep -qiE "^#{2,3}[[:space:]]+${target_re}"; then
        in_phase=1
      else
        in_phase=0
      fi
      continue
    fi
    # Stop at the next ## or ### PHASE heading (different section), not sub-headings
    if [ "$in_phase" -eq 1 ] && echo "$line" | grep -qiE '^#{2,3}[[:space:]]+phase[[:space:]]*[0-9]+'; then
      break
    fi
    # Extract deliverable lines (skip Gate/test-list lines which start with "- **Gate")
    if [ "$in_phase" -eq 1 ] && echo "$line" | grep -qE '^- ' && ! echo "$line" | grep -qE '^- \*\*Gate'; then
      echo "$line" | sed 's/^- //'
    fi
  done < plan.md
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

# Start a fresh outstanding-notes file for this run (cleared so P2/P3/P4 findings
# don't leak in from a previous run).
rm -f "${REVIEW_NOTES_FILE}"

# Docker pre-flight: build/refresh the per-project toolchain image so gates run
# in a reproducible container. Non-fatal if docker is unavailable.
docker_preflight

# ============================================================================
# PHASE 0 — PLAN (planning only: builder may touch ONLY plan.md)
# ============================================================================
echo ""
echo "============================="
echo " PHASE 0: PLAN"
echo "============================="
[ -n "$INTERACTION_LOG" ] && : > "$INTERACTION_LOG"   # fresh interaction log per run
log_int "PLAN" "----" "start" "phase 0 plan (max ${MAX_PLAN_ROUNDS} rounds)"

plan_round=0
while [ "$plan_round" -lt "$MAX_PLAN_ROUNDS" ]; do
  plan_round=$((plan_round+1))
  echo ""
  echo "  ---- plan round ${plan_round}/${MAX_PLAN_ROUNDS} ----"
  log_int "PLAN" "----" "round" "round ${plan_round}/${MAX_PLAN_ROUNDS} starts"

  # Build step: create/revise plan.md ONLY.
  if [ ! -f plan.md ]; then
    echo "==> [plan] no plan.md - builder writes the plan (planning only, NO code)..."
    run_omp "Write plan.md from the requirements. THIS IS PLANNING ONLY: you may create/Edit ONLY plan.md. Do NOT create any source files, Cargo.toml, package.json, or run any build — the plan must be approved before any code exists. Structure it with discrete PHASES (e.g. Phase 1: domain core, Phase 2: adapters, Phase 3: CLI, Phase 4: GUI) so each phase can be built and reviewed independently. Every phase must list: concrete deliverables, its tests in 'should <behavior> when <condition>' form, and which gates it satisfies. Keep it concrete but NOT bloated."
  else
    echo "==> [plan] revising plan.md per reviewer findings (planning only)..."
    run_omp "Revise plan.md ONLY to resolve the architecture review findings below. Do NOT create or edit any other file, and do NOT write any code — planning only. Keep the phase structure and do not let the plan balloon.\n\nFINDINGS:\n$(cat "${GATE0_VERDICT:-}" 2>/dev/null)"
  fi

  # Log the plan the BUILDER produced NOW (its content hash) - the critic must
  # review THIS exact state; a differing critic-side hash later flags staleness.
  log_int "PLAN" "BUILDER" "r${plan_round}" "produced plan.md hash=$(hash_of plan.md) size=$(wc -c < plan.md 2>/dev/null || echo 0)B"

  # Clear stale verdict, then run the plan gate ONLY (not the full run-gates,
  # which would try to build code before the plan is approved).
  rm -f "${GATE0_VERDICT}" "${FINDINGS_FILE}"
  set +e
  REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE}" bash "$PROJ_ROOT/tests/gate0_plan_review.sh"
  PLAN_EC=$?
  set -e
  log_int "PLAN" "CRITIC" "r${plan_round}" "reviewed plan.md hash=$(hash_of plan.md); verdict=$(tail -1 "${GATE0_VERDICT:-}" 2>/dev/null | tr -d '[:space:]'); P1=$(grep -E '\[P1\]|^- \[P1\]' "${GATE0_VERDICT:-}" 2>/dev/null | wc -l)"

  if [ "$PLAN_EC" -eq 0 ]; then
    log_int "PLAN" "CRITIC" "r${plan_round}" "**** PLAN APPROVED ****"
    echo "==> [plan] PLAN APPROVED. Proceeding to phased build."
    break
  fi
  # If verdict is empty (API returned nothing), skip revision — the builder
  # can't improve with empty feedback. The next loop iteration re-runs gate0.
  if [ ! -s "${GATE0_VERDICT:-/dev/null}" ] || ! grep -qiE 'PASS|FAIL|\[P[0-4]\]' "${GATE0_VERDICT:-/dev/null}" 2>/dev/null; then
    log_int "PLAN" "CRITIC" "r${plan_round}" "REJECTED - empty verdict (API issue), skipping revision"
    echo "==> [plan] empty verdict from critic (API issue), retrying..."
    continue
  fi
  echo "==> [plan] plan rejected (round ${plan_round}); feeding findings back."
  log_int "PLAN" "CRITIC" "r${plan_round}" "REJECTED - findings fed to builder"
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
  log_int "PHASE" "----" "${PHASE}" "phase ${phase_idx}/${#PHASES[@]} starts"

  phase_round=0
  phase_done=0
  while [ "$phase_round" -lt "$MAX_PHASE_ROUNDS" ] && [ "$phase_done" -eq 0 ]; do
    phase_round=$((phase_round+1))
    echo ""
    log_int "PHASE" "----" "${PHASE}" "round ${phase_round}/${MAX_PHASE_ROUNDS}"
    echo "  ---- phase build round ${phase_round}/${MAX_PHASE_ROUNDS} ----"

    # IMPLEMENT this phase only (from approved plan).
    if [ "$phase_round" -eq 1 ]; then
      # Sub-chunk: extract deliverables from the plan and build one at a time.
      # Each focused omp call stays within OMP_TIMEOUT.
      # Cap at MAX_SUBCHUNKS (5) per phase — if more, send as one focused call.
      MAX_SUBCHUNKS="${MAX_SUBCHUNKS:-5}"
      IFS=$'\n' read -r -d '' -a deliverables < <(extract_deliverables "$PHASE" && printf '\0')
      if [ "${#deliverables[@]}" -eq 0 ]; then
        # Fallback: no deliverables found, use monolithic call
        echo "==> [phase] implementing (monolithic): ${PHASE}"
        log_int "PHASE" "BUILDER" "${PHASE}" "monolithic (no deliverables extracted)"
        set +e; run_omp "Implement THIS PHASE ONLY per the approved plan.md: ${PHASE}. Build only the code and tests this phase requires. Do NOT edit plan.md. Do not implement future phases."; set -e
      elif [ "${#deliverables[@]}" -le "$MAX_SUBCHUNKS" ]; then
        echo "==> [phase] implementing ${#deliverables[@]} deliverables for ${PHASE}:"
        log_int "PHASE" "BUILDER" "${PHASE}" "sub-chunked: ${#deliverables[@]} deliverables"
        for i in "${!deliverables[@]}"; do
          d="${deliverables[$i]}"
          # Extract file path from backtick-quoted name (e.g. "`foo/bar.rs` -- desc" -> "foo/bar.rs")
          file_hint=$(echo "$d" | grep -oE '`[^`]+`' | head -1 | tr -d '`' || true)
          desc=$(echo "$d" | sed 's/`[^`]*`[[:space:]]*--[[:space:]]*//')
          echo "  [$((i+1))/${#deliverables[@]}] ${file_hint:-$desc}"
          log_int "PHASE" "BUILDER" "${PHASE}" "deliverable $((i+1)): ${file_hint:-$desc}"
          echo "  [$(date +%H:%M:%S)] building [$((i+1))/${#deliverables[@]}] ${file_hint:-$desc}..."
          set +e; run_omp "Implement THIS SPECIFIC deliverable for phase '${PHASE}': ${d}. Only create/modify this file. Do NOT create other files, edit plan.md, or implement other phases."; set -e
          echo "  [$(date +%H:%M:%S)] done [$((i+1))/${#deliverables[@]}] ${file_hint:-$desc} ($?)"
        done
        echo "==> [phase] all ${#deliverables[@]} deliverables complete for ${PHASE}"
      else
        # Too many deliverables — send as one focused multi-file call
        dl_list=$(printf '  - %s\n' "${deliverables[@]}")
        echo "==> [phase] implementing ${#deliverables[@]} deliverables (batched) for ${PHASE}"
        log_int "PHASE" "BUILDER" "${PHASE}" "batched: ${#deliverables[@]} deliverables"
        echo "  [$(date +%H:%M:%S)] building ${#deliverables[@]} deliverables..."
        set +e; run_omp "Implement THIS PHASE ONLY per the approved plan.md: ${PHASE}. Build these specific deliverables:${dl_list} Do NOT edit plan.md. Do not implement future phases."; set -e
        echo "  [$(date +%H:%M:%S)] done batched ($?)"
      fi
    else
      FIX_SRC="$(cat "${FINDINGS_FILE:-}" 2>/dev/null)"
      [ -z "$FIX_SRC" ] && FIX_SRC="$(cat "${RUNLOG:-}" 2>/dev/null)"
      echo "==> [phase] revising code for reviewer/test findings..."
      set +e
      run_omp "Your code for phase '${PHASE}' was reviewed and needs fixing. CRITICAL: follow the -> FIX: instructions EXACTLY as written. Do NOT patch around the issue — implement the architectural change the FIX requests. Do NOT invent your own approach if the FIX specifies one. Fix ONLY the failing code/tests for THIS phase; do not touch later phases or edit plan.md. Findings:\n${FIX_SRC}"
      set -e
    fi

    # GATE 1: tests for this phase.
    # Ensure the project's toolchain image exists (builder has now scaffolded the
    # code + manifests, so the stack is known). Cached thereafter; gate.sh runs tests
    # inside the container, removing host system-lib blockers (dbus/gtk/etc).
    docker_preflight
    rm -f "${FINDINGS_FILE}"
    set +e
    bash "$PROJ_ROOT/tests/gate.sh" 2>&1 | tee "${RUNLOG}"
    T_EC=${PIPESTATUS[0]}
    set -e
    log_int "PHASE" "GATE1" "${PHASE}" "tests rc=${T_EC}"
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
    REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE}" REVIEW_LEDGER="${REVIEW_LEDGER:-/tmp/review_ledger.txt}" PIPELINE_CRITIC_MODEL="${CRITIC_MODEL:-xiaomi/mimo-v2.5-pro}" bash "$PROJ_ROOT/tests/review_gate.sh" /tmp/phase.diff 2>&1 | tee -a "${RUNLOG}"
    C_EC=${PIPESTATUS[0]}
    set -e
    log_int "PHASE" "GATE2" "${PHASE}" "critic rc=${C_EC} verdict=$(tail -1 /tmp/review_verdict.txt 2>/dev/null | tr -d '[:space:]') P1=$(grep -E '\[P1\]|^- \[P1\]' /tmp/review_verdict.txt 2>/dev/null | wc -l)"
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
  present_outstanding_notes
  exit 0
fi

echo ""
echo "!! Build did not fully pass. See ${FINDINGS_FILE} for the latest critic findings."
echo "   Rerun with PHASE_FROM=<n> to resume from a given phase, or fix manually."
present_outstanding_notes
exit 1