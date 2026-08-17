#!/usr/bin/env bash
# GATE 0 — Plan Review with Architecture Validation
# Runs BEFORE any code is written. Critic reviews the plan for architectural soundness.

set -uo pipefail
PROJ_ROOT="$(pwd)"

echo ""
echo "=============================================="
echo "  GATE 0: PLAN REVIEW (Architecture + Plan)"
echo "=============================================="
echo ""

# Load project .env for API keys
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; . "$PROJ_ROOT/.env"; set +a
fi

PLAN_FILE="$PROJ_ROOT/plan.md"
GUIDELINES_FILE="$PROJ_ROOT/GUIDELINES.md"

if [ ! -f "$PLAN_FILE" ]; then
  echo "xx GATE 0 FAILED — no plan.md found. Builder must create plan.md from PLANNING.md.template"
  exit 1
fi

if [ ! -f "$GUIDELINES_FILE" ]; then
  echo "xx GATE 0 FAILED — no GUIDELINES.md found. Run scaffold.sh or create GUIDELINES.md"
  exit 1
fi

# Load OpenRouter key
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; . "$PROJ_ROOT/.env"; set +a
fi
CRITIC_MODEL="kimi-k2.7-code"
CRITIC_URL="https://opencode.ai/zen/go/v1/chat/completions"
CRITIC_API_KEY="${OPENCODE_GO_API_KEY:-}"
if [ -z "$CRITIC_API_KEY" ]; then
  echo "xx OPENCODE_GO_API_KEY not set. Add to .env (OpenCode Go key)"
  exit 1
fi

echo "==> GATE 0: Plan Review (critic=${CRITIC_MODEL})"
echo "Plan: $PLAN_FILE"

# Load plan and guidelines
PLAN_CONTENT=$(cat "$PLAN_FILE")
GUIDELINES_CONTENT=$(head -c 20000 "$PROJ_ROOT/GUIDELINES.md" 2>/dev/null || echo "GUIDELINES.md not found")

# Load Ponytail rules if they exist
PONYTAIL_RULES=""
if [ -d "$PROJ_ROOT/.agents/rules" ]; then
  PONYTAIL_RULES=$(find "$PROJ_ROOT/.agents/rules" -name "*.yaml" -o -name "*.md" -o -name "*.txt" 2>/dev/null | head -5 | xargs cat 2>/dev/null | head -c 8000)
fi

# Load requirements if exists
REQ_TXT=""
if [ -f "$PROJ_ROOT/requirements.md" ]; then
  REQ_TXT=$(head -c 8000 "$PROJ_ROOT/requirements.md" 2>/dev/null || echo "")
fi

# Load prior review history (so the critic can see what it asked for in earlier
# rounds and avoid contradicting itself). History is appended to each round.
HISTORY_FILE="${GATE0_HISTORY:-/tmp/gate0_history.txt}"
HISTORY_TXT=""
if [ -f "$HISTORY_FILE" ]; then
  HISTORY_TXT=$(tail -c 6000 "$HISTORY_FILE" 2>/dev/null || echo "")
fi

echo "==> GATE 0: Plan Review (critic=${CRITIC_MODEL})"

# Build the critic prompt with architecture validation focus
cat > /tmp/gate0_prompt.txt <<'PROMPT_EOF'
You are an adversarial ARCHITECTURE REVIEWER. A builder submitted a plan.md for a new project.
Your job: review the plan HARSHLY against the quality standards below.
Your job is to VETO the plan if the architecture is unsound, unjustified, or incompatible with quality gates.

QUALITY STANDARDS (MUST FOLLOW):

1. KARPATHY PRINCIPLES:
- Explicit > Implicit — no magic, no hidden behavior, no hidden state
- Types everywhere — type hints on ALL functions, variables, returns. No any/any without comment
- Small functions — one thing per function, <50 lines ideal
- No cleverness — boring code > clever code
- Explicit error handling — no bare catch/except, handle explicitly
- Tests as documentation — test names describe behavior, not implementation
- No magic numbers — named constants with descriptive names
- Explicit dependencies — dependencies injected, not imported globally
- Fail fast — validate early, fail fast with descriptive messages

2. PONYTAIL RULES (from .agents/rules/):
- Hexagonal architecture — domain at center, adapters at edges
- Dependency inversion — domain depends on abstractions, not concretions
- Strict TypeScript — strict: true, noImplicitAny: true
- Conventional commits, TDD mandatory, explicit types
- No bare catch/except, no any/any without comment
- No magic numbers — named constants
- Explicit error handling with context

3. PROJECT-SPECIFIC RULES (from GUIDELINES.md):
- Architecture: Hexagonal, domain at center
- TDD mandatory — tests written FIRST
- Conventional commits, one logical change per commit
- Strict TypeScript, no any/any
- Explicit error handling, no bare catch

REVIEW CRITERIA — FAIL if ANY missing/violated:

### 1. ARCHITECTURE DECISION (MANDATORY)
Does the plan EXPLICITLY declare:
- Language & version (e.g., "Rust 2021 edition", "TypeScript 5.0")
- Primary framework (e.g., "Tauri v2 + React 18 + egui", "Go 1.22 + Gin")
- Workspace structure (e.g., "Cargo workspace: crates/scan-engine, crates/gui, crates/cli")
- Target platform (e.g., "Tauri v2 desktop app", "CLI binary", "Web app")
- Key crates/modules and their responsibilities

FAIL if: Plan says "I'll use Rust" without specifying edition, framework, or workspace structure.
FAIL if: Plan says "web app" without specifying framework (React? Svelte? Vanilla? Tauri? Electron?)

### 2. ARCHITECTURE JUSTIFICATION (MANDATORY)
Does the plan JUSTIFY the technical choices?
- Why this language? (e.g., "Rust for memory safety + performance in disk scanning")
- Why this framework? (e.g., "Tauri for native desktop + web tech, smaller than Electron")
- Why this architecture? (e.g., "Hexagonal for testability, domain separate from UI/DB")
- Why NOT alternatives? (e.g., "Not Electron — 100MB+ overhead; not Go — less GUI ecosystem")

FAIL if: Plan picks tech without justification. "I'll use Rust" is not a justification.

### 3. ARCHITECTURE SOUNDNESS
- Hexagonal/Clean architecture: Domain at center, adapters at edges
- Dependency inversion: Domain depends on abstractions (ports), adapters implement ports
- Domain has ZERO external dependencies
- Adapters implement domain ports (Repository, API, UI, etc.)

FAIL if: Plan shows domain importing DB drivers, HTTP clients, or GUI frameworks.

### 4. TDD FEASIBILITY
- Test plan listed in order (test-first sequence)?
- Tests cover: domain logic, adapter contracts, integration points?
- Test-first approach explicitly stated?

FAIL if: No test plan, or tests are an afterthought.

### 5. KARPATHY PRINCIPLES COMPLIANCE
- Explicit types everywhere? (No `any`/`Any` without comment)
- Small functions (<50 lines) planned?
- Explicit error handling (no bare catch/except)?
- Tests as documentation (names describe behavior)?
- No magic numbers/strings?

### 6. PONYTAIL RULES COMPLIANCE
- Hexagonal architecture declared?
- Dependency inversion declared?
- Strict TypeScript / Rust strict mode?
- Conventional commits + TDD + explicit types?
- No bare catch/except, no any/any?

### 6. GATE COMPATIBILITY
- Can Gate 1 run tests? (test command detectable)
- Can Gate 2 review diff? (git diff works on structure)
- Can Gate 3 run? (UI exists → Playwright + vision model)

### 7. RISK ASSESSMENT
- Technical risks identified?
- Mitigation strategies for each?

---

REVIEW OUTPUT FORMAT:
List findings sorted by severity, tagging EVERY finding with exactly one of:
- [P0] CRITICAL BLOCKER: security vulnerability, data loss/corruption, crash, or a
  clear violation of a REQUIREMENT acceptance criterion. Plan MUST NOT proceed.
- [P1] MAJOR BLOCKER: a real architecture/correctness defect that breaks the build
  or a REQUIREMENT acceptance criterion. Plan MUST NOT proceed until fixed.
- [P2] should fix: real issue that doesn't currently break a requirement/build but
  could become P0/P1 if left. Tracked; plan MAY proceed.
- [P3] good to fix: refactor/clarity/robustness improvement. Tracked; MAY proceed.
- [P4] nice-to-have / minor UX / stylistic preference. Never blocks. MAY proceed.
End with EXACTLY one line: PASS if there are NO P0/P1 findings, else FAIL.

VETO DISCIPLINE — these rules govern your ENTIRE review. Violating them is itself a review defect:
- REQUIREMENTS ARE THE HIGHEST AUTHORITY. The REQUIREMENTS block asks for specific technologies and
  capabilities. If the plan explicitly follows the requirements (e.g. it uses the mandated GUI stack,
  libs, and features), you MUST NOT FAIL it for those choices. Requirement-mandated choices are at most
  P3, never P0/P1. Do not contradict the requirements.
- NO SELF-CONTRADICTION. Do not reject a plan in this round for something a PRIOR review round told the
  builder to change, or for a requirement-mandated tradeoff the plan already documented with justification.
  If the plan already resolved an earlier finding, acknowledge it as resolved.
- SCOPE-LOCK. The plan defines PHASES. Only the phase currently under review (or the explicitly
  cross-cutting architecture) may carry P0/P1. Issues confined to a LATER phase are P3/P4 until that
  phase is reviewed. Do not invent blockers from unbuilt later-phase detail.
- SEVERITY HONESTY. P0/P1 = correctness, security, soundness, or a direct requirements conflict that
  will break the build. A preference for a different design, a style choice, or a deferred/deemed-later
  item is P3 or P4, never P0/P1. If the only remaining objections are P3/P4, your verdict is PASS.

PLAN TO REVIEW:
{{PLAN_CONTENT}}

GUIDELINES:
{{GUIDELINES_CONTENT}}

PONYTAIL RULES:
{{PONYTAIL_RULES}}

REQUIREMENTS:
{{REQUIREMENTS}}

CRITIC REVIEW HISTORY (your prior reviews of this plan across earlier rounds):
{{REVIEW_HISTORY}}
- If HISTORY is present: treat it as a record of what you ALREADY asked the builder to change.
  Acknowledge items the builder has resolved; do NOT re-raise the same item as a blocker, and do not
  contradict a prior ruling you already gave. Only NEW, not-yet-raised defects may become blockers.

SEVERITY DISCIPLINE — this is the SINGLE most important rule for keeping the review convergent:
- P1 means a CONCRETE, UNAMBIGUOUS defect that BREAKS the build or a REQUIREMENT acceptance criterion.
- The following are P2/P3 at most, NEVER P1 -- they do not block a plan from being approved:
    * "the plan does not fully justify why X"  -> P3 (completeness/writing improvement)
    * "Rust edition / MSRV / platform not declared" -> P3 (trivial to add, does not break anything)
    * "test names should be should-when" -> P3 (style)
    * "add a Why section" -> P3 (documentation)
    * "version mismatch with GUIDELINES" -> P3 (harmless to reconcile later)
    * any preference, refactor suggestion, or "you should explain more" -> P2/P3
- The 5-round review has a PURPOSE: converge to a plan a builder can execute. If a plan is
  architecturally sound and satisfies the requirements, and only has P2/P3 completeness/judgment
  items, your verdict is PASS and list those as P2/P3. Do NOT invent escalating P1s to force more
  rounds. Re-raising the same item you already flagged is a review defect.
- Genuine P1 (blocking) examples: a requirement is unaddressed, a security hole, a contradiction
  that would break implementation, an impossible design (e.g. REST SDK used where realtime is required).
- HARD RULE: if every P0/P1 you identified is a "missing declaration", "add justification", "be more
  explicit", an edition/platform-not-declared note, or any completeness/writing concern (NOT a concrete
  build/requirement-break defect), then your verdict MUST be PASS. Those are P2/P3, they do not block,
  and FAILing on them alone is a review defect that wastes the 5-round budget.

ACTIONABLE FEEDBACK — every P0/P1 finding MUST end with a concrete remediation on its own line:
  `-> FIX: <exactly what the builder should write/add/change, with specific detail>`
  Example: "-> FIX: add an 'Architecture Justification' section that states in 3-5 concrete points
  (a) why Rust over Go/C++/Zig, (b) why Tauri v2 over Electron (smaller binary/memory), (c) why hexagonal
  isolation, and (d) why egui for the treemap. Point each at the requirement it serves."
  A finding without a concrete `-> FIX:` remediation is itself a review defect. Vague rejections
  ("this is not justified enough") are not acceptable — tell the builder precisely what to add.

RESPECT-YOUR-OWN-FIX CONTRACT — this closes the convergence loop:
  Your `-> FIX:` line is the CONTRACT. If the current plan contains the change your FIX requested
  (even phrased differently, or not exactly to your preferred level of detail), mark that finding
  RESOLVED and do NOT re-raise it. Satisfying your stated FIX satisfies the finding. Do not reject a
  plan for "not justifying X well enough" when your prior FIX was "add a justification section" and one
  now exists. Only a NEW, genuinely-unraised defect may block — never a re-litigation of a satisfied FIX.
  Re-opening or escalating an item whose FIX is now present is a review defect.
PROMPT_EOF

# Build the actual prompt with substitutions
python3 -c "
import os, glob, json, sys

with open('/tmp/gate0_prompt.txt', 'r') as f:
    template = f.read()

plan = open('${PLAN_FILE}', 'r').read()
# Cap the plan so the request stays under OpenRouter's size limit (400 on large plans).
if len(plan) > 30000:
    plan = plan[:15000] + "\n...[truncated " + str(len(plan)-30000) + " bytes]...\n" + plan[-15000:]
guidelines = open('${GUIDELINES_FILE}', 'r').read()[:20000]

ponytail = ''
rules_dir = '${PROJ_ROOT}/.agents/rules'
if os.path.exists(rules_dir):
    for f in sorted(glob.glob(os.path.join(rules_dir, '*.yaml')) + glob.glob(os.path.join(rules_dir, '*.md')) + glob.glob(os.path.join(rules_dir, '*.txt')))[:5]:
        with open(os.path.join(rules_dir, f), 'r') as fh:
            content = fh.read()
            ponytail += content + '\n'
        ponytail = ponytail[:8000]

req = ''
if os.path.exists('${PROJ_ROOT}/requirements.md'):
    with open('${PROJ_ROOT}/requirements.md', 'r') as f:
        req = f.read()[:8000]

history = ''
history_file = '${HISTORY_FILE}'
if history_file and os.path.exists(history_file):
    with open(history_file, 'r') as f:
        history = f.read()[-6000:]

prompt = template.replace('{{PLAN_CONTENT}}', plan).replace('{{GUIDELINES_CONTENT}}', guidelines[:20000]).replace('{{PONYTAIL_RULES}}', ponytail).replace('{{REQUIREMENTS}}', req).replace('{{REVIEW_HISTORY}}', history)

with open('/tmp/gate0_prompt_final.txt', 'w') as f:
    f.write(prompt)
" 2>/dev/null || true

# Build JSON request using node
cat > /tmp/build_request.js << 'NODEEOF'
const fs = require('fs');
const prompt = fs.readFileSync('/tmp/gate0_prompt_final.txt', 'utf8');
const payload = {
  model: 'kimi-k2.7-code',
  messages: [{ role: 'user', content: prompt }],
  max_tokens: 64000,
  temperature: 1   // OpenCode Go reasoning models require temperature=1
};
fs.writeFileSync('/tmp/gate0_request.json', JSON.stringify(payload, null, 2));
NODEEOF

node /tmp/build_request.js

# POST to the critic endpoint (OpenCode Go)
curl -sS --max-time 600 \
  -X POST "${CRITIC_URL}" \
  -H "Authorization: Bearer ${CRITIC_API_KEY}" \
  -H "Content-Type: application/json" \
  -H "HTTP-Referer: http://localhost" \
  -H "X-Title: omp-agent-gate0" \
  --data @/tmp/gate0_request.json > /tmp/gate0_response.json

CURL_EC=$?
if [ $CURL_EC -ne 0 ]; then
  echo "xx Gate 0 request failed (curl exit $CURL_EC)"
  exit 1
fi

# Parse response — handle reasoning models that put output in message.reasoning.
# OpenCode Go can return TWO JSON docs concatenated (a chat completion + trailing
# extras). Parse the FIRST complete JSON object by trying each '}' as the end.
node -e "
const fs = require('fs');
const raw = fs.readFileSync('/tmp/gate0_response.json', 'utf8');
let d = null, parsed = false;
if (raw.trim().startsWith('{')) {
  for (let i = raw.indexOf('}'); i < raw.length; i = raw.indexOf('}', i + 1)) {
    try { d = JSON.parse(raw.slice(0, i + 1)); parsed = true; break; } catch(e) {}
  }
}
if (!parsed) { try { d = JSON.parse(raw); parsed = true; } catch(e) {} }
if (!parsed) { console.log('RESPONSE-ERROR: could not parse response JSON'); process.exit(1); }
if (d.error) { console.log('API-ERROR: ' + JSON.stringify(d.error)); process.exit(1); }
const msg = d.choices?.[0]?.message || {};
const content = msg.content || msg.reasoning || '';
const finish = d.choices?.[0]?.finish_reason || '';
if (!content && finish === 'length') {
  console.log('WARNING: Model exhausted tokens on reasoning without producing content (finish_reason=length).');
}
console.log(content);
fs.writeFileSync('/tmp/gate0_verdict.txt', content);
" > /tmp/gate0_verdict.txt 2>&1

RC=$?
if [ $RC -ne 0 ]; then
  echo "xx Could not parse critic response (see /tmp/gate0_response.json)."
  exit 1
fi

# Extract verdict
VERDICT=$(grep -iE '(^|[^A-Za-z])(PASS|FAIL)([^A-Za-z]|$)' /tmp/gate0_verdict.txt | tail -1 | grep -oEi 'PASS|FAIL' | tail -1 | tr '[:lower:]' '[:upper:]')

# Extract NON-BLOCKING findings (P2/P3/P4) into the shared notes file so the
# pipeline can surface them to the human. P0/P1 gate; P2+ are tracked, not blocking.
REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE:-/tmp/review_notes.txt}"
{
  echo ""
  echo "===== PLAN REVIEW NOTES ($(date +%H:%M:%S)) ====="
  grep -E '^\s*\[P[234]\]' /tmp/gate0_verdict.txt 2>/dev/null
} >> "${REVIEW_NOTES_FILE}" 2>/dev/null

# Append this round's verdict to the review history so the next round's critic
# can see what it already asked for (no self-contradiction from evidence).
{
  echo ""
  echo "===== ROUND VERDICT ($(date +%H:%M:%S)) ====="
  echo "VERDICT: ${VERDICT:-unknown}"
  cat /tmp/gate0_verdict.txt
} >> "${HISTORY_FILE}" 2>/dev/null

echo ""
echo "---- critic verdict ----"
cat /tmp/gate0_verdict.txt
echo "------------------------"

if [ "$VERDICT" = "FAIL" ]; then
  echo "xx GATE 0 FAILED — plan rejected. Builder must revise plan.md and re-run run-gates.sh."
  exit 1
elif [ "$VERDICT" = "PASS" ]; then
  echo "==> GATE 0 PASSED — plan approved. Proceeding to build."
  exit 0
else
  echo "xx No clear PASS/FAIL verdict found in critic output."
  exit 1
fi