# Pipeline Audit — 2026-08-17

## Data Flow Map

```
Plan Phase:
  pipeline.sh → gate0_plan_review.sh
    reads: plan.md, GUIDELINES.md, .env (XIAOMI_API_KEY)
    writes: gate0_prompt_final.txt → gate0_request.json → curl → gate0_response.json → gate0_verdict.txt
    returns to: pipeline.sh (PASS/FAIL → builder revises or proceed)

Build Phase (per phase, per round):
  pipeline.sh extracts deliverables from plan.md
  → run_omp (builder) per deliverable
  → git add + commit
  → gate.sh (tests) → gate2 review_gate.sh (critic)
    reads: /tmp/phase.diff, .env (XIAOMI_API_KEY), PIPELINE_CRITIC_MODEL
    writes: review_request.json → curl → review_response.json → review_verdict.txt
    returns to: pipeline.sh (PASS → commit, FAIL → fix round)
```

## Verified ✅

1. **PIPELINE_CRITIC_MODEL overrides .env** — gate2 loads .env THEN sets CRITIC_MODEL from PIPELINE_CRITIC_MODEL (line 34). No collision.
2. **gate0 hardcodes CRITIC_MODEL after .env** — line 36: `CRITIC_MODEL="mimo-v2.5-pro"`. .env's stale value is overridden.
3. **gate0 prompt_final.txt rebuilt every run** — fsync'd, no || true swallow. Stale-plan bug fixed.
4. **gate0 reads current plan.md** — each invocation opens plan.md fresh.
5. **gate0 parser tolerates concatenated JSON** — first-valid-JSON parsing (line 329).
6. **extract_phases** — truncates after number, no word-splitting.
7. **extract_deliverables** — matches ## and ### headings, optional space between Phase and number, skips Gate lines, breaks only at phase headings.
8. **Sub-chunk cap** — MAX_SUBCHUNKS=5, batches if more.
9. **Interaction logging** — hashes, verdicts, P1 counts all recorded.
10. **19/19 self-tests pass** — covers extract_phases, verdict parsing, notes extraction, issue ledger, gate0 prompt freshness, docker install commands.

## Gaps to Fix ⚠️

1. **gate2 parser NOT tolerant of concatenated JSON** — gate0 has first-JSON parsing, gate2 still uses raw `JSON.parse(fs.readFileSync(...))`. If MiMo returns concatenated JSON (like kimi did), gate2 crashes.
   - **Fix**: apply same first-JSON parser to gate2.
2. **gate0 .env loaded twice** — lines 14-16 and again at line 33-34. Harmless but untidy.
   - **Fix**: remove duplicate.
3. **gate0 doesn't use PIPELINE_CRITIC_MODEL** — hardcodes the model. If pipeline wants to override gate0's model, it can't.
   - **Fix**: use PIPELINE_CRITIC_MODEL with fallback, like gate2 does.
4. **Fix-round path may exit on set -e** — v23 build exited when fix-round's run_omp returned non-zero.
   - **Fix**: wrap fix-round run_omp in `set +e ... set -e` or handle explicitly.

## Architectural Risks 🔶

1. **Same-model critic leniency** — builder and critic use the same MiMo model. Phase 1 passed trivially. Complex phases may slip through. No immediate fix (only one provider available). Mitigate by monitoring Phase 2+ critic findings for quality.
2. **MAX_SUBCHUNKS=5 may batch too aggressively** — plans with 16+ granular deliverables (every function/test) get batched into one focused call. If the batch is too large, it hits OMP_TIMEOUT.
   - **Mitigate**: make MAX_SUBCHUNKS configurable via env var.
3. **OMP_TIMEOUT=900s** — MiMo can take >15 min for complex batched phases. Sub-chunking helps but doesn't eliminate this.
   - **Mitigate**: accept as constraint; sub-chunking reduces frequency.
4. **API throttling** — Xiaomi/OpenCode Go may throttle rapid critic calls (403s). Each phase has 1-2 critic calls, so risk is low per phase but cumulative.
   - **Mitigate**: accept; retry logic in curl handles transient failures.
5. **Builder code quality** — MiMo produced 58 clippy/doc errors in Phase 1. The fix-round mechanism exists but the pipeline may exit before it runs (gap #4).
   - **Fix**: address gap #4 (set -e exit handling) so fix rounds actually execute.
