<script lang="ts">
  // RomM › Emulators › Other builds (TVRommStorefront README "RomM ›
  // Emulators"; prototype `ACT.treBuilds`): the builds GRID doesn't install
  // on this computer, one dimmed row each. The rows take focus but have no
  // A hint; A says why.
  import { sizeText } from '../../../../backups/dialog';
  import { otherReasonText, tvOtherBuildWhy, tvOtherBuildsSub } from '../../../../romm/emulators';
  import { rommEmulators } from '../../../../romm/emulators.svelte';
  import { tvf } from '../../../focus/actions';
  import TvPage from '../../../ui/TvPage.svelte';

  let view = $derived(rommEmulators.load?.phase === 'ready' ? rommEmulators.load.data.view : null);
  let host = $derived(view?.host ?? '');
</script>

<div data-testid="tv-romm-other-builds">
  <TvPage title="Other builds" sub={tvOtherBuildsSub(host)}>
    {#each view?.other_builds ?? [] as b, i (`${b.folder}/${b.file_name}`)}
      <div
        class="srow"
        data-k={`tb-b${i}`}
        aria-disabled="true"
        data-why={tvOtherBuildWhy(b.reason, host)}
        data-testid={`tv-romm-other-build-${i}`}
        use:tvf={{}}
      >
        <div class="lab">
          <div class="t tre-file">{b.file_name}</div>
          <div class="s">{b.folder} folder · {sizeText(b.size_bytes)}</div>
        </div>
        <div class="val">{otherReasonText(b.reason)}</div>
      </div>
    {/each}
  </TvPage>
</div>
