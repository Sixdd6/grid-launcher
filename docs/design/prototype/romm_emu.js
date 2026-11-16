/* ==== RomM › Emulators (2026-11-15): the server's Emulators platform as a row in the
   RomM group, and Settings › Library's Open config folder.

   - The row shows while RomM is connected and the server has an Emulators platform
     with files, sorted by name among the platform rows like any other.
   - Its view lists the files, not covers: emulators whose builds follow
     docs/server-emulator-packages.md (GRID installs the newest build for this
     computer and registers it, so Settings › Systems can use it and offer its
     updates), then the other files (installed as plain files in the library's
     Emulators folder), then the builds GRID doesn't install on this computer.
   - Emulator files never join RomM › Installed, All games, Recent, Updates or
     Discover: they are not games. In this prototype they are not in G at all. ==== */
var RE = {
  has: true, load: 'ok', os: 'Linux x64', confirm: null, timers: {},
  emus: [
    { id: 'pcsx2', name: 'PCSX2', sys: 'PlayStation 2', file: 'PCSX2 (v2.0.2) (arch-linux-x64).AppImage', ver: 'v2.0.2', size: '38 MB', mb: 38, st: 'update', have: 'v1.6.0' },
    { id: 'pico8', name: 'Pico-8', sys: 'Pico-8', file: 'Pico-8 (v0.2.7a6) (arch-linux-x64).zip', ver: 'v0.2.7a6', size: '6.1 MB', mb: 6.1, st: 'none' },
    { id: 'retroarch', name: 'RetroArch', sys: 'Many systems, through its cores', file: 'RetroArch (v1.19.1) (arch-linux-x64).AppImage', ver: 'v1.19.1', size: '156 MB', mb: 156, st: 'installed', have: 'v1.19.1' },
    { id: 'ymir', name: 'Ymir', sys: 'Sega Saturn', file: 'Ymir (v0.1.8) (arch-linux-x64).AppImage', ver: 'v0.1.8', size: '24 MB', mb: 24, st: 'none' },
    /* Its file is tagged x64, but the program inside is an ARM64 build: the install fails. */
    { id: 'duck', name: 'DuckStation', sys: 'PlayStation', file: 'DuckStation (v0.1.9226) (arch-linux-x64).zip', ver: 'v0.1.9226', size: '42 MB', mb: 42, st: 'none', bad: 'Linux ARM64', catalog: true }
  ],
  other: [
    { id: 'xemu', file: 'xemu-v0.8.5-x86_64.AppImage', folder: 'xbox', size: '18 MB', mb: 18, why: 'Not named for GRID', st: 'installed' },
    { id: 'cemu', file: 'Cemu (v2.4) (arch-linux).AppImage', folder: 'wiiu', size: '52 MB', mb: 52, why: 'Its arch tag has no processor', st: 'none' },
    { id: 'ares', file: 'Ares Nightly (v141) (arch-linux-x64).AppImage', folder: 'multi', size: '31 MB', mb: 31, why: 'GRID doesn’t know an emulator called “Ares Nightly”', st: 'none' }
  ],
  builds: [
    ['PCSX2 (v2.0.2) (arch-windows-x64).zip', 'ps2', '31 MB', 'For Windows x64'],
    ['PCSX2 (v1.6.0) (arch-linux-x64).AppImage', 'ps2', '29 MB', 'Older than v2.0.2'],
    ['Pico-8 (v0.2.7a6) (arch-linux-armhf).zip', 'pico', '5.8 MB', 'For Linux 32-bit ARM'],
    ['Pico-8 (v0.2.7a6) (arch-windows-x64).zip', 'pico', '6.4 MB', 'For Windows x64'],
    ['Dolphin (v2412) (arch-windows-x64).7z', 'multi', '19 MB', 'For Windows x64'],
    ['Dolphin (v2412) (arch-macos-universal).7z', 'multi', '44 MB', 'For macOS (universal)']
  ]
};
/* The row's count: what this computer can install (the view's two lists). */
function reCount() { return RE.load === 'none' ? 0 : RE.emus.length + RE.other.length; }
function reShows() { return rmOn() && RE.has; }
function reBusy(x) { return x.st === 'queued' || x.st === 'dl' || x.st === 'unpack'; }
function reInstalledN() { return RE.emus.concat(RE.other).filter(function (x) { return x.st === 'installed' || x.st === 'update'; }).length; }

/* ---- the rail: one more platform row, by name ---- */
var reOrigPlats = rmPlats;
rmPlats = function () {
  var l = reOrigPlats();
  if (reShows()) { l.push('emulators'); l.sort(function (a, b) { return (PLAT[a] || 'Emulators').localeCompare(PLAT[b] || 'Emulators'); }); }
  return l;
};
var reOrigRail = rmRailHTML;
rmRailHTML = function () {
  var h = reOrigRail();
  return h.replace(/(data-key="platform:emulators"[^>]*><span class="g-rail-label">)undefined(<\/span><span class="g-rail-count">)\d+/, '$1Emulators$2' + reCount());
};
/* Selection: the row goes with RomM or with the server's Emulators platform. */
var reOrigFix = rmFixSel;
rmFixSel = function () {
  if (S.view === 'server' && S.platform === 'emulators') {
    if (!rmOn()) { S.view = 'library'; S.libFilter = rmInRail() ? 'romm:installed' : 'all'; }
    else if (!RE.has) { S.view = 'library'; S.libFilter = 'romm:installed'; }
    return;
  }
  reOrigFix();
};

/* ---- the view ---- */
function reProto() {
  var o = [['ok', 'Prototype: files loaded'], ['loading', 'Loading'], ['error', 'Can’t load the list'], ['none', 'Nothing for this computer'], ['gone', 'No Emulators platform (row hidden)']];
  return '<select class="small-btn bk-proto" data-re-proto aria-label="Prototype: Emulators state">' + o.map(function (x) { return '<option value="' + x[0] + '"' + (RE.load === x[0] ? ' selected' : '') + '>' + esc(x[1]) + '</option>'; }).join('') + '</select>';
}
function reMatch(x) {
  var q = S.q.server.trim().toLowerCase();
  return !q || [x.name, x.sys, x.file, x.folder].some(function (s) { return s && s.toLowerCase().indexOf(q) >= 0; });
}
function reBar(x) {
  if (x.st === 'queued') return '<p class="re-line">Queued · waits for the download before it</p>';
  if (x.st === 'unpack') return '<div class="re-prog"><span>Installing · checking the program</span><div class="bar"><b class="re-indet"></b></div></div>';
  var got = Math.round(x.mb * x.pct) / 100;
  return '<div class="re-prog"><span>Downloading · ' + Math.round(x.pct) + '% · ' + got.toFixed(got < 10 ? 1 : 0) + ' of ' + x.size + '</span><div class="bar"><b style="width:' + x.pct + '%"></b></div></div>';
}
function reConfirm(x, isEmu) {
  var what = isEmu ? x.name + ' ' + x.have : x.file;
  var p = isEmu ? 'GRID deletes it from the emulators folder in your library. Games and saves stay. ' + esc(x.sys === 'Many systems, through its cores' ? 'Games set to ' + x.name : x.sys + ' games set to it') + ' can’t start until another emulator is picked.'
    : 'GRID deletes it from the Emulators folder in your library.';
  return '<div class="re-confirm" role="alertdialog"><b>Uninstall ' + esc(what) + '?</b><p>' + p + '</p><div class="form-actions"><button class="small-btn danger" data-re-unok="' + x.id + '">Uninstall</button><button class="small-btn" data-re-uncancel>Cancel</button></div></div>';
}
function reEmuRow(e) {
  var line = '', acts = '', extra = '';
  var meta = '<p class="re-line">' + e.ver + ' · ' + e.size + '</p><p class="re-line"><code>' + esc(e.file) + '</code></p>';
  if (e.st === 'none') { acts = '<button class="small-btn primary" data-re-install="' + e.id + '">Install</button>'; if (e.catalog) extra = '<p class="re-line">GRID’s catalog copy of ' + esc(e.name) + ' is installed. This build installs beside it as “' + esc(e.name) + ' (server)”.</p>'; }
  else if (e.st === 'installed') { acts = '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Installed</span><button class="small-btn" data-re-un="' + e.id + '">Uninstall…</button>'; extra = '<p class="re-line">Set up in Settings › Systems · <button class="linkish" data-re-systems>Open in Systems</button></p>'; }
  else if (e.st === 'update') { acts = '<button class="small-btn primary" data-re-install="' + e.id + '">Update to ' + e.ver + '</button><button class="small-btn" data-re-un="' + e.id + '">Uninstall…</button>'; extra = '<p class="re-line"><b class="re-upd">Installed: ' + e.have + '</b> · ' + e.ver + ' is on RomM · <button class="linkish" data-re-systems>Open in Systems</button></p>'; }
  else if (e.st === 'failed') { acts = '<button class="small-btn" data-re-install="' + e.id + '">Retry</button>'; extra = '<p class="re-err">Couldn’t install. The program in this file is built for ' + e.bad + ', not ' + RE.os + '. Check the file’s arch tag on RomM. Nothing was set up.</p>'; }
  else { acts = e.st === 'unpack' ? '' : '<button class="small-btn" data-re-cancel="' + e.id + '">Cancel</button>'; extra = reBar(e); }
  return '<li class="re-row' + (e.st === 'failed' ? ' bad' : '') + '"><span class="re-tile" aria-hidden="true">' + esc(e.name.charAt(0)) + '</span><div class="re-main"><div class="re-t"><b>' + esc(e.name) + '</b><span class="re-sys">' + esc(e.sys) + '</span></div>' + meta + extra +
    (RE.confirm === e.id ? reConfirm(e, true) : '') + '</div><div class="re-acts">' + acts + '</div></li>';
}
function reOtherRow(f) {
  var acts = '', extra = '';
  if (f.st === 'none') acts = '<button class="small-btn" data-re-install="' + f.id + '">Install</button>';
  else if (f.st === 'installed') { acts = '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Installed</span><button class="small-btn" data-re-un="' + f.id + '">Uninstall…</button>'; extra = '<p class="re-line">In your library’s Emulators folder · <button class="linkish" data-re-folder>Show folder</button></p>'; }
  else { acts = f.st === 'unpack' ? '' : '<button class="small-btn" data-re-cancel="' + f.id + '">Cancel</button>'; extra = reBar(f); }
  return '<li class="re-row"><span class="re-tile file" aria-hidden="true">·</span><div class="re-main"><div class="re-t"><b class="re-file">' + esc(f.file) + '</b></div><p class="re-line">' + f.folder + ' folder · ' + f.size + ' · ' + f.why + '</p>' + extra +
    (RE.confirm === f.id ? reConfirm(f, false) : '') + '</div><div class="re-acts">' + acts + '</div></li>';
}
function reBuilds() {
  return '<details class="re-more"><summary>' + bkPlural(RE.builds.length, 'other build') + ' GRID doesn’t install on this computer</summary><p class="note">GRID installs the newest build for this computer’s system and processor (' + RE.os + ').</p><ul class="re-list">' +
    RE.builds.map(function (b) { return '<li class="re-row dim"><span class="re-tile file" aria-hidden="true">·</span><div class="re-main"><div class="re-t"><b class="re-file">' + esc(b[0]) + '</b></div><p class="re-line">' + b[1] + ' folder · ' + b[2] + ' · ' + b[3] + '</p></div></li>'; }).join('') + '</ul></details>';
}
function reRender() {
  var head = document.querySelector('[data-view=server] .g-chips'), body = $('srv-grid');
  $('srv-title').textContent = 'Emulators';
  var emus = RE.emus.filter(reMatch), other = RE.other.filter(reMatch), q = S.q.server.trim();
  var none = RE.load === 'none';
  $('srv-count').textContent = RE.load === 'loading' ? 'Loading…' : RE.load === 'error' ? 'On ' + rmHost() : none ? 'No builds for this computer' :
    bkPlural(RE.emus.length, 'emulator') + ' · ' + bkPlural(RE.other.length, 'other file') + ' · ' + reInstalledN() + ' installed';
  Array.prototype.forEach.call(head.children, function (c) { if (!c.classList.contains('re-chips')) c.style.display = 'none'; });
  var rc = head.querySelector('.re-chips'); if (!rc) { rc = document.createElement('span'); rc.className = 're-chips'; head.appendChild(rc); }
  head.hidden = false;
  rc.innerHTML = '<span class="g-chip-outline" title="GRID installs the newest build for this computer’s system and processor">Builds for ' + RE.os + '</span><button class="g-chip-outline" data-re-systems>Settings › Systems</button>' + reProto();
  document.querySelectorAll('[data-view=server] [data-size-select], [data-view=server] [data-bk-select], [data-view=server] .bk-plat').forEach(function (el) { (el.closest('label') || el).style.display = 'none'; });
  var s = document.querySelector('[data-view=server] .g-search'); if (s && !document.querySelector('#srv-head.folded')) s.placeholder = 'Search emulators or files';
  body.className = 're-wrap';
  body.style.removeProperty('--card-w');
  if (RE.load === 'loading') { body.innerHTML = '<div class="g-panel re-sec"><h3><span class="st-spin"></span> Loading the Emulators platform from ' + esc(rmHost()) + '…</h3><div class="re-skel"><i></i><i></i><i></i><i></i></div></div>'; return; }
  if (RE.load === 'error') { body.innerHTML = stState({ icon: rmMark('lg'), tone: 'warn', h: 'Can’t load the Emulators platform', p: esc(rmHost()) + ' didn’t answer. Installed emulators still work.', acts: '<button class="g-btn secondary" data-re-retry>Retry</button>' }); return; }
  if (none) {
    body.innerHTML = stState({ icon: rmMark('lg'), h: 'Nothing here for this computer', p: 'RomM’s Emulators platform has ' + bkPlural(RE.builds.length, 'build') + ', all for other systems or processors. GRID installs builds for ' + RE.os + '.', acts: '<button class="g-btn secondary" data-re-guide>How to name builds</button>' }) + '<div class="g-panel re-sec">' + reBuilds() + '</div>';
    gIcons(body); return;
  }
  if (q && !emus.length && !other.length) { body.innerHTML = '<p class="g-panel empty">No emulators or files match “' + esc(q) + '”</p>'; return; }
  var h = '';
  if (emus.length) h += '<section class="g-panel re-sec"><h3>Emulators</h3><p class="note">Named for GRID. Each installs as an emulator that Settings › Systems sets up, and RomM’s newer builds show as updates.</p><ul class="re-list">' + emus.map(reEmuRow).join('') + '</ul></section>';
  if (other.length) h += '<section class="g-panel re-sec"><h3>Other files</h3><p class="note">These don’t follow GRID’s naming, so GRID installs them as plain files in your library’s Emulators folder. GRID doesn’t set them up for a system or check them for updates. <button class="linkish" data-re-guide>How to name builds</button></p><ul class="re-list">' + other.map(reOtherRow).join('') + '</ul></section>';
  if (!q) h += '<section class="g-panel re-sec">' + reBuilds() + '</section>';
  body.innerHTML = h; gIcons(body);
}
var reOrigSrv = renderServer;
renderServer = function () {
  var head = document.querySelector('[data-view=server] .g-chips');
  if (S.view === 'server' && S.platform === 'emulators') {
    /* The base draws a games platform; the Emulators view replaces its grid, chips and controls. */
    reOrigSrv(); reRender(); return;
  }
  if (head) { var rc = head.querySelector('.re-chips'); if (rc) rc.remove(); Array.prototype.forEach.call(head.children, function (c) { c.style.display = ''; }); }
  document.querySelectorAll('[data-view=server] [data-size-select], [data-view=server] [data-bk-select], [data-view=server] .bk-plat').forEach(function (el) { (el.closest('label') || el).style.display = ''; });
  reOrigSrv();
};
/* The folded header's search says what it searches. */
var reOrigFoldPh = SRV_FOLD.set;
SRV_FOLD.set = function (f) { reOrigFoldPh(f); if (S.platform === 'emulators') document.querySelector('#srv-head .g-search').placeholder = f ? 'Search Emulators…' : 'Search emulators or files'; };

/* ---- installs: progress on the row (the Downloads view has the same row) ---- */
function reFind(id) { return RE.emus.concat(RE.other).filter(function (x) { return x.id === id; })[0]; }
function reTick(x) {
  if (x.st === 'queued') { x.st = 'dl'; x.pct = 0; }
  else if (x.st === 'dl') { x.pct = Math.min(100, x.pct + Math.max(6, 900 / x.mb)); if (x.pct >= 100) x.st = 'unpack'; }
  else if (x.st === 'unpack') {
    clearInterval(RE.timers[x.id]); delete RE.timers[x.id];
    var emu = RE.emus.indexOf(x) >= 0;
    if (x.bad) { x.st = 'failed'; toast('Couldn’t install ' + x.name + ': its program is built for ' + x.bad + '. Nothing was set up.', 'error'); }
    else if (emu) { var upd = !!x.have; x.st = 'installed'; x.have = x.ver; toast(upd ? x.name + ' updated to ' + x.ver : x.name + ' ' + x.ver + ' installed from RomM · set up for ' + (x.sys === 'Many systems, through its cores' ? 'its systems' : x.sys)); }
    else { x.st = 'installed'; toast(x.file + ' installed in your library’s Emulators folder'); }
  }
  if (S.view === 'server' && S.platform === 'emulators') reRender();
}
function reStart(x) {
  if (x.st === 'failed') x.bad = null; /* the prototype's retry succeeds, as if the file was fixed on RomM */
  var busy = RE.emus.concat(RE.other).some(function (y) { return y.st === 'dl' || y.st === 'unpack'; });
  x.st = busy ? 'queued' : 'dl'; x.pct = 0;
  RE.timers[x.id] = setInterval(function () { if (x.st === 'queued' && RE.emus.concat(RE.other).some(function (y) { return y !== x && (y.st === 'dl' || y.st === 'unpack'); })) return; reTick(x); }, 350);
}

/* ---- Settings › Library: Open config folder, under the library folder ---- */
var reOrigSet = renderSettings;
renderSettings = function () {
  reOrigSet();
  if (S.view !== 'settings' || S.setPane !== 'library') return;
  var el = $('set-pane'); if (!el || el.querySelector('[data-re-cfg]')) return;
  el.insertAdjacentHTML('beforeend', '<div class="sub"><h3>Config folder</h3><p class="note">GRID’s settings file lives here. Games, emulators and saves are in the library folder.</p><code class="path">/home/player/.config/grid-launcher</code><div class="form-actions"><button class="g-btn secondary" data-re-cfg>Open config folder</button></div></div>');
};

document.addEventListener('click', function (e) {
  var t = e.target, b, x;
  if ((b = t.closest('[data-re-install]'))) { x = reFind(b.getAttribute('data-re-install')); RE.confirm = null; reStart(x); reRender(); return; }
  if ((b = t.closest('[data-re-cancel]'))) { x = reFind(b.getAttribute('data-re-cancel')); clearInterval(RE.timers[x.id]); delete RE.timers[x.id]; x.st = x.have ? 'update' : 'none'; toast('Cancelled ' + (x.name || x.file)); reRender(); return; }
  if ((b = t.closest('[data-re-un]'))) { RE.confirm = b.getAttribute('data-re-un'); reRender(); return; }
  if (t.closest('[data-re-uncancel]')) { RE.confirm = null; reRender(); return; }
  if ((b = t.closest('[data-re-unok]'))) { x = reFind(b.getAttribute('data-re-unok')); RE.confirm = null; x.st = 'none'; x.have = null; toast((x.name || x.file) + ' uninstalled'); reRender(); return; }
  if (t.closest('[data-re-systems]')) { S.view = 'settings'; S.setPane = 'systems'; renderAll(); return; }
  if (t.closest('[data-re-folder]')) { toast('Opens the Emulators folder in your library in the file manager'); return; }
  if (t.closest('[data-re-guide]')) { toast('Opens the guide to packaging emulators for GRID in your browser'); return; }
  if (t.closest('[data-re-retry]')) { RE.load = 'loading'; reRender(); setTimeout(function () { RE.load = 'ok'; if (S.platform === 'emulators') reRender(); }, 900); return; }
  if (t.closest('[data-re-cfg]')) { toast('Opens /home/player/.config/grid-launcher in the file manager'); return; }
});
document.addEventListener('change', function (e) {
  var t = e.target;
  if (!(t.hasAttribute && t.hasAttribute('data-re-proto'))) return;
  if (t.value === 'gone') { RE.has = false; RE.load = 'ok'; toast('Prototype: the server has no Emulators platform, so the row is hidden'); renderAll(); return; }
  RE.load = t.value; renderAll();
});
