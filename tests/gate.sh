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

# Returns the project's own dependency-install command, derived from ITS manifests
# (not a curated framework list). Next.js/Nuxt/React -> node json deps; LAMP -> composer;
# Python -> pip. Empty = no install needed (cargo test / go test build transitively).
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
  # Rust/Go: no explicit install step; cargo test / go test build from lockfiles.
}

run_cmd() {
  local label="$1"; shift
  echo "==> [gate] ${label}: $*"
  local out rc
  # When a toolchain image is provided (pipeline docker preflight), run the project's
  # own install step then the test INSIDE the container. The container has the native
  # build deps (dbus/gtk/webkit2gtk...), removing host-lib blockers.
  if [ -n "${OMP_DOCKER_IMAGE:-}" ]; then
    echo "==> [gate] (docker: ${OMP_DOCKER_IMAGE})"
    local inst
    inst="$(docker_install_cmd)"
    # Source the image's toolchain env (cargo/rustup live under CARGO_HOME inside the
    # container) and mount project at /app. Run install if one is declared, then the test.
    # The test command is passed via env var CMD to avoid `--`/`$*` parsing quirks.
    out="$(docker run --rm \
            -v "$(pwd)":/app -w /app \
            -e INSTALL="$inst" \
            -e CMD="$*" \
            "${OMP_DOCKER_IMAGE}" bash -lc \
            'export PATH="${CARGO_HOME:-/root/.cargo}/bin:/usr/local/bin:$PATH"; export HOME=/root; \
             [ -z "$INSTALL" ] || { echo "--[gate] install: $INSTALL"; eval "$INSTALL" || exit 1; }; \
             { [ -n "$CMD" ] && eval "$CMD" || exit 0; }' 2>&1)"
    rc=$?
  else
    out="$(eval "$*" 2>&1)"
    rc=$?
  fi
  if [ "$rc" -ne 0 ]; then
    echo "xx [gate] FAILED -> ${label}: $*"
    FAILED=$((FAILED+1))
    # Surface an actionable hint for well-known environment/dependency blockers so a
    # cloned user gets a clear fix instead of a cryptic compile panic.
    hint_for_failure "$out" "$label"
  fi
  RUN=$((RUN+1))
}

# Maps common system-dependency build failures to a clear, actionable fix. A user
# who clones omp-agent and has missing native libs sees this instead of a raw
# libdbus-sys/glib-sys panic wall.
hint_for_failure() {
  local out="$1" label="$2"
  # dbus (Tauri GUI on Linux)
  if printf '%s' "$out" | grep -qiE "dbus-1.*not found|cannot find.*dbus|libdbus-sys|dbus-1\.pc"; then
    echo ""
    echo "  >> HINT (${label}): missing 'libdbus-1-dev' (needed by Tauri/GUI builds)."
    echo "     On Ubuntu/Debian:  sudo apt install -y libdbus-1-dev pkg-config"
    echo "     On Fedora:         sudo dnf install -y dbus-devel pkgconf-pkg-config"
    echo "     On macOS:          brew install dbus pkg-config"
    return
  fi
  # glib (GTK, many native crates)
  if printf '%s' "$out" | grep -qiE "glib-2\.0.*not found|cannot find.*glib|glib-sys|glib-2\.0\.pc"; then
    echo ""
    echo "  >> HINT (${label}): missing GLib dev headers (libglib2.0-dev)."
    echo "     On Ubuntu/Debian:  sudo apt install -y libglib2.0-dev pkg-config"
    return
  fi
  # pkg-config itself
  if printf '%s' "$out" | grep -qiE "pkg-config.*not found|command 'pkg-config'.*not found"; then
    echo ""
    echo "  >> HINT (${label}): pkg-config is not installed."
    echo "     On Ubuntu/Debian:  sudo apt install -y pkg-config"
    return
  fi
  # generic Rust linker / C compiler missing
  if printf '%s' "$out" | grep -qiE "linker `cc` not found|cc: error|gcc.*not found|No such file or directory \(os error 2\)"; then
    echo ""
    echo "  >> HINT (${label}): a C compiler / linker is missing (needed to build native crates)."
    echo "     On Ubuntu/Debian:  sudo apt install -y build-essential"
    return
  fi
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
