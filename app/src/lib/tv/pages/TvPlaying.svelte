<script lang="ts">
  // Playing (items 321–325; tvmode.js `playing`, `ACT.resume`, `ACT.stop`;
  // ruling 7): "NOW PLAYING", the title as a logo, "<Platform> · <emulator>
  // · <m:ss>" counting up, the note, Back to the game (focused) and Stop
  // game (confirmed). While GRID's window has focus the controller reaches
  // them, a game running or not (the gate; user decision 2026-11-10); while
  // the game's window has focus GRID hears nothing. The page closes by
  // itself when the game exits, then the save upload notices follow.
  import { onDestroy } from 'svelte';
  import { api } from '../../api';
  import { subjectFromInstalled } from '../../background';
  import * as run from '../../games/run';
  import { installed } from '../../stores/installed.svelte';
  import { noteViewed } from '../../stores/lastViewed.svelte';
  import { sessions } from '../../stores/sessions.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import type { PlayingGame } from '../gameActions';
  import { closeLayer, nav, popPage, topLayer } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import { playingLine, playingNote, sessionTime, stopConfirm, uploadingSaveNotice } from './details';

  let { game }: { game: PlayingGame } = $props();

  const pageId = nav.topPage?.id ?? null;
  const openedAt = Date.now();
  let now = $state(Date.now());
  let uploadOnExit = $state(false);
  let seen = $state(false);
  let closed = false;

  let session = $derived(sessions.sessionFor(game.romId));
  let row = $derived(installed.list.find((r) => r.rom_id === game.romId) ?? null);
  let startedMs = $derived(session && session.started_at > 0 ? session.started_at * 1000 : openedAt);
  let line = $derived(playingLine(game.platform, session?.emulator_name ?? '', sessionTime(now - startedMs)));

  const tick = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(tick));

  api
    .cloudSettings()
    .then((s) => (uploadOnExit = s.upload_on_exit))
    .catch(() => {});

  // The game's art behind the page (item 11).
  $effect(() => {
    if (row) noteViewed(subjectFromInstalled(row));
  });

  // The game exited: the page closes by itself (ruling 7). A session that
  // never shows (it ended before the first snapshot) is asked for once.
  $effect(() => {
    if (session) seen = true;
    else if (seen) void close(true);
  });
  const fallback = setTimeout(() => {
    if (seen || closed) return;
    api
      .listSessions()
      .then((snap) => {
        if (!snap.sessions.some((s) => s.rom_id === game.romId)) void close(true);
      })
      .catch(() => {});
  }, 6000);
  onDestroy(() => clearTimeout(fallback));

  async function close(ended: boolean) {
    if (closed) return;
    closed = true;
    while (topLayer()) await closeLayer();
    if (pageId !== null && nav.topPage?.id === pageId) await popPage();
    if (ended && uploadOnExit && row) {
      // Only a game whose saves sync gets the notice; the backend's
      // upload-finished card follows with the desktop's words (item 325).
      api
        .cloudPanelInfo(row, 'save')
        .then((info) => {
          if (info.supported) pushToast(uploadingSaveNotice(game.title), 'success');
        })
        .catch(() => {});
    }
  }

  function resume() {
    api.tvYieldToGame().catch((e: unknown) => pushToast(e instanceof Error ? e.message : String(e), 'error'));
  }

  function stop() {
    void confirmBox({
      ...stopConfirm(game.title),
      danger: true,
      onOk: async () => {
        const s = sessions.sessionFor(game.romId);
        if (!s) {
          void close(true);
          return;
        }
        const outcome = await run.stopGame(s.id);
        if (outcome.kind === 'ok') void close(true);
        else pushToast(outcome.reason, 'error');
      },
    });
  }
</script>

<div class="playing" data-testid="tv-playing">
  <div class="box">
    <div class="lbl">NOW PLAYING</div>
    <div class="logo" data-testid="tv-playing-title">{game.title}</div>
    <div class="muted" data-testid="tv-playing-line">{line}</div>
    <p class="note pl-note">{playingNote(uploadOnExit)}</p>
    <div class="btns" data-nav="h">
      <TvButton k="pl-resume" autofocus testId="tv-playing-resume" onA={resume}>Back to the game</TvButton>
      <TvButton k="pl-stop" danger testId="tv-playing-stop" onA={stop}>Stop game</TvButton>
    </div>
  </div>
</div>
