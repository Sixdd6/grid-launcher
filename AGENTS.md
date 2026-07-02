# Agents follow these rules

- Reference 'SPEC.md' only when implementing new features, modifying UI/UX behavior, or resolving ambiguous product-level design questions. Do not read it for bug fixes, test-only changes, or edits within already-implemented logic.
- Reference 'ARCHITECTURE.md' only when you need to identify which module owns a behavior or choose the right file to edit for a non-trivial change. For `MainWindow` behavior, always check the `grid_launcher/ui/mixins/` section first — behavior is distributed across `CloudSaveMixin`, `EmulatorUIMixin`, `InstallMixin`, and `DetailsViewMixin` before searching `grid-launcher.py`.
- Reference 'openapi.json' only when modifying code that constructs, sends, or parses HTTP requests to the RomM server (e.g. `grid_launcher/core/api.py` or `grid_launcher/server/`).
- Use unittest when writing or modifying tests.