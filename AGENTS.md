# Agents follow these rules

## Where to look

- `SPEC.md` — product behavior and UX intent. Read it when implementing a feature,
  changing UI/UX behavior, or resolving an ambiguous product question. Do not read it for
  bug fixes, test-only changes, or edits inside already-implemented logic.
- `ARCHITECTURE.md` — module ownership. Read it when you need to decide which module owns
  a behavior or which file a non-trivial change belongs in. The dependency direction is
  one-way: `app/src` → Tauri commands (`app/src-tauri/src`) → `crates/grid-core`.
  grid-core must never depend on Tauri.
- `openapi.json` — read it before writing or changing any code that constructs, sends, or
  parses a RomM server request (`crates/grid-core/src/romm/`).
- `BUILD.md` — prerequisites, the development loop, the gate commands, the end-to-end
  runner, and the release process.

## Tests

- Rust: `#[cfg(test)]` modules beside the code, plus integration tests in
  `crates/grid-core/tests/`. Test the pure function, not the wrapper.
- TypeScript: vitest, in a `.test.ts` beside the module. Put logic in a pure `.ts` module
  so it can be tested without mounting a component.
- Flows: the WebdriverIO suite in `e2e/` (`scripts/e2e.sh`). Use it for behavior that only
  exists end to end — a real binary, a real keyring, a mock RomM server.

## Hard rules

- **Secrets.** Tokens and passwords live only in the OS keyring and in redacting in-memory
  types (`SecretString` / `Credential`). They must never appear in a config file, a log
  line at any level, an error message, an IPC payload, a test fixture, or console output.
  `scripts/check_secret_hygiene.sh` is part of the gate.
- **Never destroy work.** Do not run `git checkout`, `git restore`, `git reset`, or
  `git stash` on tracked files. Ever.
- **Commit verified work.** After a change passes the gate, commit it with
  `git commit --only <paths>` so a shared working tree is never swept up in someone
  else's commit.
