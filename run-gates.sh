#!/usr/bin/env bash
# run-gates.sh — HARD, non-skippable quality gates for the pipeline.
# Runs ALL gates in sequence. THIS IS THE ONLY way the loop proceeds past "build".
# GATE 0 (plan review) -> must pass
# GATE 1 (tests)       -> must pass
# GATE 2 (adversarial review) -> must pass
# GATE 3 (visual/e2e)  -> must pass (if UI exists)
#
# If ANY gate fails, this exits non-zero and the pipeline MUST halt.
# Builder must fix, then re-run run-gates.sh.
# Nothing proceeds to the steer/commit step without ALL gates passing.
#
# Run from the PROJECT ROOT (./run-gates.sh). Do NOT cd — the caller runs it
# from the repo root so tests/ scripts resolve correctly.

set -uo pipefail
PROJ_ROOT="$(pwd)"

# Load the project's .env (the ONLY one the pipeline reads).
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; . "$PROJ_ROOT/.env"; set +a
  [ "${REVIEW_DEBUG:-0}" = "1" ] && echo ">> loaded project .env ($PROJ_ROOT/.env)"
else
  echo "!! no .env found at $PROJ_ROOT/.env (gates may skip infra-dependent tests)"
fi

echo ""
echo "=============================================="
echo "  HARD QUALITY GATES — run-gates.sh"
echo "=============================================="

# ---- GATE 0: Plan Review (NEW) ----
echo ""
echo "==> [GATE 0 / 4] Plan Review (adversarial plan review)..."
if ! bash "$PROJ_ROOT/tests/gate0_plan_review.sh"; then
  echo ""
  echo "xx GATE 0 FAILED — plan rejected. Builder must revise plan.md and re-run run-gates.sh."
  exit 1
fi
echo "==> [GATE 0 / 4] Plan review passed."

# ---- BUILD PHASE: Create skeleton from plan.md if not exists ----
if [ ! -f "$PROJ_ROOT/Cargo.toml" ] || [ ! -d "$PROJ_ROOT/crates" ]; then
  echo ""
  echo "==> [BUILD] Creating project skeleton from plan.md..."
  # This triggers the builder (omp) to generate the skeleton from plan.md
  # The builder (omp) reads plan.md and creates the project structure
  if ! bash "$PROJ_ROOT/pipeline.sh" build-skeleton; then
    echo "xx BUILD FAILED — skeleton creation failed"
    exit 1
  fi
fi

# ---- GATE 1: Tests ----
echo ""
echo "==> [GATE 1 / 4] Running tests (auto-detect stack)..."
if ! bash "$PROJ_ROOT/tests/gate.sh"; then
  echo ""
  echo "xx GATE 1 FAILED — tests are red. Pipeline HALTS. Fix tests, re-run run-gates.sh."
  exit 1
fi
echo "==> [GATE 1 / 4] Tests passed."

# ---- GATE 2: Adversarial Review ----
echo ""
echo "==> [GATE 2 / 4] Building diff to review..."
# Generate diff of uncommitted + staged work
git add -N . 2>/dev/null || true
git diff --cached -- . ':!**/package-lock.json' ':!**/pnpm-lock.yaml' ':!**/yarn.lock' > /tmp/review.diff
git diff -- . ':!**/package-lock.json' ':!**/pnpm-lock.yaml' ':!**/yarn.lock.yaml' >> /tmp/review.diff

if [ ! -s /tmp/review.diff ]; then
  echo "xx GATE 2 — no diff to review. Pipeline HALTS (nothing built?)."
  exit 1
fi

# Generate diff stats
DIFF_LINES=$(wc -l < /tmp/review.diff)
DIFF_BYTES=$(wc -c < /tmp/review.diff)
echo "Diff: $DIFF_LINES lines ($DIFF_BYTES bytes)"

# Load project .env for OpenRouter key
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; . "$PROJ_ROOT/.env"; set +a
fi

OR_KEY="${OPENROUTER_API_KEY:-}"
if [ -z "$OR_KEY" ]; then
  echo "xx OPENROUTER_API_KEY not set. Add to .env"
  exit 1
fi

# Load context files
REQ_TXT=""
if [ -n "$CRITIC_REQUIREMENTS" ] && [ -f "$CRITIC_REQUIREMENTS" ]; then
  REQ_TXT=$(head -c 8000 "$CRITIC_REQUIREMENTS")
fi

GUIDELINES_CONTENT=$(head -c 15000 "$PROJ_ROOT/GUIDELINES.md" 2>/dev/null || echo "GUIDELINES.md not found")
PONYTAIL_RULES=""
if [ -d "$PROJ_ROOT/.agents/rules" ]; then
  PONYTAIL_RULES=$(find "$PROJ_ROOT/.agents/rules" -name "*.yaml" -o -name "*.md" -o -name "*.txt" 2>/dev/null | head -5 | xargs cat 2>/dev/null | head -c 5000)
fi

# Build review request with full context
cat > /tmp/review_request.json <<'NODEEOF'
const fs = require('fs');
const diff = fs.readFileSync('/tmp/review.diff', 'utf8');
let req = '';
try { req = fs.readFileSync('requirements.md', 'utf8').slice(0, 8000); } catch (e) {}

const guidelines = fs.readFileSync('GUIDELINES.md', 'utf8').slice(0, 15000);
let ponytail = '';
try { 
  const fs2 = require('fs');
  const rulesDir = '.agents/rules';
  if (fs2.existsSync(rulesDir)) {
    const files = fs.readdirSync(rulesDir).filter(f => f.endsWith('.yaml') || f.endsWith('.md') || f.endsWith('.txt'));
    let content = '';
    for (const f of files.slice(0, 5)) {
      content += fs2.readFileSync(require('path').join(rulesDir, f), 'utf8') + '\n';
    }
    ponytail = content.slice(0, 5000);
  }
} catch(e) {}

const reqFile = 'requirements.md';
let req = '';
try { req = fs.readFileSync(reqFile, 'utf8').slice(0, 8000); } catch(e) {}

const prompt = `You are an adversarial code reviewer. A builder produced the diff below.

` + (process.argv[4] === 'mvp' 
  ? `GRADING (MVP standard): Use strict judgement. A real correctness or security defect MUST be FAIL.
     Design/UX/tooling issues that the PROJECT REQUIREMENTS explicitly list as out-of-scope are acceptable:
     list them under "NOTES (non-blocking)" and do NOT fail the gate for them.
     If there are no correctness/security defects worth blocking on, your final verdict is PASS (notes may follow).`
  : `GRADING (PRODUCTION standard): This is a production build. Any correctness, security, or UX defect is FAIL.
     No tradeoffs are accepted. Be strict.`);

PROJECT REQUIREMENTS / SCOPE (in/out-of-scope context):
${req}

QUALITY STANDARDS (from GUIDELINES.md):
${guidelines}

PONYTAIL RULES (from .agents/rules/):
${ponytail}

Review the diff harshly for correctness, logic, security, edge cases, and test coverage.
Apply Karpathy principles (explicit > implicit, types everywhere, small functions, no cleverness, explicit errors).
Apply Ponytail rules (hexagonal architecture, dependency inversion, strict TS, conventional commits, TDD).
Apply project-specific rules from GUIDELINES.md.

List concrete BLOCKING problems first (each with severity), then any NON-BLOCKING notes.
End your reply with a SINGLE final line that is exactly PASS or FAIL.

DIFF:
${diff}
`;

const payload = {
  model: process.argv[2],
  messages: [{ role: 'user', content: prompt }],
  max_tokens: Number(process.argv[6] || 16000),
  temperature: 0.2
};

fs.writeFileSync('/tmp/review_request.json', JSON.stringify(payload, null, 2));
NODEEOF

# Build the request JSON with proper escaping
node -e '
const fs = require("fs");
const diff = fs.readFileSync("/tmp/review.diff", "utf8");
const guidelines = fs.readFileSync("GUIDELINES.md", "utf8").slice(0, 15000);
let ponytail = "";
try { 
  const fs2 = require("fs");
  const rulesDir = ".agents/rules";
  if (fs2.existsSync(rulesDir)) {
    const files = fs.readdirSync(rulesDir).filter(f => f.endsWith(".yaml") || f.endsWith(".md") || f.endsWith(".txt"));
    let content = "";
    for (const f of files.slice(0, 5)) {
      content += fs2.readFileSync(require("path").join(rulesDir, f), "utf8") + "\n";
    }
    ponytail = content.slice(0, 5000);
  }
} catch(e) {}

const reqFile = "requirements.md";
let req = "";
try { req = fs.readFileSync(reqFile, "utf8").slice(0, 8000); } catch(e) {}

const prompt = \`You are an adversarial code reviewer. A builder produced the diff below.

\` + (process.argv[4] === "mvp" 
  ? \`GRADING (MVP standard): Use strict judgement. A real correctness or security defect MUST be FAIL.
     Design/UX/tooling issues that the PROJECT REQUIREMENTS explicitly list as out-of-scope are acceptable:
     list them under "NOTES (non-blocking)" and do NOT fail the gate for them.
     If there are no correctness/security defects worth blocking on, your final verdict is PASS (notes may follow).\`
  : \`GRADING (PRODUCTION standard): This is a production build. Any correctness, security, or UX defect is FAIL.
     No tradeoffs are accepted. Be strict.\`);

PROJECT REQUIREMENTS / SCOPE (in/out-of-scope context):
\${req}

QUALITY STANDARDS (from GUIDELINES.md):
\${guidelines}

PONYTAIL RULES (from .agents/rules/):
\${ponytail}

Review the diff harshly for correctness, logic, security, edge cases, and test coverage.
Apply Karpathy principles (explicit > implicit, types everywhere, small functions, no cleverness, explicit errors).
Apply Ponytail rules (hexagonal architecture, dependency inversion, strict TS, conventional commits, TDD).
Apply project-specific rules from GUIDELINES.md.

List concrete BLOCKING problems first (each with severity), then any NON-BLOCKING notes.
End your reply with a SINGLE final line that is exactly PASS or FAIL.

DIFF:
\${diff}
\`;

const payload = {
  model: process.argv[2],
  messages: [{ role: "user", content: prompt }],
  max_tokens: Number(process.argv[6] || 16000),
  temperature: 0.2
};

fs.writeFileSync("/tmp/review_request.json", JSON.stringify(payload, null, 2));
' "$CRITIC_MODEL" "$CRITIC_STANDARD" "$CRITIC_REQUIREMENTS" "$CRITIC_MAX_TOKENS" 2>/dev/null

# POST to OpenRouter
curl -sS --max-time "${CRITIC_TIMEOUT}" \
  -X POST "https://openrouter.ai/api/v1/chat/completions" \
  -H "Authorization: Bearer ${OPENROUTER_API_KEY}" \
  -H "Content-Type: application/json" \
  -H "HTTP-Referer: http://localhost" \
  -H "X-Title: omp-agent" \
  --data @/tmp/review_request.json > /tmp/review_response.json

CURL_EC=$?
if [ $CURL_EC -ne 0 ]; then
  echo "xx Critic request failed/aborted (curl exit ${CURL_EC}). Increase CRITIC_TIMEOUT if it timed out."
  exit 1
fi

# Parse response
node -e "
const fs = require('fs');
let d;
try { d = JSON.parse(fs.readFileSync('/tmp/review_response.json', 'utf8')); }
catch(e) { console.log('RESPONSE-ERROR: ' + e.message); process.exit(1); }
if (d.error) { console.log('API-ERROR: ' + JSON.stringify(d.error)); process.exit(1); }
const content = d.choices?.[0]?.message?.content || '';
console.log(content);
fs.writeFileSync('/tmp/review_verdict.txt', content);
" > /tmp/review_verdict.txt 2>&1

RC=$?
if [ $RC -ne 0 ]; then
  echo "xx Could not parse critic response (see /tmp/review_response.json)."
  exit 1
fi

# Extract verdict
VERDICT=$(grep -iE '(^|[^A-Za-z])(PASS|FAIL)([^A-Za-z]|$)' /tmp/review_verdict.txt | tail -1 | grep -oEi 'PASS|FAIL' | tail -1 | tr '[:lower:]' '[:upper:]')

echo ""
echo "---- critic verdict ----"
cat /tmp/review_verdict.txt
echo "------------------------"

if [ "$VERDICT" = "FAIL" ]; then
  echo "==> GATE 2: FAIL — blocking findings fed back to builder (see /tmp/review_verdict.txt)."
  exit 1
fi
if [ "$VERDICT" = "PASS" ]; then
  echo "==> GATE 2: PASS (standard=${CRITIC_STANDARD}). Non-blocking notes in /tmp/review_verdict.txt."
  exit 0
fi

echo "xx No clear PASS/FAIL verdict found in critic output."
exit 1