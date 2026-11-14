<script lang="ts">
  // Playing a store game (TVStores README § The game page › Play; prototype
  // `splaying`): NOW PLAYING, the title, "GOG · Native · 0:12", "Your saves
  // sync with GOG when the game closes." while its saves sync, Back to the
  // game (focused) and Stop game (confirmed). The page closes by itself when
  // the game exits; the upload card after it is the backend's. A Steam game
  // runs in Steam: "Steam · running in Steam", its note, and no Stop game.
  import { onDestroy } from 'svelte';
  import { api } from '../../api';
  import * as run from '../../games/run';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { managedByClient, storeOfKey } from '../../gamestores/marks';
  import { sessions } from '../../stores/sessions.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { closeLayer, nav, popPage, topLayer } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import { sessionTime } from '../pages/details';
  import TvButton from '../ui/TvButton.svelte';
  import { defaultCompat } from './storeActions';
  import { stopWords, tvPlayingLine, tvPlayingNote } from './storeWords';

  let { gameKey, title }: { gameKey: string; title: string } = $props();

  const pageId = nav.topPage?.id ?? null;
  const openedAt = Date.now();
  let now = $state(Date.now());
  let seen = $state(false);
  let closed = false;

  let game = $derived(gameStores.game(gameKey));
  let session = $derived(sessions.sessionForKey(gameKey));
  let startedMs = $derived(session && session.started_at > 0 ? session.started_at * 1000 : openedAt);
  let line = $derived(game ? tvPlayingLine(game, defaultCompat(), sessionTime(now - startedMs)) : sessionTime(now - startedMs));
  let note = $derived(game ? tvPlayingNote(game) : null);
  let canStop = $derived(!managedByClient(game?.source ?? storeOfKey(gameKey)));

  const tick = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(tick));

  // The game exited: the page closes by itself.
  $effect(() => {
    if (session) seen = true;
    else if (seen) void close();
  });
  const fallback = setTimeout(() => {
    if (seen || closed) return;
    api
      .listSessions()
      .then((snap) => {
        if (!snap.sessions.some((s) => s.key === gameKey)) void close();
      })
      .catch(() => {});
  }, 6000);
  onDestroy(() => clearTimeout(fallback));

  async function close() {
    if (closed) return;
    closed = true;
    while (topLayer()) await closeLayer();
    if (pageId !== null && nav.topPage?.id === pageId) await popPage();
  }

  function resume() {
    api.tvYieldToGame().catch((e: unknown) => pushToast(e instanceof Error ? e.message : String(e), 'error'));
  }

  function stop() {
    void confirmBox({
      ...stopWords(title),
      danger: true,
      onOk: async () => {
        const s = sessions.sessionForKey(gameKey);
        if (!s) {
          void close();
          return;
        }
        const outcome = await run.stopGame(s.id);
        if (outcome.kind === 'ok') void close();
        else pushToast(outcome.reason, 'error');
      },
    });
  }
</script>

<div class="playing" data-testid="tv-store-playing">
  <div class="box">
    <div class="lbl">NOW PLAYING</div>
    <div class="logo" data-testid="tv-playing-title">{title}</div>
    <div class="muted" data-testid="tv-playing-line">{line}</div>
    {#if note}<p class="note pl-note">{note}</p>{/if}
    <div class="btns" data-nav="h">
      <TvButton k="pl-resume" autofocus testId="tv-playing-resume" onA={resume}>Back to the game</TvButton>
      {#if canStop}<TvButton k="pl-stop" danger testId="tv-playing-stop" onA={stop}>Stop game</TvButton>{/if}
    </div>
  </div>
</div>
