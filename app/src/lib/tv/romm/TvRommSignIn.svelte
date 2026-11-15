<script lang="ts">
  // The RomM sign-in page (TVRommStorefront README § The RomM sign-in page;
  // prototype `rSignInPage`): Connect to RomM, Edit RomM sign-in, or Sign in
  // to RomM again. Server URL, Sign in with (API token / Username and
  // password), then the token or the username and password; A opens the
  // on-screen keyboard (the secret masked, opening empty) or the list. View
  // pastes the clipboard into the focused field. The button (Start presses
  // it) says what is missing while disabled, "Connecting…" while it runs.
  // A failure shows above the fields and sends focus to the field to fix.
  //
  // The secret is a plain local, never reactive, never logged or shown: only
  // whether one is typed is state. It goes straight to the connect.
  import { onDestroy, tick, untrack } from 'svelte';
  import { api } from '../../api';
  import type { SignInMode } from '../../romm/words';
  import { signInError, signInStart } from '../../romm/signIn';
  import { connectRomm, romm } from '../../stores/romm.svelte';
  import { session } from '../../stores/session.svelte';
  import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { focused, popPage, scope, setFocus } from '../nav.svelte';
  import { openKeyboard, openList } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import type { SignInFrom } from './rommActions';
  import {
    canPasteInto,
    connectedCard,
    errorField,
    NOTHING_TO_PASTE,
    PASTED,
    SIGN_IN_HOW_LABELS,
    signInFields,
    signInHelp,
    signInKeyboardTitle,
    signInWhy,
    signInWords,
    SIGNING_IN_WHY,
    type SignInFieldKey,
    type SignInHow,
  } from './rommTv';
  import TvRommMark from './TvRommMark.svelte';

  let { mode, from = 'settings' }: { mode: SignInMode; from?: SignInFrom } = $props();

  const start = untrack(() => signInStart(romm.status, mode));
  const words = untrack(() => signInWords(mode, romm.status.username));
  const again = untrack(() => mode === 'again');

  let url = $state(start.serverUrl);
  let how = $state<SignInHow>(start.useToken ? 'token' : 'password');
  let user = $state(start.username);
  let secretTyped = $state(false);
  let secret = '';
  let err = $state<string | null>(null);
  let busy = $state(false);

  function setSecret(v: string) {
    secret = v;
    secretTyped = v !== '';
  }
  function takeSecret(): string {
    const s = secret;
    secret = '';
    secretTyped = false;
    return s;
  }
  onDestroy(() => void takeSecret());

  let fields = $derived(signInFields({ url, how, user, secretTyped }));
  let why = $derived(busy ? SIGNING_IN_WHY : signInWhy({ url, how, user, secretTyped }));

  function edit(key: SignInFieldKey) {
    if (key === 'how') {
      void openList({
        title: 'Sign in with',
        items: (['token', 'password'] as const).map((h) => ({
          label: SIGN_IN_HOW_LABELS[h],
          ...(h === 'token' ? { sub: 'Recommended' } : {}),
          checked: how === h,
          testId: `tv-opt-${h}`,
          onPick: () => {
            // A token and a password are different things: switching drops the one typed.
            if (how !== h) takeSecret();
            how = h;
          },
        })),
      });
      return;
    }
    const isSecret = key === 'secret';
    void openKeyboard({
      title: signInKeyboardTitle(key, how),
      value: isSecret ? '' : key === 'url' ? url : user,
      mask: isSecret,
      onDone: (v) => {
        err = null;
        if (isSecret) setSecret(v);
        else if (key === 'url') url = v.trim();
        else user = v.trim();
      },
    });
  }

  /** View: the clipboard's text into the focused field. */
  async function paste() {
    const key = focused()?.getAttribute('data-field') ?? null;
    if (!canPasteInto(key)) {
      pushToast(NOTHING_TO_PASTE.title, 'success', TOAST_DURATION_MS, { sub: NOTHING_TO_PASTE.sub });
      return;
    }
    let text = '';
    try {
      text = (await api.readClipboardText()).trim();
    } catch {
      text = '';
    }
    if (text === '') {
      pushToast(NOTHING_TO_PASTE.title, 'success', TOAST_DURATION_MS, { sub: NOTHING_TO_PASTE.sub });
      return;
    }
    err = null;
    if (key === 'secret') setSecret(text);
    else if (key === 'url') url = text;
    else user = text;
    pushToast(PASTED);
  }

  async function focusField(key: 'url' | 'secret') {
    await tick();
    const el = scope()?.querySelector<HTMLElement>(`[data-k="rs-${key}"]`) ?? null;
    if (el) setFocus(el);
  }

  async function go() {
    if (busy) return;
    if (why !== null) {
      pushToast(why, 'error');
      return;
    }
    busy = true;
    err = null;
    const token = how === 'token';
    const serverUrl = url.trim();
    // Handed straight over; the field is blank again in the same tick.
    const pending = connectRomm(serverUrl, token ? '' : user.trim(), takeSecret(), token, from);
    const raw = await pending;
    busy = false;
    if (raw !== null) {
      err = signInError(raw, serverUrl, token);
      const to = errorField(raw);
      if (to !== null) void focusField(to);
      return;
    }
    const c = connectedCard(romm.status);
    pushToast(c.title, 'success', TOAST_DURATION_MS, { sub: c.sub });
    // From the RomM page with setup needed, Choose your systems has taken
    // the stage (decision 9); otherwise back to where it was opened from.
    if (session.phase === 'setup') return;
    void popPage();
  }

  useScreen({
    hints: () => [
      { button: 'VIEW', label: 'Paste' },
      ...(why === null ? [{ button: 'MENU' as const, label: words.button }] : []),
    ],
    main: () => ({ disabled: why !== null, why: why ?? undefined, press: () => void go() }),
    view: () => void paste(),
  });
</script>

<div class="page" data-scroll="page" data-testid="tv-romm-signin">
  <div class="page-in rsign" data-nav="v">
    <h1 data-testid="tv-rows-title"><TvRommMark lg /> {words.title}</h1>
    <p class="sub" data-testid="tv-rows-sub">{words.sub}</p>
    {#if err}<p class="errline" role="alert" data-testid="tv-rs-error">{err}</p>{/if}
    <div data-nav="v" id="rs-rows">
      {#each fields as fl (fl.key)}
        <div
          class="field"
          data-k={`rs-${fl.key}`}
          data-field={fl.key}
          data-al={fl.a}
          data-autofocus={again && fl.key === 'secret' ? '' : undefined}
          data-testid={`tv-rs-${fl.key}`}
          use:tvf={{ a: () => edit(fl.key) }}
        >
          <span class="lab">{fl.label}</span><span class="v" class:ph={fl.placeholder} data-testid={`tv-rs-${fl.key}-value`}>{fl.value}</span>
        </div>
      {/each}
    </div>
    <div class="bk-foot">
      <span class="muted" data-testid="tv-rs-help">{signInHelp(how)}</span>
      <TvButton k="rs-go" primary glyph={busy ? undefined : 'MENU'} disabled={why !== null} why={why ?? undefined} testId="tv-rs-go" onA={() => void go()}>
        {#if busy}<span class="sspin"></span>Connecting…{:else}{words.button}{/if}
      </TvButton>
    </div>
  </div>
</div>
