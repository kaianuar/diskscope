# Model Configuration — the repo author's validated setup (2026-08-15)

> These are THIS repo's defaults, validated live against the author's providers.
> **You do NOT need these exact models** — the pipeline is model-agnostic. Replace
> with your own via `.omp/config.yml` (roles) and `tests/review_gate.sh` (critic).
> See README → *Configure your own models & providers* for how. The RULES below
> (builder≠critic, max_tokens, real-tests) apply regardless of which models you pick.

## Provider status

| Provider | Status | Notes |
|---|---|---|
| Xiaomi MiMo | ✅ working | builder + plan; key in ~/.hermes/.env |
| OpenRouter | ✅ working (key set) | critic + all fallbacks; key in ~/.hermes/.env |
| OpenCode Go | ⚠️ throttles (403 under load) | avoid as hot-path critic |
| DeepSeek dedicated | ⚠️ price rose | use OpenRouter instead |
| ollama-cloud | ❌ EXPIRED | removed |

## Primary roles (validated, run-the-code)

| Role | Model | Provider | In/M | Status |
|---|---|---|---|---|
| builder (default) | xiaomi-token-plan-sgp/mimo-v2.5-pro | Xiaomi MiMo | sub | ✅ PASS |
| plan | xiaomi-token-plan-sgp/mimo-v2.5 | Xiaomi MiMo | | ✅ PASS |
| critic (Gate 2) | z-ai/glm-5.2 | OpenRouter | $0.49 | ✅ PASS |

## Fallback matrix (ALL dedupe-probe tested 2026-08-15)

### Critic fallbacks (distinct from MiMo builder)
| Rank | Model | In/M | Result |
|---|---|---|---|
| 1 | qwen/qwen3-coder-30b-a3b-instruct | $0.070 | ✅ PASS — cheapest strong |
| 2 | minimax/minimax-m3 | $0.300 | ✅ PASS — cheapest of kimi/minimax tier, input-cheap ≥ good for review |
| 3 | moonshotai/kimi-k2.7-code | $0.710 | ✅ PASS — code-tuned, pricier |
| ✗ | moonshotai/kimi-k2.6 | $0.650 | ❌ FAIL — empty output on OpenRouter, DO NOT use |
| — | deepseek/deepseek-v4-flash | $0.077 | (validated in AlexQu pool; not re-run here) |

### Builder fallbacks
| Rank | Model | In/M | Result |
|---|---|---|---|
| 1 | qwen/qwen3-coder-30b-a3b-instruct | $0.070 | ✅ PASS |
| 2 | deepseek/deepseek-v4-flash | $0.077 | validated |

### Free fallbacks (rate-limited, light use only)
qwen/qwen3-coder-30b-a3b-instruct:free, deepseek/deepseek-v4-flash:free,
z-ai/glm-4.7-flash:free, openai/gpt-oss-120b:free

## Critical implementation rules (VALIDATED)
1. **Reasoning models return EMPTY if max_tokens too low** — max_tokens is a CEILING
   (model self-terminates, does NOT waste tokens on short answers). Set it generously
   (>= 4000) so reasoning never starves the answer.
2. **Builder MUST differ from critic** (cross-model adversarial review). Don't set equal.
3. **kimi-k2.6 is NOT usable on OpenRouter** (empty output) despite being in the local pool.
4. **OpenCode Go throttles** — avoid as hot-path critic.

## Optional cost lever (user decision, not a default)
Some reasoning-capable models accept a low-`thinking`/`reasoning_effort` setting that
sharply cuts completion tokens. Whether it's supported and how much it helps depends
on the model + provider, so it's NOT baked into the pipeline — it's a user choice.
(Author's note: verified ~85% completion-token cut on z-ai/glm-5.2 with
`{"thinking":{"type":"low"}}` and no quality loss, in a short code probe. Re-verify on
your own models before relying on it.)

## Known improvement (tracked — GATE-2 critic context + criteria)
**Issue (escalated 2026-08-15):** the adversarial critic in `tests/review_gate.sh` is passed ONLY the git diff
and NO requirements / acceptance criteria. Two consequences:
1. **Context-starved for larger codebases** — it can't see module boundaries, imports, or cross-file ripple
   effects; only patch hunks. Worked for a small 2-service app, under-powered for real repos.
2. **Gates on out-of-scope opinions** — without the requirements, the critic FAILs on design tradeoffs the spec
   explicitly excluded (observed at round 7 of family-todo: "no password login / CORS / no-auth" are required-MVP
   exclusions, yet the critic gate-blocked on them). A hard gate must not fail on out-of-scope items.

**Planned fix (not yet implemented):** `review_gate.sh` should pass to the critic, alongside the diff:
   (a) the **requirements + acceptance criteria** (so it knows what's in/out of scope),
   (b) a **severity rubric** (correctness/security = gate; design/UX-optional = note, don't block),
   (c) a **structural view** of changed files (file tree + key signatures/exports) for real context.
Tracked in `mnemosyne` task `omp-agent-enhancement`. Do this before relying on Gate 2 for non-trivial projects.

## How to switch
- Critic: CRITIC_MODEL env or tests/review_gate.sh default.
- Builder: modelRoles in .omp/config.yml.
- Re-validate monthly / when subs or prices change.
