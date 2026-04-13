# Agents follow these rules

- Reference 'SPEC.md' only when implementing new features, modifying UI/UX behavior, or resolving ambiguous product-level design questions. Do not read it for bug fixes, test-only changes, or edits within already-implemented logic.
- Reference 'ARCHITECTURE.md' only when you need to identify which module owns a behavior or choose the right file to edit for a non-trivial change. For `MainWindow` behavior, always check the `rom_mate/ui/mixins/` section first — behavior is distributed across `CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, and `DetailsViewMixin` before searching `rom-mate.py`.
- Reference 'openapi.json' only when modifying code that constructs, sends, or parses HTTP requests to the RomM server (e.g. `rom_mate/core/api.py` or `rom_mate/server/`).
- Use unittest when writing or modifying tests.