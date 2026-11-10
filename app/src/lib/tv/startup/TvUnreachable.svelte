<script lang="ts" module>
  // Set while a picked folder is being relocated to: the screen under the
  // folder browser comes back first, and must not reopen the dialog then.
  let relocating = false;
</script>

<script lang="ts">
  // The unreachable library (items 409–412; README § Startup screens ›
  // Unreachable library; tvmode.js `unreachable`): GRID can't run without
  // its library, so this startup screen is only the blocking dialog over an
  // empty stage. Relocate the library… opens the folder browser ("Relocate
  // the library"); B there comes back to the dialog, and a picked folder is
  // relocated to (stored paths are rewritten, no file moves), after which
  // GRID carries on to Connect or where it opens. Close without changing
  // anything quits GRID at once, with no second question (ruling 18).
  import { closeWithoutChanges, libraryUnreachable, relocateTo } from '../../stores/libraryUnreachable.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { closeLayer, openLayer, topLayer } from '../nav.svelte';
  import { useScreen } from '../screen.svelte';
  import { parentOf } from './connectPage';
  import TvUnreachableDialog from './TvUnreachableDialog.svelte';
  import { RELOCATE_BROWSER_TITLE, relocatedNotice } from './unreachable';

  let root = $derived(libraryUnreachable.status?.status === 'unreachable' ? libraryUnreachable.status.root : '');

  function openDialog() {
    if (relocating || !libraryUnreachable.blocking || topLayer() !== null) return;
    void openLayer({
      component: TvUnreachableDialog,
      props: { root, onRelocate: relocate, onClose: quit },
      blocking: true,
    });
  }

  function relocate() {
    void closeLayer().then(() =>
      openFolderBrowser({ title: RELOCATE_BROWSER_TITLE, start: parentOf(root), onPick: (p) => void relocateToPicked(p) }),
    );
  }

  async function relocateToPicked(path: string) {
    relocating = true;
    const refused = await relocateTo(path, (outcome) => {
      const n = relocatedNotice(outcome);
      pushToast(n.title, n.error ? 'error' : 'success', undefined, { sub: n.sub });
    });
    relocating = false;
    if (refused !== null) {
      pushToast(refused, 'error');
      openDialog();
    }
  }

  async function quit() {
    try {
      await closeWithoutChanges();
    } catch (err) {
      pushToast(err instanceof Error ? err.message : String(err), 'error');
    }
  }

  // The screen appears (first, or back from the folder browser): the dialog
  // opens over it. After the turn, so a folder just picked is seen first.
  useScreen({ arrive: () => void setTimeout(openDialog, 0) });
</script>

<div class="start" data-testid="tv-unreachable-screen"></div>
