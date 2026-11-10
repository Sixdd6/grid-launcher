<script lang="ts">
  // TV mode's notification cards (items 139–147; tvmode.js `toast`): top
  // right under the top bar, 26rem wide, at most three, the newest on top
  // under the pinned setup card. They never take focus or react to
  // pointing. A card with an action shows the View prompt and the action's
  // name; View runs the newest one (notices.svelte.ts `viewNewest`).
  import Glyph from './glyphs/Glyph.svelte';
  import { tvNotices } from './notices.svelte';
</script>

<div class="toasts" data-testid="tv-toasts" aria-live="polite">
  {#each tvNotices.cards as c (c.id)}
    <div class="toast" class:err={c.error} role={c.error ? 'alert' : 'status'} data-testid="tv-toast" data-pinned={c.pinned ? '' : undefined}>
      <div class="tt" data-testid="tv-toast-title">{c.title}</div>
      {#if c.sub !== null}
        <div class="ts">{#if typeof c.sub === 'string'}{c.sub}{:else}{#each c.sub as part, i (i)}{#if typeof part === 'string'}{part}{:else}<Glyph button={part.glyph} />{/if}{/each}{/if}</div>
      {/if}
      {#if c.segs}
        <div class="segs">{#each c.segs as seg, i (i)}<i class={seg.state}><b style={`width:${seg.pct}%`}></b></i>{/each}</div>
      {/if}
      {#if c.step !== null}<div class="ts">{c.step}</div>{/if}
      {#if c.action}<div class="ta" data-testid="tv-toast-action"><Glyph button="VIEW" />{c.action.label}</div>{/if}
      {#if c.countdown !== null}
        {#key c.countdown}<i class="tbar" style={`animation-duration:${c.countdown}ms`}></i>{/key}
      {/if}
    </div>
  {/each}
</div>
