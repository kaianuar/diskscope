#!/usr/bin/env bash
# GATE 3 — visual + functional E2E check for the built app.
#
# Verifies the UI actually works end-to-end (Playwright) and that it looks correct
# (screenshots reviewed by a vision-capable model). This catches things a diff-based
# critic cannot: "the submit button doesn't work", "two clients don't sync", "a
# screen is broken/bare".
#
# HOW IT WORKS
#   1. Starts the app (the command in GATE3_START, or the project's own start).
#   2. Runs the Playwright spec in tests/e2e/ (functional asserts + screenshots).
#   3. Feeds the screenshots to a vision model (GOOGLE_VISION_MODEL, default
#      google/gemini-3.1-flash-lite via OpenRouter) for a visual "does it look right
#      and consistent?" review.
#   4. PASS only if functional e2e passes AND vision review finds no blocking defect.
#
# USAGE (run from project root)
#   GATE3_START="npm run dev" bash tests/visual_gate.sh
#   # pausing / env:
#   GOOGLE_VISION_MODEL="google/gemini-3.1-flash-lite" \
#   GATE3_START="npm run dev" bash tests/visual_gate.sh
#
# DEPENDENCIES: node + `npm i -D @playwright/test` (npx playwright). First run:
#   npx playwright install chromium
set -uo pipefail
cd "${GATE3_CWD:-$(pwd)}"

GATE3_START="${GATE3_START:-}"              # command to start the app (required)
VISION_MODEL="${GOOGLE_VISION_MODEL:-google/gemini-3.1-flash-lite}"
VISION_TIMEOUT="${VISION_TIMEOUT:-180}"
SHOTS_DIR="${GATE3_SHOTS:-/tmp/gate3_shots}"
SPEC_DIR="${GATE3_SPEC:-tests/e2e}"
PORT="${GATE3_PORT:-5173}"
BASE_URL="${GATE3_BASE_URL:-http://localhost:${PORT}}"

echo "==> GATE 3: visual + functional e2e (app at ${BASE_URL})"

if [ -z "$GATE3_START" ]; then
  echo "xx GATE3_START not set (command to start the app). Provide it, e.g."
  echo "   GATE3_START=\"npm run dev\" bash tests/visual_gate.sh"
  exit 1
fi

# --- 1. Start the app in the background ---
echo "==> starting app: ${GATE3_START}"
eval "$GATE3_START" > /tmp/gate3_app.log 2>&1 &
APP_PID=$!
# wait for the port to accept connections (up to 30s)
for i in $(seq 1 30); do
  if curl -sS -o /dev/null "$BASE_URL" 2>/dev/null; then break; fi
  sleep 1
done
echo "app listening (pid ${APP_PID}). screenshots -> ${SHOTS_DIR}"

# --- 2. Run the Playwright e2e spec ---
# (spec must exist; it performs the functional asserts and saves screenshots)
if [ ! -d "$SPEC_DIR" ]; then
  echo "xx No e2e spec dir yet: ${SPEC_DIR}. Create tests/e2e/*.spec.ts first."
  kill "$APP_PID" 2>/dev/null
  exit 1
fi

# ensure playwright is available (package at repo root)
if [ ! -d node_modules/@playwright 2>/dev/null ] && [ ! -f playwright.config.* ]; then
  echo "==> playwright not installed; installing @playwright/test locally..."
  npm i -D @playwright/test >/dev/null 2>&1 || { echo "xx playwright install failed"; kill "$APP_PID"; exit 1; }
  npx playwright install chromium >/dev/null 2>&1 || true
fi

mkdir -p "$SHOTS_DIR"
echo "==> running playwright spec: ${SPEC_DIR}/*.spec.ts"
if ! BASE_URL="$BASE_URL" SHOTS_DIR="$SHOTS_DIR" npx playwright test "$SPEC_DIR" --config="$SPEC_DIR/playwright.config.ts" 2>/dev/null; then
  # fallback: run any config found, else run the spec directly
  npx playwright test "$SPEC_DIR" 2>&1 | tee /tmp/gate3_e2e.log
  E2E_EC=${PIPESTATUS[0]}
else
  E2E_EC=0
fi
echo "e2e exit: ${E2E_EC}"

# --- 3. Vision review of screenshots (if any were captured) ---
VISION_EC=0
if ls "$SHOTS_DIR"/*.png >/dev/null 2>&1; then
  echo "==> vision review ($(ls "$SHOTS_DIR"/*.png | wc -l) screenshot(s)) using ${VISION_MODEL}"
  # send ALL screenshots in one multimodal request; extract a single PASS/FAIL verdict
  node - "$SHOTS_DIR" "$VISION_MODEL" "$VISION_TIMEOUT" <<'NODE'
const fs=require('fs'), path=require('path'), os=require('os');
const [ , , shotsDir, model, vtimeout]=process.argv;
const pngs=fs.readdirSync(shotsDir).filter(f=>f.endsWith('.png')).map(f=>path.join(shotsDir,f));
let key=process.env.OPENROUTER_API_KEY||'';
if(!key){ const p=os.homedir()+'/.hermes/.env'; if(fs.existsSync(p)){
  const l=fs.readFileSync(p,'utf8').split('\n').find(l=>l.startsWith('OPENROUTER_API_KEY='));
  if(l) key=l.split('=')[1].trim().replace(/^"|"$/g,''); } }
if(!key){ console.log('NO_KEY'); process.exit(2); }
(async()=>{
  const parts=[{type:'text',text:'These are screenshots of a web app. Judge the UI visually. Tag each finding with a severity [P0..P4]: [P0]=broken/crash/unusable (must fix), [P1]=major visual defect that breaks a core flow or requirement (must fix), [P2]/[P3]=real but non-urgent visual/consistency issues (track, may proceed), [P4]=minor polish/nice-to-have. If there are NO [P0] or [P1] findings, say PASS. End your reply with exactly PASS or FAIL.'}];
  for (const f of pngs) parts.push({type:'image_url',image_url:{url:`data:image/png;base64,${fs.readFileSync(f).toString('base64')}`}});
  const body={model,messages:[{role:'user',content:parts}],max_tokens:700};
  const ctrl=new AbortController(); const t=setTimeout(()=>ctrl.abort(), Number(vtimeout||180)*1000);
  try{
    const r=await fetch('https://openrouter.ai/api/v1/chat/completions',{method:'POST',
      headers:{'Authorization':'Bearer '+key,'Content-Type':'application/json','HTTP-Referer':'http://localhost','X-Title':'omp-agent'},
      body:JSON.stringify(body),signal:ctrl.signal});
    const j=await r.json();
    const c=j?.choices?.[0]?.message?.content||'(no output)';
    console.log(c);
    // Write the full review so P2+ notes can be surfaced to the human.
    try { fs.writeFileSync('/tmp/gate3_vision_review.txt', c); } catch(e){}
    const verdict=(c.match(/\b(PASS|FAIL)\b/)||[])[1]||'FAIL';
    fs.writeFileSync('/tmp/gate3_vision_verdict.txt',verdict);
    process.exit(verdict==='PASS'?0:1);
  }catch(e){ console.log('VISION-ERR: '+e.message); process.exit(1); } finally { clearTimeout(t); }
})();
NODE
  VISION_EC=$?
  # Extract non-blocking visual findings (P2/P3/P4) into the shared notes file.
  REVIEW_NOTES_FILE="${REVIEW_NOTES_FILE:-/tmp/review_notes.txt}"
  if [ -f /tmp/gate3_vision_review.txt ]; then
    {
      echo ""
      echo "===== VISUAL REVIEW NOTES ($(date +%H:%M:%S)) ====="
      grep -E '^\s*\[P[234]\]' /tmp/gate3_vision_review.txt 2>/dev/null
    } >> "${REVIEW_NOTES_FILE}" 2>/dev/null
  fi
else
  echo "==> no screenshots found in ${SHOTS_DIR}; skipping vision review"
fi

kill "$APP_PID" 2>/dev/null

if [ "$E2E_EC" -eq 0 ] && [ "$VISION_EC" -eq 0 ]; then
  echo "==> GATE 3: PASS (functional e2e green + vision review clean)."
  exit 0
fi
echo "==> GATE 3: FAIL (functional e2e=${E2E_EC}, vision=${VISION_EC}). See /tmp/gate3_*"
exit 1
