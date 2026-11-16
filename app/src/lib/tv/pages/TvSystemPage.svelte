<script lang="ts">
  // A system's page (checklist items 326–336; tvmode.js `systemPage`),
  // from Settings › Systems, a Server header chip or a launch card's
  // "Set up <Platform>": the system's name, "<emulator> · <status>" under
  // it, then its rows (systemPage.ts): the emulator list, version, Update
  // to, RetroArch core, companions, firmware, Open <emulator> (its window
  // opens over TV mode, ruling 19), Emulator settings, Remove (asked, then
  // back) and Setup tips; a system with no emulator offers its recommended
  // one and the list; Windows has the Proton version, the umu-run line and
  // its Install / Reinstall, and the first-game runtime note; an emulator
  // no system uses (Other emulators) gets its own lines. The data and
  // commands are the shared SystemsStore's (systemsTv.svelte.ts).
  import { api } from '../../api';
  import { umuActionLabel, umuInstallBusy } from '../../emulators/compatTools';
  import { platformCoreSelect } from '../../emulators/defaults';
  import { dynamicEmulatorNotes, emulatorNotes } from '../../emulators/notes';
  import { compatTools } from '../../stores/compatTools.svelte';
  import { downloads } from '../../stores/downloads.svelte';
  import { emulatorUpdates } from '../../stores/emulatorUpdates.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { ps3FirmwareView } from '../../systems/ps3Firmware';
  import {
    alsoPlays,
    alsoPlaysText,
    companionsOf,
    missingCompanionText,
    missingText,
    otherEmulatorNote,
    canPlay,
    companionParent,
    shortName,
    testKeyOf,
    versionText,
    type SystemRow,
  } from '../../systems/rows';
  import { popPage, pushPage } from '../nav.svelte';
  import { confirmBox, openList } from '../overlays/overlays.svelte';
  import { romm } from '../../stores/romm.svelte';
  import { rommHost } from '../../romm/words';
  import { checkingTitle, type LocateAnswer } from '../../systems/locate';
  import { firmwareRows } from './firmwareRows';
  import { baseName, tvReport } from './filePicker';
  import TvFilePicker from './TvFilePicker.svelte';
  import TvLocateReport from './TvLocateReport.svelte';
  import { openEmulatorList, openProtonList, tvProton, useTvSystems, windowsNow } from '../systemsTv.svelte';
  import { pillOf } from '../tabs/settings/panes/systems';
  import TvButton from '../ui/TvButton.svelte';
  import TvPage from '../ui/TvPage.svelte';
  import TvRows from '../ui/TvRows.svelte';
  import { openEmulatorForm } from './emulatorFormOpen.svelte';
  import {
    openingNotice,
    pageSub,
    removeConfirm,
    systemPageRows,
    umuValue,
    type CompanionLine,
    type EmuLine,
    type SystemPageModel,
  } from './systemPage';

  let {
    system = '',
    emulator = null,
    windows = false,
  }: {
    /** The system's label or server name. */
    system?: string;
    /** An emulator no system uses (Other emulators). */
    emulator?: string | null;
    /** The Windows (Proton) system. */
    windows?: boolean;
  } = $props();

  const s = useTvSystems();
  let umuRequesting = $state(false);

  const errText = (err: unknown) => (err instanceof Error ? err.message : String(err));

  let row = $derived<SystemRow | null>(s.rows.find((r) => r.key === system || r.name === system) ?? null);

  function line(name: string, exceptKey: string | null): EmuLine {
    const check = emulatorUpdates.check(name);
    return {
      name,
      version: versionText(s.entryFor(name), check),
      updateTo: check && emulatorUpdates.hasUpdate(name) ? check.available_display : null,
      updating: s.updating === name,
      alsoPlays: exceptKey === null ? '' : alsoPlaysText(alsoPlays(name, s.rows, exceptKey)),
      tips: [...emulatorNotes(name), ...dynamicEmulatorNotes(name, s.facts.get(name))].map((n) => n.text),
    };
  }

  function companionLines(r: SystemRow): CompanionLine[] {
    return companionsOf(r.emulator, s.companions).map((c) => {
      const key = testKeyOf(c.profile);
      if (c.entry !== null) return { kind: 'installed', key, line: line(c.entry, null) };
      return {
        kind: 'missing',
        key,
        profile: c.profile,
        text: missingCompanionText(c, r.emulator),
        installing: s.companionInstalling(c),
        canInstall: c.sourceId !== null,
      };
    });
  }

  let model = $derived.by((): SystemPageModel => {
    if (windows) {
      const w = windowsNow();
      const umu = compatTools.umuRun;
      return {
        kind: 'windows',
        status: pillOf(w.status).pill,
        tool: w.tool,
        umu: umu ? umuValue(umu) : null,
        umuAction: umu ? umuActionLabel(umu) : null,
        umuBusy: umuRequesting || (umu !== null && umuInstallBusy(downloads.entries, umu.source_id)),
        pickerBusy: tvProton.pending || w.installing,
      };
    }
    if (!s.loaded) return { kind: 'loading', system: emulator ?? system };
    if (emulator !== null) {
      return {
        kind: 'other',
        system: emulator,
        note: otherEmulatorNote(canPlay(emulator, s.rows), companionParent(emulator, s.companions)),
        emu: line(emulator, null),
        ps3: ps3FirmwareView(s.rpcs3Firmware.get(emulator)),
        removing: s.removing,
      };
    }
    const r = row;
    if (!r) return { kind: 'unknown', system };
    const st = s.rowStatus(r);
    const status = pillOf(st.status, st.text).pill;
    if (r.emulator === '') {
      return {
        kind: 'none',
        system: r.label,
        status,
        recommended: r.recommended ? shortName(r.recommended.name) : null,
        missing: missingText(r),
        installing: st.status === 'installing',
      };
    }
    const core = platformCoreSelect(s.defaults, r.key, r.emulator, s.coreOptions[r.name] ?? [], r.aliases);
    return {
      kind: 'emulator',
      system: r.label,
      status,
      emu: line(r.emulator, r.key),
      core: core.visible ? { options: core.options, selected: core.selected, disabled: core.disabled } : null,
      // The firmware block below the rows replaces the old server line.
      firmware: null,
      ps3: ps3FirmwareView(s.rpcs3Firmware.get(r.emulator)),
      companions: companionLines(r),
      removing: s.removing,
    };
  });

  let title = $derived(windows ? 'Windows' : emulator !== null ? emulator : (row?.label ?? system));

  // --- Firmware (with or without RomM) -----------------------------------
  // The system's expected files, read when the page opens and when its
  // emulator changes (a More systems system isn't read by the pane).
  $effect(() => {
    const r = row;
    if (!r || windows || emulator !== null) return;
    void s.expectedKey(r);
    s.refreshExpected(r);
  });

  let fwView = $derived(row && !windows && emulator === null ? s.firmwareView(row) : null);
  let fwFiles = $derived(row ? (s.expected[s.expectedKey(row)] ?? []) : []);
  let fwRows = $derived(
    row && fwView && (fwFiles.length > 0 || fwView.pup)
      ? firmwareRows(
          {
            system: row.label,
            emulator: row.emulator,
            host: rommHost(romm.status),
            connection: romm.status.connection,
            files: fwFiles,
            view: fwView,
          },
          { romm: () => row && s.installFromRomm(row), locate: () => locate() },
        )
      : [],
  );

  function locate() {
    const r = row;
    if (!r || r.emulator === '') return;
    const pup = s.firmwareView(r).pup;
    const pageTitle = pup ? 'Locate PS3UPDAT.PUP' : `Locate ${r.label} firmware`;
    void pushPage({
      title: pageTitle,
      component: TvFilePicker,
      props: {
        title: pageTitle,
        emulator: r.emulator,
        single: pup,
        onCheck: (paths: string[], folder: boolean) => void check(r, paths, folder, pup),
      },
    });
  }

  async function check(r: SystemRow, paths: string[], folder: boolean, pup: boolean) {
    if (pup) {
      const path = paths[0] ?? '';
      pushToast(`Checking ${baseName(path)}…`, 'success');
      const answer = await s.locatePup(r, path);
      if (answer.refused.length > 0) {
        // Stays on the picker: Choose another file… is a pick away.
        void confirmBox({ title: 'Can’t use this file', body: answer.refused[0].reason, ok: 'Choose another file…', onOk: () => {} });
        return;
      }
      pushToast('Installing the PS3 firmware in RPCS3…', 'success', undefined, { sub: 'This takes a minute' });
      await popPage();
      return;
    }
    pushToast(folder ? 'Checking this folder…' : checkingTitle(paths.length), 'success', undefined, {
      sub: folder ? paths[0] : `${r.label} firmware`,
    });
    let answer: LocateAnswer;
    try {
      answer = await s.locateFiles(r, paths);
    } catch (err) {
      answer = { installed: [], unverified: [], refused: paths.map((p) => ({ file: baseName(p), reason: errText(err) })) };
    }
    const report = tvReport({ system: r.label, emulator: r.emulator, folder, where: paths[0] ?? '', picked: paths.length, answer });
    pushToast(report.card.title, report.card.error ? 'error' : 'success', undefined, report.card.text ? { sub: report.card.text } : {});
    await popPage();
    void pushPage({
      title: report.title,
      component: TvLocateReport,
      props: {
        report,
        onMore: () => void popPage().then(locate),
        onDone: () => void popPage(),
      },
    });
  }

  async function installUmu() {
    if (umuRequesting) return;
    umuRequesting = true;
    try {
      await api.installUmuLauncher();
    } catch (err) {
      pushToast(errText(err), 'error');
    } finally {
      umuRequesting = false;
    }
  }

  function remove(name: string) {
    // A companion (ruling E4) names no system, and the page stays.
    const companion = model.kind === 'emulator' && name !== model.emu.name;
    const label = model.kind === 'emulator' && !companion ? model.system : null;
    void confirmBox({
      ...removeConfirm(name, label),
      danger: true,
      onOk: async () => {
        if (!(await s.removeEmulator(name))) return;
        await s.refreshAll();
        if (!companion) void popPage();
      },
    });
  }

  let rows = $derived(
    systemPageRows(model, {
      pickEmulator: () => {
        if (row) openEmulatorList(row);
      },
      installRecommended: () => {
        if (row) void s.installRecommended(row);
      },
      pickCore: () => {
        if (model.kind !== 'emulator' || !model.core || !row) return;
        const r = row;
        const core = model.core;
        void openList({
          title: `Core for ${r.label}`,
          items: core.options.map((c) => ({ label: c, checked: c === core.selected, onPick: () => void s.changeCore(r, c) })),
        });
      },
      update: (name) => void s.updateEmulator(name),
      installFirmware: () => {
        if (row) s.installFirmware(row);
      },
      installPs3Firmware: (name) => void s.installPs3Firmware(name),
      open: (name) => {
        pushToast(openingNotice(name), 'success');
        void s.openEmulator(name);
      },
      settings: (name) => {
        const entry = s.entryFor(name);
        if (entry) openEmulatorForm('edit', entry);
      },
      remove,
      installCompanion: (key) => {
        const c = row ? companionsOf(row.emulator, s.companions).find((x) => testKeyOf(x.profile) === key) : undefined;
        if (c) void s.installCompanion(c);
      },
      pickProton: openProtonList,
      installUmu: () => void installUmu(),
    }),
  );
</script>

<div data-testid="tv-system-page">
  <TvPage {title} sub={pageSub(model)}>
    {#if model.kind === 'unknown'}
      <p class="note" data-testid="tv-system-unknown">{system} isn't a system that needs an emulator.</p>
      <div class="btns" data-nav="h">
        <TvButton k="sy-back" autofocus onA={() => void popPage()}>Back</TvButton>
      </div>
    {:else}
      <TvRows {rows} />
      {#if fwRows.length > 0}
        <div class="sgroup" data-testid="tv-system-firmware">Firmware</div>
        <TvRows rows={fwRows} />
      {/if}
    {/if}
  </TvPage>
</div>
