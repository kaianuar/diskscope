# PIPELINE — the operating loop omp follows

This file defines HOW you work in this repo. Load it as context on every run.
There is one overall goal (in `requirements.md`). You drive it to completion
following this exact loop. If anything is ambiguous, stop and ask.

## Roles (use omp subagents, different models via modelRoles)

- **builder** — implements code + tests. The default model (e.g. deepseek-v4-flash).
  Works in an isolated git worktree. Owns producing a working, test-passing diff.
- **critic** — adversarially reviews the builder's diff. Use a DIFFERENT model
  than builder via omp's `modelRoles` / `--model` override (e.g. deepseek-v4-pro
  or kimi / GLM). Fresh context. Harsh, concrete, never lenient. Returns PASS or
  a list of specific problems.
- **you (me)** — the human. Final approval at the steer checkpoint.

## omp configuration reference (verified)

- Global settings: `~/.omp/agent/config.yml` (`modelRoles: {smol, slow, plan, commit}`).
- Project-local overrides: `<cwd>/.omp/config.yml` (loaded when `.omp/` exists).
- CLI model flags: `--model <default>`, `--smol`, `--slow`, `--plan`.
- Critic with a different model: launch with `--model <model B>` (or set a
  dedicated role in `modelRoles` and invoke via that role).
- Non-interactive automation: `omp --mode rpc` (JSONL protocol over stdio).
- Plan/steer gate: `--plan` mode forces a planning step before code.


## The loop (in order, no skipping)

1. **PLAN (plan mode).** Read `requirements.md`. Break the goal into concrete
   work: build / test / design. Show me the plan BEFORE any code is written.
   Do not start implementing without my OK on the plan.

2. **BUILD INCREMENTALLY (commit after each green unit).** Break the work into
   small logical units (a feature, a screen, a fix). For EACH unit:
   a. Implement that unit + its tests (following `design-system/` UI rules).
   b. Run that unit's tests (or `tests/gate.sh`) — must go green.
   c. Make a SINGLE, meaningful commit: `git add` the unit's files and commit with
      a short message ("feat: add task add/toggle UI", "fix: toggle 400 on no-body PATCH").
   DO NOT accumulate one giant uncommitted blob. The git history should read as a
   logical sequence of small commits, so the final diff in the steer step is
   tiny/understood, not a 30-file wall.

3. **HARD QUALITY GATE — `run-gates.sh` (non-skippable).** After the incremental
   build, run `./run-gates.sh`. It runs:
   - **GATE 1 — TESTS:** `tests/gate.sh` auto-detects the stack(s) and runs the
     matching test command(s) — including BOTH frontend and backend for a full-stack
     project (e.g. `client/` + `server/`). Must exit 0 (all suites green).
   - **GATE 2 — ADVERSARIAL REVIEW:** `tests/review_gate.sh` runs a DIFFERENT model
     (the critic) on the diff. Must pass. Use `CRITIC_STANDARD=mvp` for an MVP build
     so out-of-scope tradeoffs become notes, not FAIL — but real correctness/security
     bugs still block.
   - **MANDATORY:** `run-gates.sh` MUST exit 0 before ANY steer/commit of the final
     state. If it fails, the builder fixes and re-runs (or only the specific failing
     unit is revisited — keep uncommitted work minimal).
   - Never proceed past a red gate. Never edit a test to make it pass unless the
     test itself is wrong AND you can justify it. Do NOT skip Gate 2 — a self-review
     by the builder is NOT a substitute; it must be a different model.
   - **Do NOT review the code yourself.** The adversarial review must be done by the
     SEPARATE critic process that `tests/review_gate.sh` launches. Run `./run-gates.sh`
     and let the independent critic do the review; do not improvise a self-review.
   - Do not hand-edit a test command into `tests/gate.sh` — let it auto-detect. If a
     suite isn't detected, tell me rather than forcing one.

3b. **GATE 3 — VISUAL + FUNCTIONAL E2E (if the app has a UI).** When a UI exists,
   also run `tests/visual_gate.sh`:
   - Start the app, run Playwright e2e (`tests/e2e/`) that asserts core flows work
     (add / toggle / delete / share / sync) AND captures screenshots.
   - A vision model (`google/gemini-3.1-flash-lite`) reviews the screenshots for
     visual correctness and consistency.
   - Functional e2e must pass. If the vision reviewer finds blocking visual defects,
     fix them. (UI-only concern; skip if the deliverable has no UI.)

4. **STEER CHECKPOINT.** Stop. Because you committed incrementally, this diff is a
   SHORT, readable summary of the recent commits — not a giant blob. Show me the git
   log / recent diff + a 2-line summary. I approve (then you may push/finalize) or
   redirect you. Do NOT force-push or rewrite history without asking.

   **Before this checkpoint, replace `README.md`** so it describes THE APP you built
   (what it does, who it's for, how to run it, its stack) — NOT the omp-agent pipeline.
   The scaffold ships a placeholder README; your job is to write the real one for the
   finished app. The pipeline docs (PIPELINE.md / USAGE.md / CONFIG.md) stay as-is and
   are not the app's README.

## UI rules (loaded from design-system/)

- Every screen/component MUST consume tokens from `design-system/tokens.json`.
- No hard-coded colors/spacing/typography outside the token file.
- Do not invent a visual style — follow the seeded design direction. If the goal
  needs unique assets, source them via the configured image-gen path and wire them
  in (do not fabricate a unique look from scratch).

## Hard rules
- Never trust the model's self-reported pass. GATE 1 runs real tests; only a
  green `gate.sh` counts as green.
- One logical change per commit. Commit incrementally.
- **Keep `.gitignore` current.** As you build, add any generated/runtime artifact to
  `.gitignore` — databases (`*.db`, `*-wal`, `*-shm`), build output (`node_modules/`,
  `dist/`), logs, secrets, QA screenshots if you don't want them committed. NEVER
  commit live database files, lock files, or secrets. Check `git status` before a
  commit: if runtime state is staged, `.gitignore` it first.
- If you're unsure or blocked > threshold, stop and ask me rather than guessing.
- Keep the diff minimal and readable.
