# GRID Launcher — Claude Code instructions

@AGENTS.md

## Orchestration and Delegation

For complex, multi-step requests, the main session acts as orchestrator: break the request into ordered steps and delegate to the specialist agents below. Coordinate the work rather than doing everything inline.

### Agents

- **planner** — Creates implementation strategies and technical plans
- **coder** — Writes code, fixes bugs, implements logic
- **designer** — Creates UI/UX, styling, visual design
- **api-tester** — Validates behaviour against the live RomM server using read-only API GET requests. Produces a PASS/FAIL/WARN report per endpoint tested.
- **doc-research** — Parses local documents and does web research/documentation lookups. Returns a structured report; does NOT modify the repository.
- **deep-diagnostics** — Deep Python root-cause analysis for non-obvious or intermittent failures, when standard debugging has failed.
- **Explore** — Fast read-only codebase Q&A. Use instead of chaining multiple search/read calls when researching the codebase.

### Delegation Rules

- UI/UX design tasks go to **designer**; code implementation goes to **coder**; planning goes to **planner**. Never assign UI/UX design tasks to the coder, or implementation tasks to the designer.
- When a task requires external information, documentation, library references, or web research, call **doc-research** BEFORE handing off to the planner or coder. Pass its report forward as context in the next delegation.
- When delegating, describe **WHAT** outcome is needed, not **HOW** to implement it, and include the files each step may touch so the step has clear scope.
- If two steps' scopes overlap or are uncertain, run them sequentially — don't let both modify the same files concurrently. Independent read-only work (research, exploration) may run in parallel.

### Workflow for non-trivial changes

1. **Plan first**: for anything beyond a single-file edit or a clear, unambiguous bug fix, get a plan from **planner** before implementing (see the `planning` skill for when planning is required).
2. **Execute in dependency order**, verifying each step's result against the repo state before moving on.
3. **Run unit tests** after implementation (`python -m unittest discover tests/` — see the `testing` skill).
4. **API verification**: after unit tests pass on changes affecting server communication, call **api-tester** to run live checks for affected endpoints (classification rules in the `api-verification` skill). If any check FAILs, delegate a fix to **coder** and re-run tests and API verification before finishing.
5. **Verify and report**: confirm the final state and summarize the outcome.
