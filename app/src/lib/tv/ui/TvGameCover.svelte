<script lang="ts">
  // A TV cover for a game from any list (Discover's rows, a Server platform,
  // Search everything): the badges from the shared stores, A opens Details,
  // X plays or installs (items 17, 172, 192). While it installs only the
  // tag's text changes (item 118): every other part stays as it is.
  import { subjectFromInstalled, subjectFromSummary } from '../../background';
  import { downloads } from '../../stores/downloads.svelte';
  import { session } from '../../stores/session.svelte';
  import { openGameDetails, runQuick } from '../gameActions';
  import { select } from '../select.svelte';
  import { liveInstall, platformTag, queueTag, quickFor, type TvGame } from '../games';
  import TvCover from './TvCover.svelte';

  let {
    k,
    game,
    plat = false,
    label = true,
    testId = undefined,
    onFocus = undefined,
    selectable = false,
  }: {
    k: string;
    game: TvGame;
    /** The platform tag (mixed lists). */
    plat?: boolean;
    label?: boolean;
    testId?: string;
    onFocus?: () => void;
    /** On a grid with select mode (Library, a Server platform): A picks it there. */
    selectable?: boolean;
  } = $props();

  let live = $derived(liveInstall(downloads.entries, game.romId));
  let queue = $derived(queueTag(live));
  let quick = $derived(quickFor(game, live, session.connected));
  let art = $derived(game.summary ? subjectFromSummary(game.summary) : game.row ? subjectFromInstalled(game.row) : null);
</script>

<TvCover
  {k}
  title={game.title}
  cover={game.cover}
  installed={game.installed}
  update={game.update}
  missing={game.missing}
  platform={plat ? platformTag(game) : null}
  {queue}
  {label}
  {art}
  {testId}
  sel={selectable ? game.id : undefined}
  picked={selectable && select.picked(game.id)}
  quick={quick?.label ?? null}
  onA={() => openGameDetails(game)}
  onX={() => quick && void runQuick(game, quick.kind)}
  {onFocus}
/>
