# GRID Launcher — Claude Code instructions

@AGENTS.md

## Orchestration and Delegation

For complex, multi-step requests, the main session acts as orchestrator: break the request into ordered steps and delegate to the specialist agents below. Coordinate the work rather than doing everything inline.

### Agents

- **planner** — Creates implementation strategies and technical plans
- **coder** — Writes code, fixes bugs, implements logic (Rust, TypeScript, Svelte)
- **designer** — Creates UI/UX, styling, visual design
- **api-tester** — Validates behaviour against the live RomM server using read-only API GET requests. Produces a PASS/FAIL/WARN report per endpoint tested.
- **doc-research** — Parses local documents and does web research/documentation lookups. Returns a structured report; does NOT modify the repository.
- **Explore** — Fast read-only codebase Q&A. Use instead of chaining multiple search/read calls when researching the codebase.

### Delegation Rules

- UI/UX design tasks go to **designer**; code implementation goes to **coder**; planning goes to **planner**. Never assign UI/UX design tasks to the coder, or implementation tasks to the designer.
- When a task requires external information, documentation, library references, or web research, call **doc-research** BEFORE handing off to the planner or coder. Pass its report forward as context in the next delegation.
- When delegating, describe **WHAT** outcome is needed, not **HOW** to implement it, and include the files each step may touch so the step has clear scope.
- If two steps' scopes overlap or are uncertain, run them sequentially — don't let both modify the same files concurrently. Independent read-only work (research, exploration) may run in parallel.
- One implementer at a time in a shared working tree. Every agent that commits uses `git commit --only <paths>`.

### Workflow for non-trivial changes

1. **Plan first**: for anything beyond a single-file edit or a clear, unambiguous bug fix, get a plan from **planner** before implementing (see the `planning` skill for when planning is required).
2. **Execute in dependency order**, verifying each step's result against the repo state before moving on.
3. **Run the gate** after implementation (see below).
4. **API verification**: after the gate passes on changes affecting server communication, call **api-tester** to run live checks for affected endpoints (classification rules in the `api-verification` skill). If any check FAILs, delegate a fix to **coder**, then re-run the gate and the API verification before finishing.
5. **Verify and report**: confirm the final state and summarize the outcome.

## The gate

These are exactly the commands `.github/workflows/build.yml`'s `check` job runs, in its
order. All must pass before work counts as done.

    scripts/check_secret_hygiene.sh
    cargo fmt --check
    cd app && npm ci && npx svelte-check && npm run build
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p app --all-targets --features e2e -- -D warnings
    cargo test --workspace
    cd app && npm test

Run the end-to-end suite (`scripts/e2e.sh`, or a named stage group) when a change touches
a user-visible flow. It is slow, so scope it: `E2E_SKIP_BUILD=1 scripts/e2e.sh <group>` is
the inner loop, and it is safe only when nothing under `app/src` or the Rust crates has
changed since the last `e2e.sh` build. `BUILD.md` documents the runner.
