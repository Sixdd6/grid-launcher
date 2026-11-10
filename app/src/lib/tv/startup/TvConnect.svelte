<script lang="ts">
  // Connect (items 392–399, 515–516; tvmode.js `connectPage`): the first
  // startup screen. Server, Sign in with, the API token or the username and
  // password, Library folder (the folder browser), Connect, "or", Use
  // without a server, and the note. A on a text field opens the keyboard;
  // the token and the password are masked and open empty. View pastes the
  // clipboard into the focused field. Start presses Connect, or says what is
  // missing while it is disabled (ruling 6). A rejected credential, or a
  // connect that failed, comes back here with the reason above the rows.
  //
  // The secret stays in `connectForm.svelte.ts` (a plain local, never
  // reactive, never logged), and is handed straight to the connect.
  import { api } from '../../api';
  import { saveConnectLibraryPath, useWithoutServer } from '../../connectFlow';
  import { connect, session } from '../../stores/session.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { openFolderBrowser } from '../gameActions';
  import { focused } from '../nav.svelte';
  import { openKeyboard, openList } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import {
    connectedText,
    connectFields,
    connectReady,
    connectWhy,
    CONNECT_SUB,
    CONNECT_TITLE,
    isSecret,
    keyboardTitle,
    LIBRARY_BROWSER_TITLE,
    NO_SERVER_NOTE,
    NO_SERVER_SUB,
    NO_SERVER_TITLE,
    offlineReady,
    parentOf,
    PASTED,
    SIGN_IN_LABELS,
    type FieldKey,
  } from './connectPage';
  import { connectForm as f, libraryToSave, setSecret, takeSecret } from './connectForm.svelte';

  let fields = $derived(connectFields(f));
  let ready = $derived(connectReady(f));
  let canOffline = $derived(offlineReady(f));
  let why = $derived(connectWhy(f.how));
  let reason = $derived(f.reason ?? session.error);
  let connecting = $state(false);

  function edit(key: FieldKey) {
    if (key === 'how') {
      void openList({
        title: 'Sign in with',
        items: (['token', 'password'] as const).map((how) => ({
          label: SIGN_IN_LABELS[how],
          checked: f.how === how,
          testId: `tv-opt-${how}`,
          onPick: () => {
            // A token and a password are different things: switching drops
            // the one typed.
            if (f.how !== how) takeSecret();
            f.how = how;
          },
        })),
      });
      return;
    }
    if (key === 'lib') {
      openFolderBrowser({ title: LIBRARY_BROWSER_TITLE, start: parentOf(f.lib), onPick: (p) => (f.lib = p) });
      return;
    }
    const secret = isSecret(key);
    void openKeyboard({
      title: keyboardTitle(key),
      value: secret ? '' : key === 'url' ? f.url : f.user,
      mask: secret,
      onDone: (v) => {
        if (secret) setSecret(v);
        else if (key === 'url') f.url = v.trim();
        else f.user = v;
      },
    });
  }

  /** View: the clipboard's text into the focused field (item 396). */
  async function paste() {
    const key = focused()?.getAttribute('data-field') as FieldKey | null;
    if (!key || key === 'how') return;
    let text = '';
    try {
      text = (await api.readClipboardText()).trim();
    } catch {
      return;
    }
    if (text === '') return;
    if (isSecret(key)) setSecret(text);
    else if (key === 'url') f.url = text;
    else if (key === 'user') f.user = text;
    else f.lib = text;
    pushToast(PASTED);
  }

  async function connectNow() {
    if (!ready || f.busy || session.busy) return;
    f.busy = true;
    connecting = true;
    f.reason = null;
    session.error = null;
    try {
      const refused = await saveConnectLibraryPath(libraryToSave());
      if (refused !== null) {
        f.reason = refused;
        return;
      }
      if (libraryToSave() !== null) f.libStored = f.lib.trim();
      const token = f.how === 'token';
      // Handed straight over; the field is blank again in the same tick.
      const pending = connect(f.url.trim(), token ? '' : f.user, takeSecret(), token);
      await pending;
      if (session.error === null && session.phase !== 'none') pushToast(connectedText(session.serverUrl || f.url.trim()));
    } finally {
      f.busy = false;
      connecting = false;
    }
  }

  async function offlineNow() {
    if (!canOffline || f.busy || session.busy) return;
    f.busy = true;
    f.reason = null;
    session.error = null;
    takeSecret();
    try {
      const refused = await useWithoutServer(libraryToSave());
      if (refused !== null) f.reason = refused;
      else if (session.error === null) pushToast(NO_SERVER_TITLE, 'success', undefined, { sub: NO_SERVER_SUB });
    } finally {
      f.busy = false;
    }
  }

  useScreen({
    hints: () => [{ button: 'VIEW', label: 'Paste' }],
    main: () => ({ disabled: !ready, why, press: () => void connectNow() }),
    view: () => void paste(),
  });
</script>

<div class="start" data-testid="tv-connect">
  <div class="start-box" data-nav="v">
    <h1>{CONNECT_TITLE}</h1>
    <p class="sub">{CONNECT_SUB}</p>
    {#if reason}<p class="errline" role="alert" data-testid="tv-connect-reason">{reason}</p>{/if}
    <div class="start-body" data-nav="v" id="cf-rows">
      {#each fields as fl (fl.key)}
        <div
          class="field"
          data-k={`cf-${fl.key}`}
          data-field={fl.key}
          data-al={fl.a}
          data-testid={`tv-cf-${fl.key}`}
          use:tvf={{ a: () => edit(fl.key) }}
        >
          <span class="lab">{fl.label}</span><span class="v" class:ph={fl.placeholder} data-testid={`tv-cf-${fl.key}-value`}>{fl.value}</span>
        </div>
      {/each}
    </div>
    <div class="btns start-foot" data-nav="h" id="cf-btns">
      <TvButton k="cf-go" primary disabled={!ready} {why} testId="tv-connect-go" onA={() => void connectNow()}>
        {connecting ? 'Connecting…' : 'Connect'}
      </TvButton>
      <span class="or">or</span>
      <TvButton k="cf-none" disabled={!canOffline} testId="tv-connect-offline" onA={() => void offlineNow()}>Use without a server</TvButton>
    </div>
    <p class="note" style="margin:.8rem 0 0">{NO_SERVER_NOTE}</p>
  </div>
</div>
