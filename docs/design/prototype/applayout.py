import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1]
import importlib.util
spec = importlib.util.spec_from_file_location('gen', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'gen.py'))
# gen.py writes previews when imported; reuse its data and icon helper.
sys.argv = [sys.argv[0], OUT]
gen = importlib.util.module_from_spec(spec); spec.loader.exec_module(gen)

# Placeholder games. "rel" is the release date and "added" the day it reached
# the server (RomM: metadata release date, created_at), both made up; Discover
# sorts by them. LINUX games are native on a Linux computer.
REL = {
  "Starfall Circuit": ("1998-03-14", "2026-10-02"), "Lantern Vale": ("1999-11-05", "2026-09-21"),
  "Ironclad Drift": ("1997-06-27", "2026-10-30"), "Moss & Marrow": ("2003-04-18", "2026-11-05"),
  "Pixel Harbor": ("1995-08-11", "2026-08-14"), "Cinder Peaks": ("2000-02-24", "2026-11-06"),
  "Echo Garden": ("2001-01-19", "2026-10-28"), "Velvet Static": ("2006-09-07", "2026-11-03"),
  "Tidewalker": ("2004-12-02", "2026-10-12"), "Brass Comet": ("1998-10-22", "2026-07-30"),
  "Hollow Signal": ("1999-05-13", "2026-11-04"), "Kite Season": ("1996-03-29", "2026-10-19"),
  "Neon Orchard": ("2005-07-21", "2026-09-09"), "Glass Monsoon": ("2000-10-10", "2026-11-01"),
  "Quiet Engine": ("1997-12-04", "2026-10-25"), "Ember Choir": ("2005-03-31", "2026-11-07"),
}
EXTRA = [
  ("Lumen Drift", "hsl(265 55% 42%)", "LINUX", "2026-10-24", "2026-10-26"),
  ("Paper Orbit", "hsl(28 70% 44%)", "LINUX", "2026-09-12", "2026-09-15"),
  ("Kestrel Run", "hsl(160 45% 32%)", "LINUX", "2026-06-30", "2026-11-02"),
  ("Saltmarsh", "hsl(200 35% 36%)", "LINUX", "2025-11-02", "2026-08-03"),
  ("Quiet Atlas", "hsl(340 40% 38%)", "LINUX", "2025-03-15", "2026-06-11"),
  ("Rook & Ribbon", "hsl(48 60% 38%)", "LINUX", "2024-08-08", "2026-05-20"),
  ("Coral Static", "hsl(10 55% 40%)", "PS", "2000-11-16", "2026-10-31"),
  ("Signal Fires", "hsl(230 45% 40%)", "PS2", "2007-03-08", "2026-10-29"),
  ("Orbit Bloom", "hsl(300 40% 40%)", "GC", "2006-06-15", "2026-11-06"),
  ("Harbor Lights", "hsl(205 50% 38%)", "WIN", "2026-08-21", "2026-08-25"),
  ("Grey Meridian", "hsl(220 15% 34%)", "WIN", "2026-03-05", "2026-10-27"),
  ("Foxglove", "hsl(330 45% 40%)", "WIN", "2025-10-16", "2026-07-08"),
  ("Iron Lullaby", "hsl(15 35% 34%)", "WIN", "2024-12-11", "2026-09-30"),
  ("Low Tide Radio", "hsl(185 40% 34%)", "WIN", "2023-05-25", "2026-11-03"),
  ("Pale Summit", "hsl(210 25% 46%)", "WIN", "2022-02-17", "2026-04-14"),
  # 2026 releases, so New releases (last six months) has something to show.
  ("Night Ferry", "hsl(250 40% 30%)", "LINUX", "2026-11-04", "2026-11-05"),
  ("Copperline", "hsl(25 55% 36%)", "WIN", "2026-10-30", "2026-11-01"),
  ("Wren Hollow", "hsl(120 30% 30%)", "LINUX", "2026-10-15", "2026-10-18"),
  ("Static Bloom", "hsl(315 45% 38%)", "WIN", "2026-10-02", "2026-10-06"),
  ("Pocket Monsoon", "hsl(190 50% 34%)", "SNES", "2026-09-27", "2026-10-01"),
  ("Glassjaw", "hsl(0 45% 36%)", "WIN", "2026-09-03", "2026-09-10"),
  ("Lowland Signal", "hsl(90 30% 32%)", "LINUX", "2026-08-12", "2026-08-20"),
  ("Tinroof Rally", "hsl(40 60% 38%)", "N64", "2026-07-25", "2026-08-02"),
  ("Ashgrove", "hsl(15 25% 30%)", "WIN", "2026-07-09", "2026-07-15"),
  ("Mirrorlake", "hsl(200 45% 42%)", "LINUX", "2026-06-18", "2026-06-25"),
  ("Fable Engine", "hsl(275 35% 34%)", "WIN", "2026-05-29", "2026-06-04"),
  ("Old Moon Arcade", "hsl(55 45% 34%)", "LINUX", "2026-04-20", "2026-05-02"),
]
data = {
  "games": [
    {"t": t, "c": c, "p": p, "rel": REL[t][0], "added": REL[t][1]} for (t, c, p) in gen.GAMES
  ] + [{"t": t, "c": c, "p": p, "rel": r, "added": a} for (t, c, p, r, a) in EXTRA],
}

HTML = r"""<!-- @dsCard group="Prototype" height=800 width=1280 subtitle="The app as it is today, clickable: every view, rail pane, Details and dialogs" -->
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Current app</title>
<style>
  html, body { height: 100%; overflow: hidden; }
  body { padding: 0; background: var(--bg); }
  .app { position: relative; height: 100vh; min-height: 420px; display: flex; flex-direction: column; overflow: clip; isolation: isolate; }
  .art-clip { position: absolute; inset: 0; overflow: clip; z-index: -1; pointer-events: none; }

  /* Background art: the last-viewed cover, blurred, under every panel. */
  .art { position: absolute; inset: -60px; filter: blur(40px); transition: opacity 220ms ease; }
  .art i { position: absolute; border-radius: 50%; }

  .view { flex: 1 1 auto; min-height: 0; position: relative; box-sizing: border-box; padding: var(--gutter); overflow-y: auto; }
  .view[hidden] { display: none; }
  .rail-divider { border: 0; border-top: 1px solid var(--line); margin: 6px 8px; }
  .columns { display: flex; align-items: stretch; gap: var(--gutter); height: 100%; min-height: 0; }
  .body { flex: 1 1 auto; min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow-y: auto; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--card-w, 160px), 1fr)); gap: 18px 14px; padding: var(--gutter) 2px 40px; }
  .grid.hover-labels { row-gap: 18px; }
  .grid.hover-labels .g-card { gap: 0; }
  .grid.hover-labels .g-label { position: absolute; top: calc(100% + 6px); left: 50%; transform: translateX(-50%); opacity: 0; width: max-content; max-width: calc(100% + 32px); background: var(--overlay); box-shadow: var(--shadow-lift); pointer-events: none; transition: opacity 150ms ease; }
  .grid.hover-labels .g-card:hover .g-label { opacity: 1; top: calc(100% + 6px); }
  .empty { align-self: flex-start; margin: var(--gutter) 0 0; padding: 10px 14px; color: var(--text-muted); font-size: 14px; }
  .content { max-width: 1100px; margin: 0 auto; display: flex; flex-direction: column; gap: var(--gutter); }

  /* Downloads */
  .seg { padding: 14px 16px; display: flex; flex-direction: column; gap: 10px; }
  .seg h3 { margin: 0; font-size: 13px; font-weight: 600; color: var(--text-h); }
  .seg h3 span { color: var(--text-muted); font-weight: 400; margin-left: 6px; }
  .legend { margin: 0; font-size: 12px; color: var(--text-muted); padding: 10px 16px; }
  .dl { display: grid; grid-template-columns: 1fr 120px auto; gap: 14px; align-items: center; padding: 10px 12px; border-radius: var(--r-row); background: var(--surface); }
  .dl .t { color: var(--text-h); font-weight: 600; }
  .dl .s { font-size: 12px; color: var(--text-muted); }
  .bar { height: 4px; border-radius: 2px; background: var(--field); margin-top: 6px; overflow: hidden; }
  .bar b { display: block; height: 100%; background: var(--primary); }
  .kind { font-size: 10px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-chip); border: 1px solid var(--border); color: var(--text-muted); margin-left: 6px; }
  .small-btn { font: inherit; font-size: 12px; padding: 5px 12px; border-radius: var(--r-control); border: 1px solid var(--border); background: transparent; color: var(--text); cursor: pointer; }
  .small-btn:hover { background: var(--surface); }
  .small-btn.primary { background: var(--primary); color: var(--on-primary); border-color: var(--primary); }
  .small-btn.primary:hover { background: var(--primary-hover); }
  .small-btn.ok { color: var(--text-muted); cursor: default; display: inline-flex; gap: 4px; align-items: center; }
  .small-btn.ok:hover { background: transparent; }

  /* Emulators / Settings panes */
  /* As in the app (Emulators.svelte, Settings.svelte): the panel is the whole
     column and stays put; only the page inside it scrolls. */
  .panes { flex: 1 1 auto; min-width: 0; height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .pane-scroll { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
  .pane { width: 100%; max-width: 1100px; margin: 0 auto; box-sizing: border-box; padding: 24px; display: flex; flex-direction: column; gap: 14px; }
  .pane h2 { margin: 0; font-size: 18px; font-weight: 600; color: var(--text-h); }
  .emu { padding: 12px 14px; border-radius: var(--r-row); border: 1px solid var(--line); background: var(--surface); display: flex; flex-direction: column; gap: 8px; }
  .emu-top { display: flex; align-items: baseline; gap: 10px; }
  .emu-top b { color: var(--text-h); font-size: 14px; }
  .emu-top code, .fw code { font-size: 12px; color: var(--text-muted); font-family: ui-monospace, monospace; }
  .emu-top .acts { margin-left: auto; display: flex; gap: 6px; }
  .fw { display: flex; align-items: center; gap: 10px; font-size: 12px; color: var(--text-muted); padding-top: 8px; border-top: 1px solid var(--line); }
  .fw .acts { margin-left: auto; }
  .setting { display: grid; grid-template-columns: 200px 1fr; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .setting:last-child { border-bottom: none; }
  .setting > span { color: var(--text-h); }
  .setting > .g-switch, .setting > .g-control { justify-self: start; }
  .setting small { display: block; color: var(--text-muted); font-size: 11px; }
  .setting input[type=range] { width: 240px; accent-color: var(--primary); }
  .note { margin: 0; color: var(--text-muted); font-size: 13px; }
  .pane-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .sub { display: flex; flex-direction: column; gap: 10px; }
  .sub h3 { margin: 0; font-size: 14px; font-weight: 600; color: var(--text-h); }
  .sub h4 { margin: 6px 0 0; font-size: 13px; font-weight: 600; color: var(--text-h); }
  .cat-list, .defaults, .radios, .moves { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .cat-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 12px; border-radius: var(--r-row); background: var(--surface); border: 1px solid var(--line); }
  .cat-row .t, .umu .t { display: block; color: var(--text-h); font-weight: 600; }
  .cat-row .s { display: block; font-size: 12px; color: var(--text-muted); }
  .small-btn:disabled { opacity: .6; cursor: default; }
  .defaults { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px; }
  .dcard { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border-radius: var(--r-row); background: var(--surface); border: 1px solid var(--line); }
  .dcard b { color: var(--text-h); }
  .dfield { display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--text-muted); }
  .dfield > span { width: 54px; flex: none; }
  .dfield .field { flex: 1; }
  .umu { padding: 12px 14px; border-radius: var(--r-row); background: var(--surface); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .umu-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .radios label { display: flex; align-items: center; gap: 8px; color: var(--text); cursor: pointer; }
  .radios input, .check input { accent-color: var(--primary); }
  .form { display: flex; flex-direction: column; gap: 12px; max-width: 520px; }
  .form label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--text-muted); }
  .form label.check, .check { flex-direction: row; align-items: center; gap: 8px; font-size: 13px; color: var(--text); }
  .field { font: inherit; font-size: 13px; height: var(--control-h); box-sizing: border-box; padding: 0 10px; border-radius: var(--r-control); border: 1px solid var(--line-strong); background: var(--field); color: var(--text-h); }
  .field.narrow { width: 120px; }
  .inline { display: flex; gap: 8px; }
  .inline .field { flex: 1; }
  .inline .g-btn { padding: 0 14px; height: var(--control-h); }
  .form-actions, .choice { display: flex; flex-wrap: wrap; gap: 8px; }
  .form-actions .g-btn, .choice .g-btn, .update-line .g-btn { padding: 7px 16px; }
  .kv { margin: 0; display: flex; flex-direction: column; }
  .kv div { display: grid; grid-template-columns: 140px 1fr; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .kv dt { color: var(--text-muted); }
  .kv dd { margin: 0; color: var(--text-h); display: flex; align-items: center; }
  .path { display: block; padding: 8px 10px; border-radius: var(--r-control); background: var(--field); border: 1px solid var(--line); color: var(--text-h); font: 13px ui-monospace, monospace; }
  .line { margin: 0; color: var(--text-h); }
  .update-line { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .moves li { display: flex; justify-content: space-between; padding: 8px 12px; border-radius: var(--r-row); background: var(--surface); }
  .moves span { color: var(--text-muted); font-size: 12px; }
  .g-dialog.libdlg { width: 480px; height: auto; max-height: calc(100% - 48px); overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }
  .libdlg h3 { margin: 0; font-size: 16px; color: var(--text-h); }


  /* Details sits inside the app frame here (fixed in the app). */
  .g-scrim[hidden] { display: none; }
  .g-dialog { width: min(1040px, calc(100% - 48px)); height: min(680px, calc(100% - 48px)); }
  .g-details-left { overflow-y: auto; }
  .tabpanel { flex: 1; min-height: 0; overflow-y: auto; padding-right: 4px; }
  .overview { display: flex; flex-wrap: wrap; align-items: flex-start; gap: 20px 28px; }
  .media-row { display: flex; gap: 10px; overflow-x: auto; padding-bottom: 6px; }
  .shot { flex: none; width: 220px; aspect-ratio: 16/9; border-radius: var(--r-row); box-shadow: var(--shadow-card); }
  .sec-title { margin: 4px 0 8px; font-size: 13px; font-weight: 600; color: var(--text-h); }
  .save-row { display: flex; align-items: center; gap: 12px; padding: 9px 12px; border-radius: var(--r-row); background: var(--surface); margin-top: 8px; }
  .save-row .t { color: var(--text-h); }
  .save-row .s { color: var(--text-muted); font-size: 12px; margin-left: auto; }

  .toasts { position: absolute; left: 50%; transform: translateX(-50%); bottom: calc(var(--footer-h) + 24px); z-index: 30; display: flex; flex-direction: column; align-items: center; gap: 8px; pointer-events: none; }
  .toasts .g-toast { animation: toast-in 220ms ease; }
  @keyframes toast-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
  .g-strip[hidden] { display: none; }
  .menu-wrap { position: absolute; top: calc(100% + 6px); right: 0; z-index: 6; }
  .menu-wrap[hidden] { display: none; }
__COVER_CSS__
</style></head>
<body>
<div class="app" id="app">
  <div class="art-clip" aria-hidden="true"><div class="art" id="art"></div></div>

  <header class="g-topbar">
    <div class="g-brand"><span class="g-logo" data-icon="grid" data-size="20"></span><span class="g-wordmark">GRID</span></div>
    <nav class="g-pills" aria-label="Views" id="pills"></nav>
    <div class="g-session">
      <span class="g-status-dot online" aria-hidden="true"></span>
      <button class="g-session-chip" id="chip" aria-expanded="false">player@romm.home.lan</button>
      <div class="menu-wrap" id="menu" hidden>
        <div class="g-menu" role="menu"><button role="menuitem">Disconnect</button><button role="menuitem">Open RomM in browser</button><span class="g-menu-host">romm.home.lan</span></div>
      </div>
    </div>
  </header>

  <div class="view" data-view="library" hidden>
    <div class="columns">
      <nav class="g-rail" id="lib-rail" aria-label="Library filters"></nav>
      <div class="body">
        <div class="g-panel g-head">
          <header class="g-platform-header"><h2 id="lib-title">All games</h2><p class="g-counts" id="lib-count"></p></header>
          <div class="g-toolbar">
            <input class="g-search" type="search" placeholder="Search by title" aria-label="Search the library" data-search="library">
            <label class="g-control"><span>Sort</span><select><option>Recently played</option><option>Recently installed</option><option>Title</option><option>Platform</option></select></label>
            <label class="g-control"><span>Size</span><select data-size-select><option value="120">Small</option><option value="160" selected>Medium</option><option value="200">Large</option></select></label>
          </div>
        </div>
        <div class="grid" id="lib-grid"></div>
      </div>
    </div>
  </div>

  <div class="view" data-view="server">
    <div class="columns">
      <nav class="g-rail" id="srv-rail" aria-label="Server platforms"></nav>
      <div class="body">
        <div class="g-panel g-head">
          <header class="g-platform-header">
            <h2 id="srv-title"></h2><p class="g-counts" id="srv-count"></p>
            <div class="g-chips"><button class="g-chip-outline" data-go="emulators">Firmware: 3 files · installed</button><button class="g-chip-outline" data-go="emulators" id="srv-emu"></button></div>
          </header>
          <div class="g-toolbar">
            <input class="g-search" type="search" placeholder="Search title, platform or genre" aria-label="Search this platform" data-search="server">
            <label class="g-control"><span>Size</span><select data-size-select><option value="120">Small</option><option value="160" selected>Medium</option><option value="200">Large</option></select></label>
          </div>
        </div>
        <div class="grid" id="srv-grid"></div>
      </div>
    </div>
  </div>

  <div class="view" data-view="downloads" hidden>
    <div class="content">
      <p class="g-panel legend">Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled</p>
      <section class="g-panel seg"><h3>Active<span id="dl-active-n">1</span></h3><div id="dl-active"></div></section>
      <section class="g-panel seg"><h3>Queued<span id="dl-queued-n">0</span></h3><div id="dl-queued"><p class="note">Nothing is waiting.</p></div></section>
      <section class="g-panel seg"><h3>Completed<span id="dl-done-n">0</span></h3><div id="dl-done"></div></section>
    </div>
  </div>

  <div class="view" data-view="emulators" hidden>
    <div class="columns">
      <nav class="g-rail" id="emu-rail" aria-label="Emulator categories"></nav>
      <div class="panes g-panel"><div class="pane-scroll"><section class="pane" id="emu-pane"></section></div></div>
    </div>
  </div>

  <div class="view" data-view="settings" hidden>
    <div class="columns">
      <nav class="g-rail" id="set-rail" aria-label="Settings"></nav>
      <div class="panes g-panel"><div class="pane-scroll"><section class="pane" id="set-pane"></section></div></div>
    </div>
  </div>

  <footer class="g-strip" id="strip" role="button" tabindex="0" data-go="downloads">
    <span class="g-strip-lead" data-icon="download" data-size="14"></span>
    <span class="g-strip-line" id="strip-line"></span>
    <svg width="120" height="18" viewBox="0 0 120 18" aria-hidden="true"><polyline id="spark-net" fill="none" stroke="var(--primary)" stroke-width="1.5"/><polyline id="spark-disk" fill="none" stroke="var(--graph-disk)" stroke-width="1.5"/></svg>
    <span class="g-strip-open">Open Downloads</span>
  </footer>

  <div class="toasts" id="toasts" aria-live="polite"></div>

  <div class="g-scrim" id="libscrim" hidden><div class="g-dialog libdlg" role="dialog" aria-modal="true" id="libdlg"></div></div>

  <div class="g-scrim" id="scrim" hidden>
    <div class="g-dialog" role="dialog" aria-modal="true" id="dialog" tabindex="-1" style="outline:none">
      <button class="g-icon-btn g-dialog-close" aria-label="Close" id="close"><span data-icon="close" data-size="20"></span></button>
      <div class="g-details">
        <aside class="g-details-left" id="d-left"></aside>
        <section class="g-details-right">
          <header class="g-details-head" id="d-head"></header>
          <div class="g-tabs" role="tablist" id="d-tabs"></div>
          <div class="tabpanel" id="d-panel"></div>
        </section>
      </div>
    </div>
  </div>
</div>
__ICON_JS__
<script>
(function(){
var DATA = __DATA__;
var G = DATA.games.map(function(g, i){ return Object.assign({ id: i, installed: [0,1,2,4,6,9,12].indexOf(i) >= 0, update: i === 1, missing: i === 9, cloud: [0,4,6].indexOf(i) >= 0 }, g); });
var PLAT = { PS: 'PlayStation', N64: 'Nintendo 64', SAT: 'Sega Saturn', GC: 'GameCube', SNES: 'Super Nintendo', PS2: 'PlayStation 2', LINUX: 'Linux', WIN: 'Windows' };
var EMU = { PS: 'DuckStation', N64: 'RetroArch · Mupen64Plus-Next', SAT: 'Ymir (Sega Saturn)', GC: 'Dolphin', SNES: 'RetroArch · Snes9x', PS2: 'PCSX2', LINUX: 'none', WIN: 'none' };
var VIEWS = ['library','server','downloads','emulators','settings'];
var S = { view: 'server', platform: 'PS', libFilter: 'all', q: { library: '', server: '' }, cardW: 160, labels: 'always', fade: 50, blur: 2, lastFade: 50, emuPane: 'installed', setPane: 'appearance', detail: null, tab: 'overview', kind: 'save', dl: null, fw: { PS: 'installed', SAT: 'install', GC: 'unavailable' } };
var $ = function(id){ return document.getElementById(id); };
function esc(s){ return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;'); }

/* ---- background art ---- */
function paintArt(g){
  var c = g ? g.c : 'hsl(258 45% 38%)';
  $('art').innerHTML = '<i style="left:-5%;top:-10%;width:55%;height:80%;background:'+c+'"></i><i style="right:-10%;top:20%;width:50%;height:90%;background:'+c+';opacity:.7"></i><i style="left:30%;bottom:-20%;width:40%;height:50%;background:hsl(220 40% 20%)"></i>';
  /* The app: opacity = 1 - fade/100; 100 is fully faded, which is off. */
  $('art').style.opacity = Math.max(0, 1 - S.fade / 100);
  $('art').style.filter = 'blur(' + (12 + S.blur * 1.4) + 'px)';
}

/* ---- top bar ---- */
function renderPills(){
  $('pills').innerHTML = VIEWS.map(function(v){ return '<button class="g-pill'+(S.view===v?' active':'')+'" data-view-btn="'+v+'"'+(S.view===v?' aria-current="page"':'')+'>'+v.charAt(0).toUpperCase()+v.slice(1)+'</button>'; }).join('');
  document.querySelectorAll('.view').forEach(function(el){ el.hidden = el.getAttribute('data-view') !== S.view; });
}

/* ---- rails ---- */
function rail(id, rows, onPick){
  $(id).innerHTML = rows.map(function(r){
    return (r.heading ? '<span class="g-rail-heading">'+r.heading+'</span>' : '') + '<button class="g-rail-item'+(r.sel?' active':'')+'" data-key="'+r.key+'"'+(r.sel?' aria-current="page"':'')+'><span class="g-rail-label">'+esc(r.label)+'</span>'+(r.count!==undefined?'<span class="g-rail-count">'+r.count+'</span>':'')+'</button>'+(r.divider?'<hr class="rail-divider">':'');
  }).join('');
  $(id).onclick = function(e){ var b = e.target.closest('[data-key]'); if (b) onPick(b.getAttribute('data-key')); };
}

/* ---- cards ---- */
function card(g, showPlatform){
  var tags = (g.missing || g.update) ? '<div class="g-tag-stack">'+(g.missing?'<span class="g-tag missing">MISSING</span>':'')+(g.update?'<span class="g-tag update">UPDATE</span>':'')+'</div>' : '';
  var prim = g.missing ? 'Details' : (g.installed ? 'Play' : (S.dl && S.dl.id === g.id ? 'Installing…' : DLQ.indexOf(g.id) >= 0 ? 'Queued' : 'Install'));
  return '<div class="g-card" data-game="'+g.id+'"><div class="g-cover"><div class="g-art" style="background:'+g.c+'"><span>'+esc(g.t)+'</span></div>'+tags+
    (g.installed?'<span class="g-dot" role="img" aria-label="Installed"></span>':'')+
    (showPlatform?'<span class="g-tag platform g-corner">'+g.p+'</span>':'')+
    (g.cloud?'<span class="g-cloud-badge g-corner" data-icon="cloud" data-size="14"></span>':'')+
    '<div class="g-shade"></div><div class="g-bar"><button class="g-card-primary" data-act="primary">'+prim+'</button><button class="g-round" data-act="cloud" aria-label="Cloud sync" data-icon="cloud" data-size="15"></button><button class="g-round" data-act="details" aria-label="Details" data-icon="more" data-size="15"></button></div></div>'+
    '<span class="g-label" title="'+esc(g.t)+'">'+esc(g.t)+'</span></div>';
}
function fillGrid(id, list, showPlatform, emptyText){
  var el = $(id);
  el.className = 'grid' + (S.labels === 'hover' ? ' hover-labels' : '');
  el.style.setProperty('--card-w', S.cardW + 'px');
  el.innerHTML = list.length ? list.map(function(g){ return card(g, showPlatform); }).join('') : '<p class="g-panel empty">'+esc(emptyText)+'</p>';
  gIcons(el);
}
function match(g, q){ q = q.trim().toLowerCase(); return !q || g.t.toLowerCase().indexOf(q) >= 0 || PLAT[g.p].toLowerCase().indexOf(q) >= 0; }

function renderServer(){
  var plats = Object.keys(PLAT).sort(function(a,b){ return PLAT[a].localeCompare(PLAT[b]); });
  rail('srv-rail', plats.map(function(p){ return { key: p, label: PLAT[p], count: G.filter(function(g){return g.p===p;}).length, sel: S.platform === p }; }), function(k){ S.platform = k; renderServer(); });
  var inPlat = G.filter(function(g){ return g.p === S.platform; });
  $('srv-title').textContent = PLAT[S.platform];
  $('srv-count').textContent = inPlat.length + (inPlat.length === 1 ? ' game · ' : ' games · ') + inPlat.filter(function(g){return g.installed;}).length + ' installed';
  $('srv-emu').textContent = 'Emulator: ' + EMU[S.platform];
  /* Native and Windows games run without an emulator: no emulator or firmware chips. */
  document.querySelector('[data-view=server] .g-chips').hidden = S.platform === 'LINUX' || S.platform === 'WIN';
  var shown = inPlat.filter(function(g){ return match(g, S.q.server); });
  fillGrid('srv-grid', shown, false, S.q.server.trim() ? 'No games match “'+S.q.server.trim()+'”' : 'This platform has no games');
}
function renderLibrary(){
  var inst = G.filter(function(g){ return g.installed; });
  var plats = Object.keys(PLAT).filter(function(p){ return inst.some(function(g){return g.p===p;}); }).sort(function(a,b){ return PLAT[a].localeCompare(PLAT[b]); });
  var rows = [{key:'all',label:'All games',count:inst.length},{key:'recent',label:'Recent',count:3},{key:'updates',label:'Updates',count:inst.filter(function(g){return g.update;}).length}]
    .concat(plats.map(function(p,i){ return { key: p, label: PLAT[p], count: inst.filter(function(g){return g.p===p;}).length, heading: i===0?'PLATFORMS':undefined }; }));
  rows.forEach(function(r){ r.sel = r.key === S.libFilter; });
  rail('lib-rail', rows, function(k){ S.libFilter = k; renderLibrary(); });
  var f = S.libFilter, list = inst.filter(function(g){ return f==='all' ? true : f==='recent' ? g.id < 5 : f==='updates' ? g.update : g.p === f; }).filter(function(g){ return match(g, S.q.library); });
  $('lib-title').textContent = rows.filter(function(r){return r.sel;})[0].label;
  $('lib-count').textContent = list.length + (list.length === 1 ? ' game' : ' games');
  fillGrid('lib-grid', list, true, f === 'recent' ? 'Nothing played in the last 30 days' : f === 'updates' ? 'Everything is up to date' : f === 'all' ? 'No games installed' : 'No games installed for ' + PLAT[f]);
}

/* ---- downloads ---- */
var spark = [];
function renderDownloads(){
  var d = S.dl;
  $('strip').hidden = !d;
  $('dl-active-n').textContent = d ? 1 : 0;
  $('dl-queued-n').textContent = DLQ.length;
  $('dl-done-n').textContent = DONE.length;
  $('dl-done').innerHTML = DONE.length ? DONE.map(function(x, i){ return '<div class="dl"' + (i ? ' style="margin-top:8px"' : '') + '><div><span class="t">'+esc(x.t)+'</span>'+(x.kind ? '<span class="kind">'+x.kind+'</span>' : '')+'<div class="s">'+esc(x.sub)+'</div></div><span></span><button class="small-btn" data-undone="'+i+'">Dismiss</button></div>'; }).join('') : '<p class="note">Nothing has finished yet.</p>';
  $('dl-queued').innerHTML = DLQ.length ? DLQ.map(function(id){ var q = G[id]; return '<div class="dl" style="margin-top:8px"><div><span class="t">'+esc(q.t)+'</span><div class="s">'+PLAT[q.p]+' · Waiting for a slot</div></div><span></span><button class="small-btn" data-unqueue="'+id+'">Cancel</button></div>'; }).join('') : '<p class="note">Nothing is waiting.</p>';
  if (!d) { $('dl-active').innerHTML = '<p class="note">Nothing is downloading.</p>'; return; }
  var pct = Math.round(d.pct), g = G[d.id];
  var line = d.pct < 100 ? esc(g.t)+' · '+pct+'% · '+d.speed.toFixed(1)+' MB/s' : esc(g.t)+' · Installing';
  $('strip-line').innerHTML = line;
  var pts = function(arr){ return arr.map(function(v,i){ return (i*120/59).toFixed(1)+','+(17-v).toFixed(1); }).join(' '); };
  $('spark-net').setAttribute('points', pts(spark));
  $('spark-disk').setAttribute('points', pts(spark.map(function(v){return Math.max(1, v-3);})));
  $('dl-active').innerHTML = '<div class="dl"><div><span class="t">'+esc(g.t)+'</span><div class="s">'+PLAT[g.p]+' · '+(d.pct<100?'Downloading '+pct+'%':'Installing')+'</div><div class="bar"><b style="width:'+pct+'%"></b></div></div><svg width="120" height="38" viewBox="0 0 120 18" preserveAspectRatio="none"><polyline fill="none" stroke="var(--primary)" stroke-width="1.2" points="'+pts(spark)+'"/></svg><button class="small-btn" data-cancel>Cancel</button></div>';
}
var DLQ = [];
/* Completed downloads, newest first (the app keeps the last 50). */
var DONE = [{ t: 'Moss & Marrow', kind: 'Update', sub: 'GameCube · Finished' }, { t: 'DuckStation', kind: 'Emulator', sub: 'Finished' }];
function addDone(t, kind, sub) { DONE.unshift({ t: t, kind: kind, sub: sub }); if (DONE.length > 50) DONE.length = 50; renderDownloads(); }
function startInstall(g, fromQueue){
  if (S.dl) {
    if (S.dl.id !== g.id && DLQ.indexOf(g.id) < 0) { DLQ.push(g.id); toast('Added to download queue'); renderAll(); }
    return;
  }
  S.dl = { id: g.id, pct: 0, speed: 30 }; spark = []; for (var i=0;i<60;i++) spark.push(2);
  if (!fromQueue) toast('Added to download queue');
  var timer = setInterval(function(){
    if (!S.dl) { clearInterval(timer); return; }
    S.dl.speed = 28 + Math.random()*16; S.dl.pct = Math.min(100, S.dl.pct + 4 + Math.random()*4);
    spark.push(4 + Math.random()*11); spark.shift();
    if (S.dl.pct >= 100) { clearInterval(timer); renderDownloads(); setTimeout(function(){ G[S.dl.id].installed = true; toast(G[S.dl.id].t + ' installed'); addDone(G[S.dl.id].t, '', PLAT[G[S.dl.id].p] + ' · Finished'); S.dl = null; renderAll(); if (DLQ.length) startInstall(G[DLQ.shift()], true); }, 900); }
    renderDownloads();
  }, 400);
  renderAll();
}

/* ---- emulators ---- */
var CATALOG = [
  { n: 'DuckStation', p: 'GitHub · stenzek/duckstation', tag: 'v0.1-9226', inst: true },
  { n: 'Dolphin', p: 'dolphin-emu.org', tag: '2509', inst: true },
  { n: 'Ymir (Sega Saturn)', p: 'GitHub · StrikerX3/Ymir', tag: 'v0.4.1', inst: true },
  { n: 'PCSX2', p: 'GitHub · PCSX2/pcsx2', tag: 'v2.4.0', inst: false },
  { n: 'RetroArch', p: 'buildbot.libretro.com', tag: '1.21.0', inst: false },
  { n: 'RPCS3', p: 'GitHub · RPCS3/rpcs3-binaries-linux', tag: 'build-0.0.37', inst: false },
  { n: 'Eden', p: 'git.eden-emu.dev', tag: 'v0.0.3', inst: false },
  { n: 'xemu', p: 'GitHub · xemu-project/xemu', tag: 'v0.8.92', inst: false },
  { n: 'PPSSPP', p: 'GitHub · hrydgard/ppsspp', tag: 'v1.19.3', inst: false },
  { n: 'Cemu', p: 'GitHub · cemu-project/Cemu', tag: 'v2.6', inst: false }
];
var COMPAT = [
  { n: 'GE-Proton10-17', p: 'GitHub · GloriousEggroll/proton-ge-custom', tag: 'GE-Proton10-17', inst: true },
  { n: 'Wine-GE 8-26', p: 'GitHub · GloriousEggroll/wine-ge-custom', tag: 'GE-Proton8-26', inst: false },
  { n: 'Proton Experimental (umu)', p: 'GitHub · Open-Wine-Components/umu-proton', tag: 'UMU-Proton-9.0-4e', inst: false }
];
var DEFAULTS = { PS: 'DuckStation', N64: 'RetroArch', SAT: 'Ymir (Sega Saturn)', GC: 'Dolphin', SNES: 'RetroArch', PS2: '(none)' };
var CORES = { N64: ['mupen64plus_next', 'parallel_n64'], SNES: ['snes9x', 'bsnes'] };
S.addTab = 'catalog'; S.catQ = ''; S.compatDefault = 'GE-Proton10-17'; S.umu = 'system';

function catRow(e, kind) {
  var busy = S.installing && S.installing[e.n];
  var btn = e.inst ? '<button class="small-btn" disabled>Installed</button>'
    : '<button class="small-btn primary" data-cat="' + kind + ':' + esc(e.n) + '"' + (busy ? ' disabled' : '') + '>' + (busy ? 'Installing…' : 'Install') + '</button>';
  return '<li class="cat-row"><div><span class="t">' + esc(e.n) + '</span><span class="s">' + esc(e.p) + ' • ' + esc(e.tag) + '</span></div>' + btn + '</li>';
}

function renderEmulators(){
  var panes = [['installed','Installed'],['catalog','Add from catalog'],['defaults','Platform defaults'],['compat','Compat tools']];
  rail('emu-rail', panes.map(function(p){ return { key: p[0], label: p[1], sel: S.emuPane === p[0] }; }), function(k){ S.emuPane = k; renderEmulators(); });
  var el = $('emu-pane'), h;
  if (S.emuPane === 'installed') {
    var emus = [['DuckStation','PS','~/Games/GRID/emulators/duckstation/DuckStation.AppImage','-fullscreen "%rom%"'],['Ymir (Sega Saturn)','SAT','~/Games/GRID/emulators/ymir/ymir-sdl3','-f "%rom%"'],['Dolphin','GC','~/Games/GRID/emulators/dolphin/dolphin-emu','-b -e "%rom%"']];
    h = '<h2>Installed</h2>' + emus.map(function(e){
      var st = S.fw[e[1]], act;
      if (st === 'installed') act = '<span class="small-btn ok"><span data-icon="check" data-size="14"></span>Installed</span>';
      else if (st === 'requested') act = '<button class="small-btn" disabled>Requested…</button>';
      else if (st === 'unavailable') act = '<button class="small-btn" data-fw="'+e[1]+'">Not needed</button>';
      else act = '<button class="small-btn primary" data-fw="'+e[1]+'">Install firmware</button>';
      return '<div class="emu"><div class="emu-top"><b>'+e[0]+'</b><code>'+esc(e[3])+'</code><div class="acts"><button class="small-btn" data-edit="'+esc(e[0])+'">Edit</button><button class="small-btn">Delete</button></div></div><div class="emu-top"><code>'+esc(e[2])+'</code></div>'+
        '<div class="fw">'+PLAT[e[1]]+' firmware · '+(e[1]==='GC'?'1 file':'3 files')+'<span class="acts">'+act+'</span></div></div>';
    }).join('');
  } else if (S.emuPane === 'catalog') {
    h = '<h2>Add from catalog</h2><div class="g-tabs" role="tablist"><button data-addtab="catalog" class="' + (S.addTab === 'catalog' ? 'active' : '') + '">Catalog</button><button data-addtab="manual" class="' + (S.addTab === 'manual' ? 'active' : '') + '">Manual</button></div>';
    if (S.addTab === 'catalog') {
      var q = S.catQ.trim().toLowerCase(), list = CATALOG.filter(function(e){ return !q || e.n.toLowerCase().indexOf(q) >= 0; });
      h += '<input class="g-search" style="flex:none" type="search" placeholder="Search emulators…" aria-label="Search emulators" id="cat-q" value="' + esc(S.catQ) + '">' +
        (list.length ? '<ul class="cat-list">' + list.map(function(e){ return catRow(e, 'emu'); }).join('') + '</ul>' : '<p class="note">No emulators found.</p>');
    } else {
      h += '<form class="form" onsubmit="return false"><label>Profile<select class="field"><option>Choose a profile to fill in the fields…</option><option>DuckStation</option><option>PCSX2</option></select></label>' +
        '<label>Name<input class="field" placeholder="PCSX2"></label><label>Executable path<span class="inline"><input class="field" placeholder="/path/to/emulator"><button class="g-btn secondary" type="button">Browse…</button></span></label>' +
        '<label>Arguments<input class="field" placeholder="-fullscreen &quot;%rom%&quot;"></label><label>Save strategy<select class="field"><option>auto</option><option>single_file</option><option>folder</option></select></label>' +
        '<label>Save directories<input class="field" placeholder="memcards;saves (semicolon-separated)"></label><div class="form-actions"><button class="g-btn" type="button" data-toast="Emulator saved">Save</button><button class="g-btn secondary" type="button" data-addtab="catalog">Cancel</button></div></form>';
    }
  } else if (S.emuPane === 'defaults') {
    var plats = Object.keys(PLAT).sort(function(a,b){ return PLAT[a].localeCompare(PLAT[b]); });
    h = '<div class="pane-head"><h2>Platform defaults</h2><button class="g-btn secondary" data-toast="Opens the first-run platform picker">Set up platforms…</button></div><ul class="defaults">' + plats.map(function(p){
      var opts = ['(none)', 'DuckStation', 'Dolphin', 'Ymir (Sega Saturn)', 'RetroArch'];
      var sel = '<select class="field" data-default="' + p + '">' + opts.map(function(o){ return '<option' + (DEFAULTS[p] === o ? ' selected' : '') + '>' + o + '</option>'; }).join('') + '</select>';
      var core = DEFAULTS[p] === 'RetroArch' ? '<div class="dfield"><span>Core</span><select class="field">' + (CORES[p] || []).map(function(c){ return '<option>' + c + '</option>'; }).join('') + (CORES[p] ? '' : '<option>No installed core</option>') + '</select></div>' : '';
      return '<li class="dcard"><b>' + PLAT[p] + '</b><div class="dfield"><span>Emulator</span>' + sel + '</div>' + core + '</li>';
    }).join('') + '</ul>';
  } else {
    var umuLabel = S.umu === 'system' ? 'umu-run: system · /usr/bin/umu-run' : S.umu === 'busy' ? 'umu-run: installing…' : 'umu-run: GRID-managed · 1.2.9';
    h = '<h2>Compat tools</h2><div class="sub"><h3>Compatibility tools</h3>' +
      '<div class="umu"><div class="umu-row"><span class="t">' + umuLabel + '</span><button class="small-btn" data-umu' + (S.umu === 'busy' ? ' disabled' : '') + '>' + (S.umu === 'busy' ? 'Installing…' : S.umu === 'system' ? 'Install' : 'Reinstall') + '</button></div>' +
      '<p class="note">Proton games run through umu-run. The first Proton launch downloads the Steam Linux Runtime (about 200 MB), so it takes longer than later ones.</p></div>' +
      '<h4>Default</h4><ul class="radios">' + ['GE-Proton10-17', 'Wine 10.0 (system)'].map(function(t){ return '<li><label><input type="radio" name="cd" data-compat="' + esc(t) + '"' + (S.compatDefault === t ? ' checked' : '') + '> ' + esc(t) + '</label></li>'; }).join('') + '</ul>' +
      '<h4>Install</h4><ul class="cat-list">' + COMPAT.map(function(e){ return catRow(e, 'compat'); }).join('') + '</ul></div>';
  }
  el.innerHTML = h;
  gIcons(el);
  var cq = $('cat-q'); if (cq) { cq.oninput = function(){ S.catQ = this.value; var pos = this.selectionStart; renderEmulators(); var n = $('cat-q'); n.focus(); n.setSelectionRange(pos, pos); }; }
}

function installCatalog(kind, name) {
  var list = kind === 'emu' ? CATALOG : COMPAT, e = list.filter(function(x){ return x.n === name; })[0];
  S.installing = S.installing || {}; S.installing[name] = true; toast('Installing ' + name + '…'); renderEmulators();
  setTimeout(function(){ e.inst = true; delete S.installing[name]; toast(name + ' installed'); renderEmulators(); }, 1800);
}

/* ---- settings ---- */
S.cloud = { restore: true, upload: true, skip: true, delay: 5, keep: 10 };
S.ra = { user: '', set: false };
S.connEdit = false; S.updateNotice = true;

function renderSettings(){
  var panes = [['connection','Connection'],['library','Library'],['cloud','Cloud saves'],['ra','RetroAchievements'],['updates','Updates'],['appearance','Appearance']];
  rail('set-rail', panes.map(function(p){ return { key: p[0], label: p[1], sel: S.setPane === p[0] }; }), function(k){ S.setPane = k; S.connEdit = false; renderSettings(); });
  var el = $('set-pane'), h;
  if (S.setPane === 'connection') {
    h = '<h2>Connection</h2><dl class="kv"><div><dt>Server</dt><dd>https://romm.home.lan</dd></div><div><dt>User</dt><dd>player</dd></div><div><dt>Credential</dt><dd>Stored in the OS keyring · session verified</dd></div><div><dt>Status</dt><dd><span class="g-status-dot online" style="display:inline-block;margin-right:6px"></span>Connected</dd></div></dl>';
    h += S.connEdit
      ? '<form class="form" onsubmit="return false"><label>Server URL<input class="field" value="https://romm.home.lan"></label><label>Username<input class="field" value="player"></label><label>Password<input class="field" type="password" placeholder="Not shown"></label><label class="check"><input type="checkbox"> Use API token</label><div class="form-actions"><button class="g-btn" type="button" data-conn="save">Connect</button><button class="g-btn secondary" type="button" data-conn="cancel">Cancel</button></div></form>'
      : '<div class="form-actions"><button class="g-btn" data-conn="edit">Change server…</button><button class="g-btn secondary" data-toast="Reconnected to romm.home.lan">Reconnect</button><button class="g-btn secondary">Disconnect</button></div>';
  } else if (S.setPane === 'library') {
    h = '<h2>Library</h2><div class="sub"><h3>Library folder</h3><p class="note">Games, emulators and saves live in this folder. New installs go here.</p><code class="path">/home/player/Games/GRID</code><div class="form-actions"><button class="g-btn" data-libchange>Change…</button></div></div>';
  } else if (S.setPane === 'cloud') {
    var c = S.cloud;
    h = '<h2>Cloud saves</h2><form class="form" onsubmit="return false">' +
      '<label class="check"><input type="checkbox" data-cloud="restore"' + (c.restore ? ' checked' : '') + '> Restore cloud saves before launch</label>' +
      '<label class="check"><input type="checkbox" data-cloud="upload"' + (c.upload ? ' checked' : '') + '> Upload cloud saves after exit</label>' +
      '<label class="check"><input type="checkbox" data-cloud="skip"' + (c.skip ? ' checked' : '') + '> Skip download when the local save is newer</label>' +
      '<label>Upload delay (seconds)<input class="field narrow" type="number" min="0" max="60" value="' + c.delay + '"></label>' +
      '<label>Save retention limit (0 = unlimited)<input class="field narrow" type="number" min="0" value="' + c.keep + '"></label>' +
      '<p class="note">Auto-sync uses the latest server save record only.</p><div class="form-actions"><button class="g-btn" type="button" data-toast="Cloud save settings saved">Save</button></div></form>';
  } else if (S.setPane === 'ra') {
    h = '<h2>RetroAchievements</h2><p class="note">' + (S.ra.set ? 'Set for ' + esc(S.ra.user) : 'Not set') + '</p><form class="form" onsubmit="return false">' +
      '<label>Username<input class="field" id="ra-user" value="' + esc(S.ra.user) + '" autocomplete="off"></label><label>Password<input class="field" type="password" autocomplete="off"></label><label>Token<input class="field" type="password" autocomplete="off"></label>' +
      '<div class="form-actions"><button class="g-btn secondary" type="button" data-ra="login">Log In</button><button class="g-btn" type="button" data-ra="save">Save</button><button class="g-btn secondary" type="button" data-ra="clear">Clear</button></div></form>';
  } else if (S.setPane === 'updates') {
    h = '<h2>Updates</h2><p class="line">GRID Launcher 0.9.10</p>' +
      (S.updateNotice ? '<p class="line update-line">GRID Launcher v0.9.11 is available <button class="g-btn" data-toast="Opens the release page in your browser">Open release</button><button class="g-btn secondary" data-dismiss-update>Dismiss</button></p>' : '<p class="line">Checked 2 hours ago — you have the latest release</p>') +
      '<p class="note">GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.</p>';
  } else {
    var theme = document.documentElement.getAttribute('data-theme') || 'light';
    h = '<h2>Appearance</h2>'+
      '<label class="setting"><span>Theme</span><span class="g-control"><select id="set-theme"><option value="light"'+(theme==='light'?' selected':'')+'>Light</option><option value="dark"'+(theme==='dark'?' selected':'')+'>Dark</option></select></span></label>'+
      '<label class="setting"><span>Background art</span><span class="check"><input type="checkbox" id="set-art"'+(S.fade<100?' checked':'')+'> Show the last game\u2019s art behind the views</span></label>'+
      '<label class="setting"><span>Background art fade<small>0 is full strength; 100 turns the art off</small></span><span style="display:flex;align-items:center;gap:10px"><input type="range" min="0" max="100" step="1" value="'+S.fade+'" id="set-fade"><span class="g-counts" id="fade-val">'+S.fade+'%'+(S.fade>=100?' · off':'')+'</span></span></label>'+
      '<label class="setting"><span>Background art blur</span><span style="display:flex;align-items:center;gap:10px"><input type="range" min="0" max="40" step="1" value="'+S.blur+'" id="set-blur"><span class="g-counts">'+S.blur+'</span></span></label>'+
      '<label class="setting"><span>Game labels</span><span class="g-switch" id="set-labels"><button data-l="always" class="'+(S.labels==='always'?'active':'')+'">Always visible</button><button data-l="hover" class="'+(S.labels==='hover'?'active':'')+'">Visible on hover</button></span></label>'+
      '<label class="setting"><span>Card size</span><span class="g-control"><select id="set-size"><option value="120"'+(S.cardW===120?' selected':'')+'>Small</option><option value="160"'+(S.cardW===160?' selected':'')+'>Medium</option><option value="200"'+(S.cardW===200?' selected':'')+'>Large</option></select></span></label>'+
      '<label class="setting"><span>Hardware acceleration<small>Applies after a restart</small></span><span class="check"><input type="checkbox" checked> Render on the graphics card</span></label>';
  }
  el.innerHTML = h;
  if (S.setPane === 'appearance') {
    $('set-theme').onchange = function(){ document.documentElement.setAttribute('data-theme', this.value); };
    $('set-fade').oninput = function(){ S.fade = +this.value; if (S.fade < 100) S.lastFade = S.fade; paintArt(G[S.lastViewed || 0]); $('fade-val').textContent = S.fade + '%' + (S.fade >= 100 ? ' · off' : ''); $('set-art').checked = S.fade < 100; };
    $('set-art').onchange = function(){ S.fade = this.checked ? (S.lastFade < 100 ? S.lastFade : 50) : 100; paintArt(G[S.lastViewed || 0]); renderSettings(); };
    $('set-blur').onchange = function(){ S.blur = +this.value; paintArt(G[S.lastViewed || 0]); renderSettings(); };
    $('set-labels').onclick = function(e){ var b = e.target.closest('[data-l]'); if (b) { e.preventDefault(); S.labels = b.getAttribute('data-l'); renderAll(); } };
    $('set-size').onchange = function(){ S.cardW = +this.value; syncSize(); renderAll(); };
  }
}

/* ---- library change dialog ---- */
function openLibChange(step) {
  S.libStep = step || 'pick';
  var d = $('libdlg'), h;
  if (S.libStep === 'pick') h = '<h3>Change library folder</h3><p class="note">Pick or type the new folder. It must be a full path, not inside the current folder.</p><label class="form"><span class="inline"><input class="field" value="/mnt/games/GRID"><button class="g-btn secondary" type="button">Browse…</button></span></label>' +
    '<div class="choice"><button class="g-btn" data-lib="move">Move existing files</button><button class="g-btn secondary" data-lib="fresh">Start fresh</button><button class="g-btn secondary" data-lib="close">Cancel</button></div>';
  else if (S.libStep === 'move') h = '<h3>Move existing files</h3><ul class="moves"><li><code>games/</code><span>Copied and checked</span></li><li><code>emulators/</code><span>Copied and checked</span></li><li><code>saves/</code><span>Copied and checked</span></li></ul><p class="line">Copies 48.2 GB; 1.3 TB free</p><p class="note">Installs, launches and cloud transfers wait while the move runs. Follow it in Downloads.</p><div class="choice"><button class="g-btn" data-lib="start">Start move</button><button class="g-btn secondary" data-lib="pick">Back</button></div>';
  else h = '<h3>Start fresh</h3><p class="line">Delete the old game files, or leave them on disk?</p><p class="note">Either way, games in the old folder leave the library. Emulators and saves are not touched.</p><div class="choice"><button class="g-btn secondary confirm" data-lib="delete">Delete…</button><button class="g-btn" data-lib="leave">Leave them</button><button class="g-btn secondary" data-lib="pick">Back</button></div>';
  d.innerHTML = h; $('libscrim').hidden = false;
}
function closeLibChange() { $('libscrim').hidden = true; }

/* ---- details ---- */
/* Both grids' Size selects show the one card size. */
function syncSize(){ document.querySelectorAll('[data-size-select]').forEach(function(s){ s.value = String(S.cardW); }); }
function openDetails(g, tab){
  S.detail = g; S.tab = tab || 'overview'; S.lastViewed = g.id; paintArt(g);
  $('scrim').hidden = false; renderDetails(); $('dialog').focus({ preventScroll: true });
}
function renderDetails(){
  var g = S.detail; if (!g) return;
  var acts;
  if (g.missing) acts = '<div class="g-chip status missing" style="align-self:center">Files not found</div><p class="g-meta-line">The game’s files are not in the library folder.</p><button class="g-btn">Locate…</button><button class="g-btn secondary destructive">Remove from library (keeps files)</button>';
  else if (g.installed) acts = '<button class="g-btn" data-d="play">Play</button>'+(g.update?'<button class="g-btn secondary">Update to v1.2</button>':'')+'<button class="g-btn secondary" data-d="uninstall">Uninstall</button>';
  else acts = '<button class="g-btn" data-d="install">'+(S.dl && S.dl.id===g.id?'Installing…':'Install')+'</button>';
  $('d-left').innerHTML = '<div class="g-cover" style="box-shadow:var(--shadow-lift);max-height:320px"><div class="g-art" style="background:'+g.c+';font-size:20px"><span>'+esc(g.t)+'</span></div></div>'+acts+
    '<p class="g-meta-line">'+(g.installed?'Last played 2 days ago':'Never played')+'</p><p class="g-meta-line">'+(g.p==='LINUX' ? 'Runs natively' : g.p==='WIN' ? 'Runs through Proton' : EMU[g.p]==='none' ? 'No emulator set up' : EMU[g.p])+'</p>';
  $('d-head').innerHTML = '<h2>'+esc(g.t)+'</h2><p class="g-header-line">'+PLAT[g.p]+' · 1998 · Action · <span class="g-rating"><span data-icon="star" data-size="14"></span>8.'+(g.id%9)+'</span></p><div class="row" style="display:flex;gap:6px">'+(S.playing===g.id?'<span class="g-chip playing">Playing</span>':'')+'<span class="g-chip">Identified</span>'+(g.installed?'<span class="g-chip">v1.1</span>':'')+'</div>';
  var tabs = [['overview','Overview'],['media','Media'],['saves','Saves'],['achievements','Achievements']];
  $('d-tabs').innerHTML = tabs.map(function(t){ return '<button role="tab" data-tab="'+t[0]+'" class="'+(S.tab===t[0]?'active':'')+'" aria-selected="'+(S.tab===t[0])+'">'+t[1]+'</button>'; }).join('');
  var p = $('d-panel');
  if (S.tab === 'overview') p.innerHTML = '<div class="overview"><div class="g-description" style="flex:1 1 340px"><p>'+esc(g.t)+' is a placeholder title for this prototype. The real Overview shows the server’s summary here, broken into paragraphs at about 70 characters a line.</p><p>Related games from the same franchise sit under the text as chips.</p></div><dl class="g-facts" style="flex:0 1 220px"><div><dt>Released</dt><dd>14 March 1998</dd></div><div><dt>Genres</dt><dd>Action, Adventure</dd></div><div><dt>Companies</dt><dd>Northwind Interactive</dd></div><div><dt>Size</dt><dd>612 MB</dd></div><div><dt>Version</dt><dd>'+(g.update?'v1.1 installed · v1.2 on server':'v1.1')+'</dd></div></dl></div>';
  else if (S.tab === 'media') p.innerHTML = '<p class="sec-title">Screenshots</p><div class="media-row">'+[0,1,2,3].map(function(i){ return '<div class="shot" style="background:'+g.c+';filter:hue-rotate('+(i*25)+'deg)"></div>'; }).join('')+'</div><p class="sec-title" style="margin-top:14px">Videos</p><div class="media-row"><div class="shot" style="background:hsl(220 20% 20%);display:grid;place-items:center;color:#fff"><span data-icon="play" data-size="28"></span></div></div>';
  else if (S.tab === 'saves') p.innerHTML = '<div class="g-switch" role="tablist" id="kind"><button data-k="save" class="'+(S.kind==='save'?'active':'')+'">Saves</button><button data-k="state" class="'+(S.kind==='state'?'active':'')+'">States</button></div>'+
    (S.kind==='save' ? '<div class="save-row"><span class="t">Memory card 1</span><span class="s">Uploaded today, 14:02</span><button class="small-btn">Restore</button></div><div class="save-row"><span class="t">Memory card 1</span><span class="s">Uploaded 3 days ago</span><button class="small-btn">Restore</button></div><div style="margin-top:12px"><button class="g-btn">Upload save</button></div>'
                     : '<div class="save-row"><span class="t">Slot 1</span><span class="s">Uploaded yesterday</span><button class="small-btn">Restore</button></div><div style="margin-top:12px"><button class="g-btn">Upload state</button></div>');
  else p.innerHTML = '<p class="note">Sign in to RetroAchievements in Settings to see progress here.</p>';
  gIcons($('dialog'));
}
function closeDetails(){ S.detail = null; $('scrim').hidden = true; }

/* ---- toasts ---- */
function toast(msg){
  var t = document.createElement('p'); t.className = 'g-toast'; t.textContent = msg; $('toasts').appendChild(t);
  setTimeout(function(){ t.remove(); }, 2600);
}

/* ---- events ---- */
document.addEventListener('click', function(e){
  var v = e.target.closest('[data-view-btn]'); if (v) { S.view = v.getAttribute('data-view-btn'); renderAll(); return; }
  var go = e.target.closest('[data-go]'); if (go) { S.view = go.getAttribute('data-go'); if (S.view==='emulators') S.emuPane='installed'; renderAll(); return; }
  if (e.target.closest('#chip')) { var m = $('menu'); m.hidden = !m.hidden; $('chip').setAttribute('aria-expanded', !m.hidden); return; }
  if (!e.target.closest('#menu')) $('menu').hidden = true;
  var fw = e.target.closest('[data-fw]'); if (fw) { var k = fw.getAttribute('data-fw'); if (S.fw[k] === 'unavailable') { toast('Nothing on the server lands in this emulator’s folders'); return; } S.fw[k] = 'requested'; toast('Installing '+PLAT[k]+' firmware for '+EMU[k]+'…'); renderEmulators(); setTimeout(function(){ S.fw[k] = 'installed'; toast(PLAT[k]+' firmware installed'); renderEmulators(); }, 1600); return; }
  var ud = e.target.closest('[data-undone]'); if (ud) { DONE.splice(+ud.getAttribute('data-undone'), 1); renderDownloads(); return; }
  var uq = e.target.closest('[data-unqueue]'); if (uq) { DLQ.splice(DLQ.indexOf(+uq.getAttribute('data-unqueue')), 1); renderAll(); return; }
  if (e.target.closest('[data-cancel]')) { toast('Cancelled ' + G[S.dl.id].t); addDone(G[S.dl.id].t, '', PLAT[G[S.dl.id].p] + ' · Cancelled'); S.dl = null; renderAll(); if (DLQ.length) startInstall(G[DLQ.shift()], true); return; }

  var tst = e.target.closest('[data-toast]'); if (tst) { toast(tst.getAttribute('data-toast')); return; }
  var at = e.target.closest('[data-addtab]'); if (at) { S.addTab = at.getAttribute('data-addtab'); renderEmulators(); return; }
  var ct = e.target.closest('[data-cat]'); if (ct) { var pr = ct.getAttribute('data-cat').split(':'); installCatalog(pr[0], pr.slice(1).join(':')); return; }
  if (e.target.closest('[data-umu]')) { S.umu = 'busy'; toast('Installing umu-launcher…'); renderEmulators(); setTimeout(function(){ S.umu = 'managed'; toast('umu-launcher installed'); renderEmulators(); }, 1600); return; }
  var ed = e.target.closest('[data-edit]'); if (ed) { S.emuPane = 'catalog'; S.addTab = 'manual'; renderEmulators(); return; }
  var cn = e.target.closest('[data-conn]'); if (cn) { var c2 = cn.getAttribute('data-conn'); S.connEdit = c2 === 'edit'; if (c2 === 'save') toast('Connected to romm.home.lan'); renderSettings(); return; }
  var ra = e.target.closest('[data-ra]'); if (ra) { var r = ra.getAttribute('data-ra'), u = ($('ra-user') || {}).value || ''; if (r === 'clear') { S.ra = { user: '', set: false }; toast('RetroAchievements cleared from every emulator'); } else if (!u.trim()) { toast('Enter a username first'); return; } else { S.ra = { user: u.trim(), set: true }; toast(r === 'login' ? 'Logged in as ' + u.trim() : 'Updated: RetroArch, DuckStation, Dolphin'); } renderSettings(); return; }
  if (e.target.closest('[data-dismiss-update]')) { S.updateNotice = false; renderSettings(); return; }
  if (e.target.closest('[data-libchange]')) { openLibChange('pick'); return; }
  var lb = e.target.closest('[data-lib]'); if (lb) { var l = lb.getAttribute('data-lib');
    if (l === 'close') closeLibChange(); else if (l === 'move' || l === 'fresh' || l === 'pick') openLibChange(l);
    else { closeLibChange(); toast(l === 'start' ? 'Library move started — follow it in Downloads' : l === 'leave' ? 'Library now at /mnt/games/GRID; old files left on disk' : 'Shows the folders and size to delete, then asks again'); }
    return; }
  if (e.target === $('libscrim')) { closeLibChange(); return; }
  var c = e.target.closest('[data-game]');
  if (c) {
    var g = G[+c.getAttribute('data-game')], act = e.target.closest('[data-act]');
    var a = act ? act.getAttribute('data-act') : 'open';
    if (a === 'primary') { if (g.missing) openDetails(g); else if (g.installed) { S.playing = g.id; toast('Launching ' + g.t + '…'); } else startInstall(g); }
    else if (a === 'cloud') openDetails(g, 'saves');
    else openDetails(g);
    return;
  }
  if (e.target === $('scrim') || e.target.closest('#close')) { closeDetails(); return; }
  var tb = e.target.closest('[data-tab]'); if (tb) { S.tab = tb.getAttribute('data-tab'); renderDetails(); return; }
  var kd = e.target.closest('[data-k]'); if (kd) { S.kind = kd.getAttribute('data-k'); renderDetails(); return; }
  var d = e.target.closest('[data-d]');
  if (d) { var a2 = d.getAttribute('data-d'); if (a2 === 'install') startInstall(S.detail); else if (a2 === 'play') { S.playing = S.detail.id; toast('Launching ' + S.detail.t + '…'); renderDetails(); } else if (a2 === 'uninstall') { if (d.textContent === 'Confirm uninstall') { S.detail.installed = false; toast(S.detail.t + ' uninstalled'); renderAll(); } else { d.textContent = 'Confirm uninstall'; d.classList.add('confirm'); } } }
});
document.addEventListener('input', function(e){ var k = e.target.getAttribute && e.target.getAttribute('data-search'); if (k) { S.q[k] = e.target.value; k === 'server' ? renderServer() : renderLibrary(); } });
document.addEventListener('change', function(e){ var df = e.target.getAttribute && e.target.getAttribute('data-default'); if (df) { DEFAULTS[df] = e.target.value; EMU[df] = e.target.value === 'RetroArch' ? 'RetroArch · ' + (CORES[df] || ['core'])[0] : e.target.value; renderEmulators(); renderServer(); return; } var cd = e.target.getAttribute && e.target.getAttribute('data-compat'); if (cd) { S.compatDefault = cd; return; } var ck = e.target.getAttribute && e.target.getAttribute('data-cloud'); if (ck) { S.cloud[ck] = e.target.checked; return; } if (e.target.hasAttribute && e.target.hasAttribute('data-size-select')) { S.cardW = +e.target.value; syncSize(); renderAll(); } });
document.addEventListener('keydown', function(e){
  if (e.key === 'Escape') { if (!$('libscrim').hidden) closeLibChange(); else if (S.detail) closeDetails(); $('menu').hidden = true; }
  if ((e.ctrlKey || e.metaKey) && /^[1-5]$/.test(e.key)) { e.preventDefault(); S.view = VIEWS[+e.key - 1]; renderAll(); }
});
document.querySelector('#app').addEventListener('mouseover', function(e){ var c = e.target.closest('[data-game]'); if (c && S.fade < 100) { var id = +c.getAttribute('data-game'); if (S.hoverId !== id) { S.hoverId = id; clearTimeout(S.hoverT); S.hoverT = setTimeout(function(){ S.lastViewed = id; paintArt(G[id]); }, 600); } } });

function renderAll(){ renderPills(); renderServer(); renderLibrary(); renderDownloads(); renderEmulators(); renderSettings(); renderDetails(); }
paintArt(G[0]); renderAll(); gIcons();
})();
</script>
</body>
</html>
"""

out = HTML.replace('__COVER_CSS__', gen.COVER_CSS).replace('__ICON_JS__', gen.ICON_JS).replace('__DATA__', json.dumps(data))
d = os.path.join(OUT, 'CurrentApp'); os.makedirs(d, exist_ok=True)
open(os.path.join(d, 'preview.html'), 'w').write(out)
print('CurrentApp written', len(out))

# The proposed design: one Systems tab in Settings replaces the Emulators view.
HERE = os.path.dirname(os.path.abspath(__file__))
NEXT_CSS = open(os.path.join(HERE, 'next.css')).read()
NEXT_JS = open(os.path.join(HERE, 'panes_systems.js')).read()
def sub(t, a, b):
    assert a in t, a[:70]
    return t.replace(a, b, 1)
nxt = out
nxt = sub(nxt, 'subtitle="The app as it is today, clickable: every view, rail pane, Details and dialogs"', 'subtitle="Proposed: emulators and Windows setup in one Systems tab under Settings"')
nxt = sub(nxt, '<title>Current app</title>', '<title>Proposed app</title>')
nxt = sub(nxt, '</style></head>', NEXT_CSS + '\n</style></head>')
nxt = sub(nxt, '/* ---- details ---- */', NEXT_JS + '\n/* ---- details ---- */')
nxt = sub(nxt, "var VIEWS = ['library','server','downloads','emulators','settings'];", "var VIEWS = ['library','server','downloads','settings'];")
nxt = sub(nxt, "var S = { view: 'server',", "var S = { view: 'settings',")
# Settings rail: Systems joins, and the panes run alphabetically.
nxt = sub(nxt, "var panes = [['connection','Connection'],['library','Library'],['cloud','Cloud saves'],['ra','RetroAchievements'],['updates','Updates'],['appearance','Appearance']];",
          "var panes = [['appearance','Appearance'],['cloud','Cloud saves'],['connection','Connection'],['library','Library'],['ra','RetroAchievements'],['systems','Systems'],['updates','Updates']];")
nxt = sub(nxt, "  var el = $('set-pane'), h;\n", "  var el = $('set-pane'), h;\n  if (S.setPane === 'systems') { renderSystems(el); return; }\n")
nxt = sub(nxt, "S.view = go.getAttribute('data-go'); if (S.view==='emulators') S.emuPane='installed';", "S.view = go.getAttribute('data-go'); if (S.view==='emulators') { S.view='settings'; S.setPane='systems'; }")
nxt = sub(nxt, "$('srv-emu').textContent = 'Emulator: ' + EMU[S.platform];", "$('srv-emu').textContent = EMU[S.platform]==='none' ? 'No default emulator' : 'Emulator: ' + EMU[S.platform]; var fwN = { PS: 3, SAT: 2 }[S.platform], fwS = SY.fw[S.platform]; $('srv-fw').textContent = !fwN ? 'No server firmware' : (EMU[S.platform]==='none' ? 'Firmware: ' + fwN + ' files — no default emulator' : fwS==='installed' ? 'Firmware: ' + fwN + ' files · installed' : 'Firmware: ' + fwN + ' files');")
nxt = sub(nxt, '<button class="g-chip-outline" data-go="emulators">Firmware: 3 files · installed</button>', '<button class="g-chip-outline" data-go="emulators" id="srv-fw">Firmware: 3 files · installed</button>')
nxt = sub(nxt, "/^[1-5]$/.test(e.key)", "/^[1-4]$/.test(e.key)")
FRESH_JS = open(os.path.join(HERE, 'fresh.js')).read()
nxt = sub(nxt, '      <p class="g-panel legend">Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled</p>\n', '')
SH_JS = open(os.path.join(HERE, 'serverhead.js')).read()
SH_CSS = open(os.path.join(HERE, 'serverhead.css')).read()
nxt = sub(nxt, '</style></head>', SH_CSS + '\n</style></head>')
nxt = sub(nxt, """      <div class="body">
        <div class="g-panel g-head">
          <header class="g-platform-header">
            <h2 id="srv-title"></h2>""", """      <div class="body sbody" id="srv-body">
        <div class="g-panel g-head shead" id="srv-head"><div class="shead-fold"><div class="shead-fold-in">
          <header class="g-platform-header">
            <h2 id="srv-title"></h2>""")
nxt = sub(nxt, """          </header>
          <div class="g-toolbar">
            <input class="g-search" type="search" placeholder="Search title, platform or genre" aria-label="Search this platform" data-search="server">""", """          </header></div></div>
          <div class="g-toolbar">
            <input class="g-search" type="search" placeholder="Search title, platform or genre" aria-label="Search this platform" data-search="server">""")
nxt = sub(nxt, """        <div class="grid" id="srv-grid"></div>""", """        <div class="sscroll" id="srv-scroll"><div class="grid" id="srv-grid"></div></div>\n        <div class="discover" id="discover" hidden></div>""")
nxt = sub(nxt, """      <div class="body">
        <div class="g-panel g-head">
          <header class="g-platform-header"><h2 id="lib-title">All games</h2><p class="g-counts" id="lib-count"></p></header>
          <div class="g-toolbar">""", """      <div class="body sbody" id="lib-body">
        <div class="g-panel g-head shead" id="lib-head"><div class="shead-fold"><div class="shead-fold-in">
          <header class="g-platform-header"><h2 id="lib-title">All games</h2><p class="g-counts" id="lib-count"></p></header></div></div>
          <div class="g-toolbar">""")
nxt = sub(nxt, """        <div class="grid" id="lib-grid"></div>""", """        <div class="sscroll" id="lib-scroll"><div class="grid" id="lib-grid"></div></div>""")
nxt = sub(nxt, 'function toast(msg){', 'function centredToastUnused(msg){')
FRESH_CSS = open(os.path.join(HERE, 'fresh.css')).read()
nxt = sub(nxt, '</style></head>', FRESH_CSS + '\n</style></head>')
DISC_JS = open(os.path.join(HERE, 'discover.js')).read()
TT_JS = open(os.path.join(HERE, 'themetoggle.js')).read()
TT_CSS = open(os.path.join(HERE, 'themetoggle.css')).read()
nxt = sub(nxt, '</style></head>', TT_CSS + '\n</style></head>')
nxt = sub(nxt, """    <div class="g-session">
      <span class="g-status-dot online" aria-hidden="true"></span>""", """    <div class="g-session">
      <button class="g-icon-btn theme-toggle" data-theme-toggle aria-label="Switch to dark mode"></button>
      <span class="g-status-dot online" aria-hidden="true"></span>""")
nxt = sub(nxt, '  <div class="g-scrim" id="scrim" hidden>', '  <button class="g-icon-btn theme-toggle theme-start" id="theme-start" data-theme-toggle aria-label="Switch to dark mode" hidden></button>\n  <div class="g-scrim" id="scrim" hidden>')
DISC_CSS = open(os.path.join(HERE, 'discover.css')).read()
nxt = sub(nxt, '</style></head>', DISC_CSS + '\n</style></head>')
nxt = sub(nxt, '/* ---- details ---- */', FRESH_JS + '\n' + SH_JS + '\n' + DISC_JS + '\n' + TT_JS + '\n/* ---- details ---- */')
nxt = sub(nxt, "var S = { view: 'settings', platform: 'PS',", "var S = { view: 'settings', platform: 'DISCOVER',")
nxt = sub(nxt, '  <div class="toasts" id="toasts" aria-live="polite"></div>', '  <div class="notes" id="notes" aria-live="polite"><aside class="note-card setup-card" id="setup-card" hidden></aside></div>\n  <div class="fresh" id="fresh" hidden></div>')
nxt = sub(nxt, "paintArt(G[0]); renderAll(); gIcons();", "freshReset(); paintArt(null); $('art').style.opacity = 0; renderAll(); gIcons(); freshShow('connect');")
nxt = sub(nxt, 'subtitle="Proposed: emulators and Windows setup in one Systems tab under Settings"', 'subtitle="Proposed: starts as a new user; emulators and Windows setup in one Systems tab under Settings"')
d = os.path.join(OUT, 'ProposedApp'); os.makedirs(d, exist_ok=True)
open(os.path.join(d, 'preview.html'), 'w').write(nxt)
print('ProposedApp written', len(nxt))

# Game backups: ProposedApp plus backup drives, multi-select, the backup
# dialog, Library › Backup drives and an offline mode. Its own card, so the
# Proposed app card keeps showing what is on main.
BK_JS = open(os.path.join(HERE, 'backups.js')).read()
BK_CSS = open(os.path.join(HERE, 'backups.css')).read()
bk = nxt
bk = sub(bk, 'subtitle="Proposed: starts as a new user; emulators and Windows setup in one Systems tab under Settings"', 'subtitle="Game backups: copy games to a USB drive and install them from it, with or without a server"')
bk = sub(bk, '<title>Proposed app</title>', '<title>Game backups</title>')
bk = sub(bk, '</style></head>', BK_CSS + '\n</style></head>')
bk = sub(bk, '/* ---- details ---- */', BK_JS + '\n/* ---- details ---- */')
bk = sub(bk, "[['appearance','Appearance'],['cloud','Cloud saves'],", "[['appearance','Appearance'],['backup','Backup drives'],['cloud','Cloud saves'],")
bk = sub(bk, "  if (S.setPane === 'systems') { renderSystems(el); return; }\n", "  if (S.setPane === 'systems') { renderSystems(el); return; }\n  if (S.setPane === 'backup') { renderBackupPane(el); return; }\n")
bk = sub(bk, '<header class="g-platform-header"><h2 id="lib-title">All games</h2><p class="g-counts" id="lib-count"></p></header>', '<header class="g-platform-header"><h2 id="lib-title">All games</h2><p class="g-counts" id="lib-count"></p><div class="g-chips" id="lib-chips" hidden></div></header>')
SIZE = '<label class="g-control"><span>Size</span><select data-size-select><option value="120">Small</option><option value="160" selected>Medium</option><option value="200">Large</option></select></label>'
assert bk.count(SIZE) == 2
bk = bk.replace(SIZE, SIZE + '\n            <button class="small-btn bk-select-btn" data-bk-select aria-pressed="false">Select</button>')
bk = sub(bk, '<nav class="g-rail" id="srv-rail" aria-label="Server platforms"></nav>', '<nav class="g-rail" id="srv-rail" aria-label="Server platforms"></nav>\n      <div class="g-panel bk-off" id="bk-srv-off" hidden><div class="bk-off-in"><span class="bk-drive-icon" data-icon="cloud" data-size="24"></span><h2>Not connected to a server</h2><p class="note">Your Library and backup drives work without one. Connect to browse the server and backup games.</p><button class="g-btn" data-bk-connect>Connect to a server…</button></div></div>')
bk = sub(bk, '<div class="g-scrim" id="libscrim" hidden>', '<div class="g-scrim" id="bkscrim" hidden><div class="g-dialog bkdlg" role="dialog" aria-modal="true" aria-labelledby="bk-dlg-title" id="bkdlg"></div></div>\n  <div class="g-scrim" id="libscrim" hidden>')
bk = sub(bk, "freshReset(); paintArt(null); $('art').style.opacity = 0; renderAll(); gIcons(); freshShow('connect');", "bkBoot();")
d = os.path.join(OUT, 'GameBackups'); os.makedirs(d, exist_ok=True)
open(os.path.join(d, 'preview.html'), 'w').write(bk)
print('GameBackups written', len(bk))

# External stores: GameBackups (which is what main has) plus GOG, Epic Games,
# Amazon Games and Steam in the Library, Settings › Stores, the store login
# window and GOG offline installers. Its own card, so Game backups keeps
# showing what is on main.
ST_JS = open(os.path.join(HERE, 'stores.js')).read() + '\n' + open(os.path.join(HERE, 'stores_bk.js')).read()
ST_CSS = open(os.path.join(HERE, 'stores.css')).read()
st = bk
st = sub(st, 'subtitle="Game backups: copy games to a USB drive and install them from it, with or without a server"', 'subtitle="Game stores: GOG (and its offline installers), Epic Games, Amazon Games and Steam in the Library, with Settings › Stores"')
st = sub(st, '<title>Game backups</title>', '<title>Game stores</title>')
st = sub(st, '</style></head>', ST_CSS + '\n</style></head>')
st = sub(st, '/* ---- details ---- */', ST_JS + '\n/* ---- details ---- */')
st = sub(st, "['ra','RetroAchievements'],['systems','Systems'],", "['ra','RetroAchievements'],['stores','Stores'],['systems','Systems'],")
st = sub(st, "  if (S.setPane === 'backup') { renderBackupPane(el); return; }\n", "  if (S.setPane === 'backup') { renderBackupPane(el); return; }\n  if (S.setPane === 'stores') { renderStoresPane(el); return; }\n")
st = sub(st, '  <div class="g-scrim" id="libscrim" hidden>', '  <div class="g-scrim" id="stscrim" hidden><div class="g-dialog bkdlg stdlg" role="dialog" aria-modal="true" id="stdlg"></div></div>\n  <div class="st-login" id="stlogin" hidden></div>\n  <div class="g-scrim" id="libscrim" hidden>')
st = sub(st, 'bkBoot();', 'stBoot();')
d = os.path.join(OUT, 'GameStores'); os.makedirs(d, exist_ok=True)
open(os.path.join(d, 'preview.html'), 'w').write(st)
print('GameStores written', len(st))
