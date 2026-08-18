#!/usr/bin/env bash
# GATE 2 — INDEPENDENT adversarial review via a DIFFERENT model.
# Calls OpenRouter directly (NO nested omp), so it's not subject to omp's
# per-command 120s timeout and is fully bulletproof as a separate process.
#
# Verdict: last non-empty line that is exactly "PASS" or "FAIL".
# Run from the project root, or pass a diff path as $1.
#
# CRITIC_STANDARD controls how strict the gate is:
#   production  -> any correctness/security/UX issue = FAIL (must fix).
#                  Accepts no tradeoffs. (DEFAULT)
#   mvp         -> real correctness/security bugs = FAIL; but design/UX/tooling
#                  gaps that are out-of-scope per the requirements become
#                  highlighted NOTES, not FAIL. The gate only blocks on genuine
#                  defects, so an MVP can converge (PASS-with-notes).
# Pass CRITIC_STANDARD=mvp (and CRITIC_REQUIREMENTS=requirements.md) to let the
# critic grade an MVP build without blocking on accepted tradeoffs.
set -uo pipefail
PROJ_ROOT="$(pwd)"

DIFF_FILE="${1:-/tmp/review.diff}"
# Load the project's .env — the ONLY env this pipeline reads (per project setup:
# keys live in the repo's .env, not in Hermes's file). This puts XIAOMI_API_KEY
# etc. into scope whether run via run-gates.sh or directly in omp.
if [ -f "$PROJ_ROOT/.env" ]; then
  set -a; # shellcheck disable=SC1091
  . "$PROJ_ROOT/.env"
  set +a
fi

# CRITIC_MODEL: the pipeline passes its choice as PIPELINE_CRITIC_MODEL (separate
# from CRITIC_MODEL to avoid collision with .env). .env may define CRITIC_MODEL
# (stale values like z-ai/glm-5.2). The pipeline prefix always wins.
CRITIC_MODEL="${PIPELINE_CRITIC_MODEL:-${CRITIC_MODEL:-mimo-v2.5-pro}}"
CRITIC_TIMEOUT="${CRITIC_TIMEOUT:-300}"
CRITIC_MAX_TOKENS="${CRITIC_MAX_TOKENS:-16000}"
CRITIC_STANDARD="${CRITIC_STANDARD:-production}"
CRITIC_REQUIREMENTS="${CRITIC_REQUIREMENTS:-requirements.md}"
REVIEW_HISTORY_FILE="${REVIEW_HISTORY_FILE:-/tmp/review_history.txt}"
REVIEW_LEDGER="${REVIEW_LEDGER:-/tmp/review_ledger.txt}"
CRITIC_URL="${CRITIC_URL:-https://token-plan-sgp.xiaomimimo.com/v1/chat/completions}"

echo "==> GATE 2: adversarial review (critic=${CRITIC_MODEL}, standard=${CRITIC_STANDARD}, timeout=${CRITIC_TIMEOUT}s, max_tokens=${CRITIC_MAX_TOKENS})"

# Xiaomi MiMo key — read from the environment (which we just loaded from .env).
# Do NOT fall back to ~/.hermes/.env: a cloned repo sets up its provider key in
# the project's .env, not in Hermes's file.
CRITIC_API_KEY="${XIAOMI_API_KEY:-${CRITIC_API_KEY:-}}"
if [ -z "$CRITIC_API_KEY" ]; then
  echo "xx XIAOMI_API_KEY not set. Add it to $PROJ_ROOT/.env with the Xiaomi key."
  exit 1
fi
[ "${REVIEW_DEBUG:-0}" = "1" ] && echo ">> using XIAOMI_API_KEY from the environment"

if [ ! -s "$DIFF_FILE" ]; then
  echo "!! No diff (${DIFF_FILE} empty). Pass a path to the diff."
  exit 1
fi
echo "Diff: $(wc -l < "$DIFF_FILE") lines ($(wc -c < "$DIFF_FILE") bytes)"

# Optional: load the requirements/scope into the review context so the critic
# knows what's in/out of scope (prevents FAILing on documented MVP exclusions).
REQ_TXT=""
if [ -n "$CRITIC_REQUIREMENTS" ] && [ -f "$CRITIC_REQUIREMENTS" ]; then
  REQ_TXT="$(head -c 8000 "$CRITIC_REQUIREMENTS")"
fi

# Build the JSON request with the review prompt + requirements + diff (max_tokens
# generous for reasoning models; not streamed).
node - "$DIFF_FILE" "$CRITIC_MODEL" "$CRITIC_STANDARD" "$CRITIC_REQUIREMENTS" "$CRITIC_MAX_TOKENS" "$REVIEW_HISTORY_FILE" "$REVIEW_LEDGER" <<'NODE' > /tmp/review_request.json
const fs=require('fs');
const [ , , diffFile, model, standard, reqFile, maxTokens, histFile, ledgerFile ]=process.argv;
// Cap the diff so the OpenRouter request stays under its size limit (large diffs
// across phases caused 400 "JSON parsing failed"). Head+tail keeps the start and end.
let diff=fs.readFileSync(diffFile,'utf8');
const MAX_DIFF=30000;
if (Buffer.byteLength(diff,'utf8')>MAX_DIFF){
  const head=diff.slice(0,Math.floor(MAX_DIFF/2));
  const tail=diff.slice(-Math.floor(MAX_DIFF/2));
  diff=head+'\n...[truncated '+(Buffer.byteLength(diff,'utf8')-MAX_DIFF)+' bytes]...\n'+tail;
}
let req='';
try { req=fs.readFileSync(reqFile,'utf8').slice(0,8000); } catch(e){}
// Load this critic's OWN prior verdicts (its review history) so it can see what
// it already asked for and avoid contradicting itself or re-raising settled items.
let history='';
try { history=fs.readFileSync(histFile,'utf8').slice(-6000); } catch(e){}
// Load the structured ISSUE LEDGER: each previously-raised finding with status.
let ledger='';
try { ledger=fs.readFileSync(ledgerFile,'utf8').slice(-4000); } catch(e){}
// Escape backticks and template-literal expressions in user-controlled content
// so they cannot break the template literal that builds the prompt.
const esc = s => (s||'').replace(/[`\\]/g, '\\$&').replace(/\$\{/g, '\\${');
const historyBlock = esc(history);
const reqBlock = esc(req);
const diffBlock = esc(diff);
const ledgerBlock = esc(ledger||'(no open issues yet)');
// scope-aware severity instruction
const scope = standard==='mvp'
  ? `GRADING (MVP standard): Use strict judgement. A real correctness or security defect MUST be FAIL.
     Design/UX/tooling issues that the PROJECT REQUIREMENTS explicitly list as out-of-scope are acceptable:
     list them under "NOTES (non-blocking)" and do NOT fail the gate for them.
     If there are no correctness/security defects worth blocking on, your final verdict is PASS (notes may follow).`
  : `GRADING (PRODUCTION standard): This is a production build. Any correctness, security, or UX defect is FAIL.
     No tradeoffs are accepted. Be strict.`;
const prompt=`You are an adversarial code reviewer. A builder produced the diff below.
${scope}

PROJECT REQUIREMENTS / SCOPE (in/out-of-scope context):
${reqBlock}

YOUR PRIOR REVIEWS OF THIS CODE (history - read and honor it):
${historyBlock||'(none yet)'}
- If HISTORY is present: it records what you ALREADY told the builder to change in
  earlier rounds. Treat resolved items as resolved; do NOT re-raise the same defect
  as a blocker, and do not contradict a ruling you already gave. Only NEW,
  not-yet-raised defects may become blockers.

ISSUE LEDGER (structured status of every previously-raised finding):
${ledgerBlock}
- Each ledger line is: [P<N>] STATUS <OPEN|RESOLVED|BACKLOG> <description>
- Re-flag ONLY items marked OPEN. An item marked RESOLVED is closed: do NOT re-raise
  it as a P0/P1 blocker unless the CURRENT diff shows a demonstrable regression of that
  exact fix.
- CONVERGENCE RULE: if the current code satisfies a fix you stated in an earlier round,
  acknowledge it as resolved and do NOT re-flag it. Do not let an ever-repeating concern
  block convergence when the described fix is present in the code.
- New P0/P1 findings this round are the only reasons to FAIL.

VETO DISCIPLINE - these rules govern your ENTIRE review:
- REQUIREMENTS ARE THE HIGHEST AUTHORITY. If the code follows a design the PROJECT
  REQUIREMENTS explicitly mandate (e.g. an extension-based file-type classifier,
  a specific mandated dependency or architecture), you MUST NOT FAIL it for that
  choice. A requirement-approved decision is at most a P3, never a P0/P1.
  Do not argue the code should use a different approach than the requirements specify.
- SCOPE. Review THIS diff (this phase's code). Do not fail it for later-phase or
  whole-project concerns outside what this diff adds or changes.

SEVERITY CLASSIFICATION - tag EVERY finding with exactly one of these (P0..P4):
- P0 = CRITICAL BLOCKER: security vulnerability, data loss/corruption, crash, or a
  clear violation of a REQUIREMENT acceptance criterion. Phase MUST NOT proceed.
- P1 = MAJOR BLOCKER: a real correctness or logic defect that breaks the build or
  a REQUIREMENT acceptance criterion. Phase MUST NOT proceed until fixed.
- P2 = should fix: a real correctness/robustness issue that does not currently
  break a requirement or the build, but could become P0/P1 if left. Tracked; the
  phase MAY proceed with it outstanding, but it must be resolved before final ship.
- P3 = good to fix: a refactor, clarity, or minor robustness improvement. Impacts
  quality but not correctness. Tracked; phase MAY proceed.
- P4 = nice-to-have / minor UX / stylistic preference / future-hardening idea.
  Never blocks. Tracked; phase MAY proceed.

A finding is P0/P1 ONLY if it is a genuine, concrete defect that breaks the build or
an acceptance criterion in THIS diff. A preference, refactor suggestion, or future
idea is P3 or P4 - never P0/P1. If a design choice is not contradicted by the
requirements, treat it as P3 at most.

Tag format: start each finding line with [P0], [P1], [P2], [P3], or [P4].
Every P0/P1 finding MUST end with a concrete remediation on its own line:
  -> FIX: exactly what to change in the code, with specific detail. Include the
     target file path, function signature, and the specific code structure to implement.
     For architectural changes, specify what to delete, what to replace it with, and
     how it connects to the rest of the code. Vague FIX lines ("replace X with Y")
     without the code structure are themselves a review defect.
  A P0/P1 without a FIX line is a review defect. Never just say "this is broken" -- tell the
  builder precisely what to change so the next round can satisfy it.

RESPECT-YOUR-OWN-FIX CONTRACT -- this closes the convergence loop:
  Your FIX line is the CONTRACT. If the current diff contains the change your FIX requested
  (even phrased slightly differently), mark that finding RESOLVED and do NOT re-raise it. Satisfying
  your stated FIX satisfies the finding. Do not escalate or re-flag an item whose FIX is now present
  just because you would now prefer a different implementation. Only a NEW, genuinely-unraised defect
  may block -- never a re-litigation of a satisfied FIX.

Review the diff harshly for correctness, logic, security, edge cases, and test coverage.
List findings sorted by severity (P0 first, then P1, P2, P3, P4).
Then list NON-BLOCKING NOTES (these are P3/P4 by definition).
End with a SINGLE final line that is PASS if there are NO P0 or P1 findings, else FAIL.
Verbatim: if any P0/P1 finding exists, the last line is exactly "FAIL". Otherwise it is exactly "PASS".

DIFF:
${diffBlock}`;
process.stdout.write(JSON.stringify({
  model,
  messages:[{role:'user',content:prompt}],
  max_tokens:Number(maxTokens||16000),
  temperature:0.2
}));
NODE

echo "Request bytes: $(wc -c < /tmp/review_request.json)"

# POST to the critic endpoint (OpenCode Go) with an explicit generous timeout.
curl -sS --max-time "${CRITIC_TIMEOUT}" \
  -X POST "$CRITIC_URL" \
  -H "Authorization: Bearer ${CRITIC_API_KEY}" \
  -H "Content-Type: application/json" \
  -H "HTTP-Referer: http://localhost" \
  -H "X-Title: omp-agent" \
  --data @/tmp/review_request.json > /tmp/review_response.json
CURL_EC=$?

if [ "$CURL_EC" -ne 0 ]; then
  echo "xx Critic request failed/aborted (curl exit ${CURL_EC}). Increase CRITIC_TIMEOUT if it timed out."
  exit 1
fi

# Extract the assistant content and print it (for the log + verdict).
# Tolerant of concatenated JSON (MiMo/SSE can return two JSON objects).
node - <<'NODE' > /tmp/review_verdict.txt
const fs=require('fs');
const raw=fs.readFileSync('/tmp/review_response.json','utf8');
let d=null, parsed=false;
if(raw.trim().startsWith('{')){
  for(let i=raw.indexOf('}'); i<raw.length; i=raw.indexOf('}',i+1)){
    try{ d=JSON.parse(raw.slice(0,i+1)); parsed=true; break; }catch(e){}
  }
}
if(!parsed){ try{ d=JSON.parse(raw); parsed=true; }catch(e){} }
if(!parsed){ console.log('RESPONSE-ERROR: could not parse response JSON'); process.exit(1); }
if(d.error){ console.log('API-ERROR: '+JSON.stringify(d.error)); process.exit(1); }
const content=(d.choices?.[0]?.message?.content)||'(empty)';
console.log(content);
NODE
RC=$?

if [ "$RC" -ne 0 ]; then
  echo "xx Could not parse critic response (see /tmp/review_response.json)."
  exit 1
fi

# Verdict: scan the LAST occurrence of a line containing a bare PASS/FAIL token.
# Loosen so `PASS`, `PASS.`, `**PASS**`, "PASS (see notes)" all match — but anchor on
# the final line(s) so a mid-text "FAIL" mention doesn't override a final PASS.
# Only a token that is PASS or FAIL (case-insensitive) counts; nothing else.
VERDICT="$(grep -iE '(^|[^A-Za-z])(PASS|FAIL)([^A-Za-z]|$)' /tmp/review_verdict.txt | tail -1 | grep -oEi 'PASS|FAIL' | tail -1 | tr '[:lower:]' '[:upper:]')"

# Extract NON-BLOCKING findings (P2/P3/P4) into a dedicated file the pipeline
# collects and surfaces to the human at the end. P0/P1 gate the phase; P2+ do not.
# The gate writes to ${REVIEW_NOTES_FILE} (default /tmp/review_notes.txt).
REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE:-/tmp/review_notes.txt}"
{
  echo ""
  echo "===== PHASE REVIEW NOTES ($(date +%H:%M:%S)) ====="
  # Lines tagged [P2]/[P3]/[P4] are the non-blocking findings to surface.
  grep -E '^\s*\[P[234]\]' /tmp/review_verdict.txt 2>/dev/null
} >> "${REVIEW_NOTES_FILE}" 2>/dev/null

# Append this round's verdict to the review history so the NEXT round's critic
# sees what it already asked for (no self-contradiction / no re-raising settled items).
{
  echo ""
  echo "===== ROUND VERDICT ($(date +%H:%M:%S)) ====="
  echo "VERDICT: ${VERDICT:-unknown}"
  cat /tmp/review_verdict.txt
} >> "${REVIEW_HISTORY_FILE}" 2>/dev/null

# ── Update the structured ISSUE LEDGER ────────────────────────────────────────
# Each finding is keyed by a STABLE SYMBOL (e.g. `test_incremental_scan`, a fn or
# struct name) so re-phrasings of the same issue match. This breaks the
# "re-raise forever" deadlock: fixed items become RESOLVED and are not re-flagged.
# Logic lives in tests/lib/review_ledger.py (single source of truth, self-tested).
REVIEW_LEDGER="${REVIEW_LEDGER:-/tmp/review_ledger.txt}"
python3 "$PROJ_ROOT/tests/lib/review_ledger.py" "$REVIEW_LEDGER" /tmp/review_verdict.txt

echo "---- critic verdict ----"
echo "(last verdict token: ${VERDICT:-<none>})"
echo "------------------------"

if [ "$VERDICT" = "FAIL" ]; then
  echo "==> GATE 2: FAIL — blocking findings fed back to builder (see /tmp/review_verdict.txt)."
  exit 1
fi
if [ "$VERDICT" = "PASS" ]; then
  echo "==> GATE 2: PASS${CRITIC_STANDARD:+, standard=${CRITIC_STANDARD}}. Non-blocking notes, if any, are in /tmp/review_verdict.txt."
  exit 0
fi
echo "xx No clear PASS/FAIL verdict found in critic output."
exit 1
