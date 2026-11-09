/* ==== Game backups: copy games from the server to a removable drive, with
   their art, details, emulators and firmware, and install them from that
   drive on any computer, with or without a server.

   A drive is set up once (Settings › Backup drives): GRID makes one
   `gridpack` folder at the drive's root and never touches anything else on
   it. A drive is recognised by `gridpack/gridpack.json`. ==== */
var BK = {
  drives: [
    { id: 't7', label: 'SAMSUNG T7', fs: 'exFAT', size: 1000, other: 361.4, connected: true, init: true,
      name: 'Couch kit', created: '2026-10-12', last: '2026-11-02',
      games: [0, 3, 5, 8, 10, 15], emus: ['DuckStation', 'Dolphin'], fw: ['PS'], os: ['linux', 'windows'] },
    { id: 'clas', label: 'ClasPlatf', fs: 'exFAT', size: 64, other: 2.1, connected: true, init: true,
      name: 'Classic Platformers', created: '2026-10-10', last: '2026-10-10',
      games: [4, 11, 1, 6], emus: ['RetroArch'], fw: [], os: ['linux'] },
    { id: 'king', label: 'KINGSTON', fs: 'FAT32', size: 32, other: 0.8, connected: true, init: false,
      name: '', games: [], emus: [], fw: [], os: [] },
    { id: 'wd', label: 'WD Elements', fs: 'NTFS', size: 2000, other: 840, connected: false, init: true,
      name: 'Everything', created: '2026-09-01', last: '2026-09-28', seen: '3 days ago',
      games: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], emus: ['DuckStation', 'Dolphin', 'PCSX2', 'RetroArch', 'Ymir', 'GE-Proton10-17'], fw: ['PS', 'SAT'], os: ['linux', 'windows'] }
  ],
  sel: { view: null, ids: [] },
  dlg: null,
  q: [], job: null, batches: {}, from: {},
  opts: { fw: true }, osEdit: null,
  setup: null, rename: null, confirm: null
};
var BK_SIZE = { PS: .62, PS2: 4.6, N64: .03, SNES: .004, GC: 1.4, SAT: .58, WIN: 6.5, LINUX: 2.1 };
var BK_EMU_SIZE = { DuckStation: .05, Dolphin: .04, RetroArch: .41, Ymir: .02, PCSX2: .06, Mednafen: .01, 'GE-Proton10-17': .48 };
var BK_FW = { PS: 3, SAT: 2 };
var FAT32_MAX = 4;

/* Games installed on this computer. A backup copies an eligible one from the
   library instead of downloading it: as plain files when the install is
   exactly the server's files (`file`), otherwise as an uncompressed tar in
   parts of at most 4 GiB (`extracted`, `native`). `have`/`srv` mark a game
   whose server copy is newer: the dialog asks which copy to back up. `why` is
   the reason an installed game downloads instead. Wine prefixes are never
   copied (`wine`). */
G[12].update = true; G[16].installed = true; G[25].installed = true;
var BK_LOCAL = {
  0: { layout: 'extracted', ver: 'v1.1' },
  1: { layout: 'file', ver: 'v1.1', have: 'v1.1', srv: 'v1.2' },
  2: { why: 'Installed outside the library folder' },
  4: { layout: 'file', ver: 'v1.1' },
  6: { layout: 'file', ver: 'v1.1' },
  9: { why: 'Its files weren’t found on this computer' },
  12: { layout: 'extracted', ver: 'v1.1', have: 'v1.1', srv: 'v1.2' },
  16: { layout: 'native', ver: 'v2.4' },
  25: { layout: 'native', ver: 'v1.0.3', wine: true }
};
/* Local copies already on a drive (Details from a drive says so). */
BK.copies = { t7: { 0: { ver: 'v1.1' }, 10: { ver: 'v1.1', outdated: true } } };
function bkLocalOf(g) { return g.installed ? (BK_LOCAL[g.id] || { layout: 'file', ver: 'v1.1' }) : null; }
/* What the local copy writes: plain files are the download's size; a tar of
   an extracted or native tree is bigger than the server's compressed file. */
function bkLocalSize(g) { var l = bkLocalOf(g); return +(bkSize(g) * (l && l.layout !== 'file' ? 1.4 : 1)).toFixed(3); }

/* The computers a pack is for, picked at setup and changed in its card. Each
   emulator on the drive has a build for each; Proton only for Linux. */
var BK_OS = [['linux', 'Linux'], ['windows', 'Windows']];
function bkOsNames(os) { return os.map(function (o) { return o === 'linux' ? 'Linux' : 'Windows'; }).join(' and '); }
function bkEmuOs(n, d) { var os = d ? d.os : ['linux']; return n === 'GE-Proton10-17' ? os.filter(function (o) { return o === 'linux'; }) : os; }
function bkEmuSize(n, d) { return (BK_EMU_SIZE[n] || .05) * Math.max(1, bkEmuOs(n, d).length); }
function bkSize(g) { return +(BK_SIZE[g.p] * (0.7 + ((g.id * 37) % 60) / 100)).toFixed(3); }
function bkFmt(gb) { return gb >= 100 ? Math.round(gb) + ' GB' : gb >= 1 ? gb.toFixed(1) + ' GB' : gb >= 0.001 ? Math.max(1, Math.round(gb * 1000)) + ' MB' : '<1 MB'; }
function bkDrive(id) { return BK.drives.filter(function (d) { return d.id === id; })[0]; }
function bkReady() { return BK.drives.filter(function (d) { return d.connected && d.init; }); }
function bkGridUsed(d) {
  return d.games.reduce(function (s, id) { return s + bkSize(G[id]); }, 0) +
    d.emus.reduce(function (s, n) { return s + bkEmuSize(n, d); }, 0) + d.fw.length * .002;
}
function bkFree(d) { return Math.max(0, d.size - d.other - bkGridUsed(d)); }
function bkPath(d) { return '/run/media/player/' + d.label; }
function bkPlural(n, one, many) { return n + ' ' + (n === 1 ? one : many || one + 's'); }
/* The emulator a backup takes for a system: the default, else the recommended
   one. Windows games take the compat tool. */
function bkEmuFor(p) { return p === 'WIN' ? (S.compatDefault || 'GE-Proton10-17') : p === 'LINUX' ? '' : (SY.pick[p] || SY.rec[p] || ''); }
function bkSysOf(p) { return p === 'WIN' ? 'PC' : p; }

/* ---- Settings › Backup drives ---- */
function bkUsage(d, extra) {
  var used = d.size - d.other - bkFree(d) - (extra || 0);
  var pc = function (v) { return Math.max(0, Math.min(100, v / d.size * 100)).toFixed(2) + '%'; };
  return '<div class="bk-usage" role="img" aria-label="' + bkFmt(bkFree(d)) + ' free of ' + bkFmt(d.size) + '">' +
    '<b class="u-other" style="width:' + pc(d.other) + '"></b><b class="u-grid" style="width:' + pc(used) + '"></b>' +
    (extra ? '<b class="adding' + (extra > bkFree(d) ? ' over' : '') + '" style="width:' + pc(Math.min(extra, bkFree(d) || extra)) + '"></b>' : '') + '</div>';
}
function bkDriveCard(d) {
  var head = '<header class="bk-drive-head"><span class="bk-drive-icon" data-icon="drive" data-size="20"></span><div class="bk-drive-id"><b>' +
    esc(d.init ? d.name : d.label) + '</b><span class="note">' + (d.init && d.name !== d.label ? esc(d.label) + ' · ' : '') + d.fs + ' · ' + bkFmt(d.size) + '</span></div>' +
    (d.connected ? '<span class="pill-s' + (d.init ? ' ok' : '') + '">' + (d.init ? '<span data-icon="check" data-size="12"></span>Ready for backups' : 'Not set up') + '</span>'
                 : '<span class="pill-s">Not connected</span>') + '</header>';
  if (!d.connected) {
    return '<article class="bk-drive away">' + head + '<p class="note">Last seen ' + d.seen + ' · ' + bkPlural(d.games.length, 'game') + ' · ' + bkFmt(bkGridUsed(d)) +
      '. Plug it in to back up to it or install from it.</p><div class="form-actions"><button class="small-btn" data-bk-forget="' + d.id + '">Forget</button>' +
      '<button class="small-btn bk-proto" data-bk-plug="' + d.id + '">Prototype: plug in</button></div></article>';
  }
  var body;
  if (!d.init) {
    if (BK.setup === d.id) {
      body = '<form class="form bk-setup" onsubmit="return false"><label>Pack name<input class="field" id="bk-setup-name" value="' + syEsc(d.label) + '"><span class="help">Shown in Library and on other computers. You can change it later.</span></label>' +
        '<fieldset class="bk-os"><legend>Computers this pack is for</legend>' + BK_OS.map(function (o) { return '<label class="check"><input type="checkbox" data-bk-setup-os="' + o[0] + '"' + (o[0] === 'linux' ? ' checked' : '') + '> ' + o[1] + (o[0] === 'linux' ? ' (this computer)' : '') + '</label>'; }).join('') +
          '<span class="help">GRID adds the emulators each system needs, built for every computer you pick. You can change this later.</span></fieldset>' +
        '<p class="note">GRID makes a <code>gridpack</code> folder at the root of ' + esc(d.label) + '. Nothing else on the drive is changed.</p>' +
        (d.fs === 'FAT32' ? '<p class="bk-warn"><b>FAT32 can’t hold files over 4 GB.</b> Games with a larger file (many PlayStation 2, GameCube and Windows games) can’t go on this drive. Reformat it as exFAT to back up everything.</p>' : '') +
        '<div class="form-actions"><button class="g-btn" type="button" data-bk-setup-go="' + d.id + '">Set up drive</button><button class="g-btn secondary" type="button" data-bk-setup-cancel>Cancel</button></div></form>';
    } else {
      body = '<p class="note">' + bkFmt(bkFree(d)) + ' free. Set it up to back up games to it.</p>' +
        '<div class="form-actions"><button class="g-btn" data-bk-setup="' + d.id + '">Set up for GRID…</button></div>';
    }
  } else {
    var name = BK.rename === d.id
      ? '<form class="bk-rename" onsubmit="return false"><input class="field" id="bk-rename" value="' + syEsc(d.name) + '" aria-label="Pack name"><button class="small-btn primary" type="button" data-bk-rename-go="' + d.id + '">Save</button><button class="small-btn" type="button" data-bk-rename-cancel>Cancel</button></form>' : '';
    var fwNames = d.fw.map(function (k) { return SY.name[k]; });
    body = name + bkUsage(d) +
      '<p class="bk-legend"><span><i class="u-other"></i>Other files ' + bkFmt(d.other) + '</span><span><i class="u-grid"></i>GRID ' + bkFmt(bkGridUsed(d)) + '</span><span><i></i>Free ' + bkFmt(bkFree(d)) + '</span></p>' +
      bkOsRow(d) + '<dl class="kv bk-kv"><div><dt>Games</dt><dd>' + d.games.length + '</dd></div><div><dt>Emulators</dt><dd>' + (d.emus.length ? esc(d.emus.join(', ')) : 'None') + '</dd></div>' +
      '<div><dt>Firmware</dt><dd>' + (fwNames.length ? esc(fwNames.join(', ')) : 'None') + '</dd></div><div><dt>Last backup</dt><dd>' + fmtDate(d.last) + '</dd></div>' +
      '<div><dt>Folder</dt><dd><code>' + esc(bkPath(d)) + '/gridpack</code></dd></div></dl>' +
      (BK.confirm === 'wipe:' + d.id
        ? '<div class="bk-confirm"><p><b>Delete the gridpack folder from ' + esc(d.label) + '?</b> This removes ' + bkPlural(d.games.length, 'game') + ' and ' + bkFmt(bkGridUsed(d)) + ' of GRID files from the drive. Nothing else on the drive is touched, and games installed on this computer stay.</p>' +
          '<div class="form-actions"><button class="g-btn destructive" data-bk-wipe-go="' + d.id + '">Delete GRID files</button><button class="g-btn secondary" data-bk-confirm-cancel>Cancel</button></div></div>'
        : '<div class="form-actions"><button class="g-btn" data-bk-show="' + d.id + '">Show in Library</button><button class="g-btn secondary" data-toast="Opens ' + syEsc(bkPath(d)) + '/gridpack in the file manager">Open folder</button>' +
          '<button class="g-btn secondary" data-bk-rename="' + d.id + '">Rename…</button><button class="g-btn secondary destructive" data-bk-wipe="' + d.id + '">Remove GRID files…</button></div>');
  }
  return '<article class="bk-drive' + (d.init ? '' : ' fresh-drive') + '">' + head + body +
    '<button class="small-btn bk-proto bk-proto-corner" data-bk-plug="' + d.id + '">Prototype: unplug</button></article>';
}
function renderBackupPane(el) {
  var on = BK.drives.filter(function (d) { return d.connected; }), away = BK.drives.filter(function (d) { return !d.connected; });
  el.innerHTML = '<h2>Backup drives</h2>' +
    '<p class="note bk-lede">Copy games from your server to a USB drive, with their art, details, emulators and firmware. Plug the drive into any computer with GRID to install them, even without a network.</p>' +
    '<div class="sub"><h3>Connected</h3>' + (on.length ? on.map(bkDriveCard).join('') :
      '<div class="empty-state"><b>No removable drives</b><span class="note">Plug in a USB flash drive or SSD. It shows up here.</span></div>') + '</div>' +
    (away.length ? '<div class="sub"><h3>Not connected</h3>' + away.map(bkDriveCard).join('') + '</div>' : '') +
    '<div class="sub"><h3>Every backup also copies</h3>' +
    '<label class="check"><input type="checkbox" data-bk-default="fw"' + (BK.opts.fw ? ' checked' : '') + '> The server’s firmware for the systems you back up</label>' +
    '<p class="note">You can change this for each backup. Each game always takes its covers, fanart, screenshots, logo and details, and the emulator its system needs, built for every computer the drive is for.</p></div>' +
    '<details class="bk-tree"><summary>What’s in the gridpack folder</summary><pre>gridpack/\n  gridpack.json        pack name, drive id, format version, game index\n  games/&lt;platform&gt;/     the files the server gives (zip, 7z, iso, folder),\n                       or installed.tar.001 … for a game copied from a computer\n  assets/&lt;platform&gt;/&lt;game&gt;/\n    cover.jpg  fanart.jpg  logo.png  screenshots/\n  emulators/&lt;emulator&gt;/&lt;version&gt;/&lt;os&gt;/\n  firmware/&lt;platform&gt;/</pre></details>';
  gIcons(el);
}

/* "For: Linux and Windows · Change…" in a set-up drive's card. Adding a
   computer queues that build of every emulator already on the drive;
   removing one deletes those builds. */
function bkOsRow(d) {
  if (BK.osEdit !== d.id) return '<p class="bk-for"><span class="note">For</span> <b>' + bkOsNames(d.os) + '</b> <button class="small-btn" data-bk-os-edit="' + d.id + '">Change…</button></p>';
  return '<form class="bk-os bk-os-edit" onsubmit="return false"><span class="note">Computers this pack is for</span>' + BK_OS.map(function (o) {
      return '<label class="check"><input type="checkbox" data-bk-os="' + o[0] + '"' + (d.os.indexOf(o[0]) >= 0 ? ' checked' : '') + '> ' + o[1] + '</label>'; }).join('') +
    '<span class="help">Adding one writes its build of the ' + bkPlural(d.emus.length, 'emulator') + ' on this drive. Removing one deletes those builds; games and firmware stay.</span>' +
    '<div class="form-actions"><button class="small-btn primary" type="button" data-bk-os-go="' + d.id + '">Save</button><button class="small-btn" type="button" data-bk-os-cancel>Cancel</button></div></form>';
}
function bkOsSave(d) {
  var next = Array.prototype.filter.call(document.querySelectorAll('[data-bk-os]'), function (c) { return c.checked; }).map(function (c) { return c.getAttribute('data-bk-os'); });
  if (!next.length) { toast('Pick at least one computer', 'error'); return; }
  var added = next.filter(function (o) { return d.os.indexOf(o) < 0; }), removed = d.os.filter(function (o) { return next.indexOf(o) < 0; });
  var emusFor = function (os) { return d.emus.filter(function (n) { return n !== 'GE-Proton10-17' || os.indexOf('linux') >= 0; }); };
  BK.osEdit = null;
  if (removed.length) {
    var gone = emusFor(removed); d.os = d.os.filter(function (o) { return removed.indexOf(o) < 0; });
    if (removed.indexOf('linux') >= 0) d.emus = d.emus.filter(function (n) { return n !== 'GE-Proton10-17'; });
    toast('Removed the ' + bkOsNames(removed) + ' builds of ' + bkPlural(gone.length, 'emulator') + ' from ' + d.name);
  }
  if (added.length) {
    var todo = emusFor(added), bid = 'o' + Date.now();
    d.os = d.os.concat(added);
    var items = todo.map(function (n) { return { kind: 'emu', t: n, sub: 'Emulator · ' + bkOsNames(added) + ' build', size: (BK_EMU_SIZE[n] || .05) * added.length, drive: d.id, batch: bid, pct: 0, phase: 'queued' }; });
    if (items.length) {
      BK.batches[bid] = { drive: d.id, games: 0, emus: items.length, fw: 0, left: items.length, written: 0 };
      BK.q = BK.q.concat(items);
      toast(bkOsNames(added) + ' builds of ' + bkPlural(items.length, 'emulator') + ' added to the backup queue for ' + d.name);
    }
  }
  renderAll(); bkRun();
}

/* ---- grid multi-select ---- */
function bkGridView() { return S.view === 'library' ? 'library' : 'server'; }
function bkSelecting() { return BK.sel.view === bkGridView() && (S.view === 'library' || S.view === 'server'); }
function bkVisible() {
  var g = S.view === 'library' ? $('lib-grid') : $('srv-grid');
  return Array.prototype.map.call(g.querySelectorAll('[data-game]'), function (c) { return +c.getAttribute('data-game'); });
}
function bkDecorate(gridId) {
  var el = $(gridId); if (!el) return;
  var view = gridId === 'lib-grid' ? 'library' : 'server';
  var on = BK.sel.view === view;
  el.classList.toggle('selecting', on);
  if (!on) return;
  el.querySelectorAll('[data-game]').forEach(function (c) {
    var id = +c.getAttribute('data-game'), picked = BK.sel.ids.indexOf(id) >= 0;
    c.classList.toggle('picked', picked);
    c.setAttribute('aria-selected', picked);
    c.querySelector('.g-cover').insertAdjacentHTML('beforeend', '<span class="bk-check" aria-hidden="true">' + (picked ? gIcon('check', 14) : '') + '</span>');
  });
}
var bkOrigFill = fillGrid;
fillGrid = function (id, list, sp, empty) { bkOrigFill(id, list, sp, empty); bkDecorate(id); };

function bkSelBar() {
  ['srv', 'lib'].forEach(function (k) {
    var body = $(k + '-body'); if (!body) return;
    var bar = $(k + '-selbar');
    if (!bar) { bar = document.createElement('div'); bar.id = k + '-selbar'; bar.className = 'g-panel bk-selbar'; bar.setAttribute('role', 'toolbar'); bar.setAttribute('aria-label', 'Selected games'); body.appendChild(bar); }
    var view = k === 'lib' ? 'library' : 'server';
    var on = BK.sel.view === view && !(view === 'server' && S.platform === 'DISCOVER') && !S.offline;
    bar.hidden = !on;
    var btn = document.querySelector('[data-view=' + view + '] [data-bk-select]');
    if (btn) { btn.classList.toggle('active', on); btn.setAttribute('aria-pressed', on); btn.textContent = on ? 'Done' : 'Select'; btn.hidden = S.offline && view === 'server'; }
    if (!on) return;
    var vis = bkVisible(), ids = BK.sel.ids, size = ids.reduce(function (s, id) { return s + bkSize(G[id]); }, 0);
    var all = vis.length && vis.every(function (id) { return ids.indexOf(id) >= 0; });
    var drv = view === 'library' && bkLibDrive();
    var acts = drv
      ? '<button class="g-btn" data-bk-sel-install' + (ids.length ? '' : ' disabled') + '>Install ' + (ids.length ? bkPlural(ids.length, 'game') : '') + '</button>' +
        (BK.confirm === 'selrm' ? '<button class="g-btn destructive" data-bk-sel-rm-go>Remove ' + ids.length + ' from drive</button>' : '<button class="g-btn secondary destructive" data-bk-sel-rm' + (ids.length ? '' : ' disabled') + '>Remove from drive…</button>')
      : '<button class="g-btn" data-bk-open' + (ids.length ? '' : ' disabled') + '>Back up to drive…</button>';
    bar.innerHTML = '<span class="bk-sel-count"><b>' + (ids.length ? ids.length + ' selected' : 'Select games') + '</b>' + (ids.length ? '<span class="note">' + bkFmt(size) + '</span>' : '<span class="note">Click covers to pick them · Shift-click picks a range</span>') + '</span>' +
      '<button class="small-btn" data-bk-all' + (vis.length ? '' : ' disabled') + '>' + (all ? 'All ' + vis.length + ' selected' : 'Select all ' + vis.length) + '</button>' +
      (ids.length ? '<button class="small-btn" data-bk-clear>Clear</button>' : '') + '<span class="bk-sel-gap"></span>' + acts;
  });
}

/* ---- Library › Backup drives ---- */
function bkLibDrive() { var f = S.libFilter; return f && f.indexOf('drive:') === 0 ? bkDrive(f.slice(6)) : null; }
var bkOrigLib = renderLibrary;
renderLibrary = function () {
  var d = bkLibDrive();
  if (d && !(d.connected && d.init)) { S.libFilter = 'all'; d = null; }
  var keep = S.libFilter; if (d) S.libFilter = 'all';
  bkOrigLib();
  S.libFilter = keep;
  var ready = bkReady(), rail = $('lib-rail');
  if (ready.length) {
    if (d) rail.querySelectorAll('.g-rail-item').forEach(function (b) { b.classList.remove('active'); b.removeAttribute('aria-current'); });
    rail.insertAdjacentHTML('beforeend', '<span class="g-rail-heading">BACKUP DRIVES</span>' + ready.map(function (x) {
      var sel = d && d.id === x.id;
      return '<button class="g-rail-item bk-rail' + (sel ? ' active' : '') + '" data-key="drive:' + x.id + '"' + (sel ? ' aria-current="page"' : '') + ' title="' + syEsc(x.label) + '"><span class="bk-rail-icon" data-icon="drive" data-size="14"></span><span class="g-rail-label">' + esc(x.name) + '</span><span class="g-rail-count">' + x.games.length + '</span></button>';
    }).join(''));
    gIcons(rail);
  }
  var chips = $('lib-chips');
  if (!d) { if (chips) chips.hidden = true; bkPlatAct(); bkSelBar(); return; }
  var list = d.games.map(function (id) { return G[id]; }).filter(function (g) { return match(g, S.q.library); });
  $('lib-title').textContent = d.name;
  $('lib-count').textContent = bkPlural(d.games.length, 'game') + ' · ' + bkFmt(bkGridUsed(d)) + ' · on ' + d.label;
  chips.hidden = false;
  chips.innerHTML = '<span class="g-chip-outline">Emulators: ' + (d.emus.length ? esc(d.emus.join(', ')) : 'none') + '</span><span class="g-chip-outline">Firmware: ' +
    (d.fw.length ? esc(d.fw.map(function (k) { return SY.name[k]; }).join(', ')) : 'none') + '</span>';
  fillGrid('lib-grid', list, true, S.q.library.trim() ? 'No games on this drive match “' + S.q.library.trim() + '”' : 'Nothing on this drive yet. Back up games from Server or Library.');
  bkPlatAct(); bkSelBar();
};

/* "Back up all N…" for a whole platform: Server platform headers and
   Library platform rows. */
function bkPlatAct() {
  [['srv', 'server'], ['lib', 'library']].forEach(function (p) {
    var head = document.querySelector('#' + p[0] + '-head .g-platform-header'); if (!head) return;
    var b = head.querySelector('[data-bk-plat]');
    var plat = p[1] === 'server' ? S.platform : S.libFilter;
    var ok = !S.offline && PLAT[plat] && plat !== 'DISCOVER';
    var n = !ok ? 0 : G.filter(function (g) { return g.p === plat && (p[1] === 'server' || g.installed); }).length;
    if (!b) { head.insertAdjacentHTML('beforeend', '<button class="small-btn bk-plat" data-bk-plat="' + p[1] + '"></button>'); b = head.querySelector('[data-bk-plat]'); }
    b.hidden = !n; b.textContent = 'Back up all ' + n + '…';
  });
}
var bkOrigSrv = renderServer;
renderServer = function () {
  var off = !!S.offline, srvView = document.querySelector('[data-view=server]');
  $('srv-rail').style.display = off ? 'none' : ''; $('srv-body').style.display = off ? 'none' : ''; $('bk-srv-off').hidden = !off;
  if (!off) bkOrigSrv();
  bkPlatAct(); bkSelBar();
};

/* ---- the backup dialog ---- */
function bkOpen(ids, title) {
  if (S.offline) { toast('Connect to a server to back up games', 'error'); return; }
  var ready = bkReady();
  var pick = BK.dlg && BK.dlg.drive && bkDrive(BK.dlg.drive) && bkDrive(BK.dlg.drive).connected ? BK.dlg.drive : (ready[0] ? ready[0].id : null);
  BK.dlg = { ids: ids.slice(), title: title, drive: pick, emus: true, fw: BK.opts.fw, local: true, choices: {}, each: false };
  $('bkscrim').hidden = false; bkRenderDlg();
}
/* Where each game comes from. A game is copied from this computer when the
   checkbox is on, it is an eligible install, and it is not out of date or the
   user chose this computer's copy. An out-of-date game with no choice yet
   counts as a download and holds the Back up button. */
function bkPlan(dl) {
  var d = bkDrive(dl.drive), games = dl.ids.map(function (id) { return G[id]; });
  var useLocal = dl.local !== false, choices = dl.choices || {};
  var have = d ? games.filter(function (g) { return d.games.indexOf(g.id) >= 0; }) : [];
  var srcOf = function (g) {
    var l = bkLocalOf(g);
    if (!useLocal || !l || l.why) return 'download';
    if (l.srv) return choices[g.id] === 'local' ? 'local' : 'download';
    return 'local';
  };
  var big = d && d.fs === 'FAT32' ? games.filter(function (g) { return bkSize(g) > FAT32_MAX && have.indexOf(g) < 0 && srcOf(g) === 'download'; }) : [];
  var todo = games.filter(function (g) { return have.indexOf(g) < 0 && big.indexOf(g) < 0; });
  var emus = [], fw = [];
  todo.forEach(function (g) {
    var e = bkEmuFor(g.p); if (e && bkEmuOs(e, d).length && emus.indexOf(e) < 0 && (!d || d.emus.indexOf(e) < 0)) emus.push(e);
    if (BK_FW[g.p] && fw.indexOf(g.p) < 0 && (!d || d.fw.indexOf(g.p) < 0)) fw.push(g.p);
  });
  var sources = todo.map(function (g) {
    var l = bkLocalOf(g), s = srcOf(g);
    return { g: g, source: s, note: s === 'download' && l && l.why ? l.why : '', outdated: !!(l && !l.why && l.srv) };
  });
  var outdated = useLocal ? sources.filter(function (r) { return r.outdated; }).map(function (r) { var l = bkLocalOf(r.g); return { g: r.g, have: l.have, srv: l.srv, choice: choices[r.g.id] || null }; }) : [];
  var local = sources.filter(function (r) { return r.source === 'local'; });
  var localBytes = local.reduce(function (s, r) { return s + bkLocalSize(r.g); }, 0);
  var gameDl = sources.filter(function (r) { return r.source === 'download'; }).reduce(function (s, r) { return s + bkSize(r.g); }, 0);
  var extraDl = emus.reduce(function (s, n) { return s + bkEmuSize(n, d); }, 0) + (dl.fw ? fw.length * .002 : 0);
  return { d: d, todo: todo, have: have, big: big, emus: emus, fw: fw, size: localBytes + gameDl + extraDl,
    localAvailable: games.some(function (g) { return g.installed; }), useLocal: useLocal, sources: sources, outdated: outdated,
    needsChoice: outdated.filter(function (o) { return !o.choice; }).length, local: local,
    localBytes: localBytes, downloadBytes: gameDl + extraDl,
    wine: local.some(function (r) { return bkLocalOf(r.g).wine; }) };
}
/* The Source section: the checkbox, where each game comes from, and the
   choice for an out-of-date game. Shown only when a requested game is
   installed on this computer. */
function bkSourceSection(pl, dl) {
  if (!pl.localAvailable) return '';
  var nAsk = pl.needsChoice, nDl = pl.sources.length - pl.local.length - nAsk;
  var line = !pl.useLocal ? 'Every game downloads from romm.home.lan'
    : [pl.local.length ? bkPlural(pl.local.length, 'game') + ' from this computer (' + bkFmt(pl.localBytes) + ')' : nAsk ? '' : 'No game here can be copied',
       nDl ? nDl + ' downloaded' : '', nAsk ? nAsk + ' to choose' : ''].filter(Boolean).join(' · ');
  var h = '<label class="check bk-also"><input type="checkbox" data-bk-opt="local"' + (pl.useLocal ? ' checked' : '') + '><span><b>Copy installed games from this computer</b><span class="note">' + line + '</span></span></label>';
  if (!pl.useLocal || !pl.sources.length) return '<div class="sub"><h4>Source</h4>' + h + '</div>';
  if (pl.wine) h += '<p class="note bk-src-wine">Windows games are copied without their Wine prefix, so saves kept in the prefix stay on this computer.</p>';
  var rows = '<ul class="bk-src-list">' + pl.sources.map(function (r) {
    var chose = (dl.choices || {})[r.g.id];
    var why = r.note ? r.note
      : r.outdated ? (chose === 'local' ? 'an older copy than the server’s' : chose ? 'the newer version' : 'the server has newer files') : '';
    var tag = r.source === 'local' ? '<span class="bk-src-tag local"><span data-icon="drive" data-size="12"></span>From this computer</span>'
      : r.outdated && !chose ? '<span class="bk-src-tag ask">Choose below</span>'
      : '<span class="bk-src-tag"><span data-icon="download" data-size="12"></span>Download</span>';
    return '<li><span class="bk-src-name"><b>' + esc(r.g.t) + '</b><span class="note">' + esc(PLAT[r.g.p]) + (why ? ' · ' + esc(why) : '') + '</span></span>' + tag + '</li>';
  }).join('') + '</ul>';
  h += pl.sources.length > 6
    ? '<details class="bk-src-each"' + (dl.each ? ' open' : '') + ' data-bk-each><summary>Show each game <span class="note">' + pl.sources.length + '</span></summary>' + rows + '</details>'
    : rows;
  if (pl.outdated.length) {
    var many = pl.outdated.length > 1;
    h += '<div class="bk-outdated" role="group" aria-label="Out-of-date games">' +
      '<p class="bk-outdated-head"><b>' + (many ? 'The server has newer files for ' + pl.outdated.length + ' games' : 'The server has newer files') + '</b>' +
      '<span class="note">Back up the copy installed here, or download the newer version.</span></p>' +
      (many ? '<div class="bk-outdated-bulk"><button class="small-btn" type="button" data-bk-bulk="local">Use this computer’s copies</button><button class="small-btn" type="button" data-bk-bulk="download">Download all</button></div>' : '') +
      pl.outdated.map(function (o) {
        var nm = 'bk-choice-' + o.g.id;
        return '<fieldset class="bk-choice"><legend><b>' + esc(o.g.t) + '</b>: the server has newer files (' + o.srv + '; this computer has ' + o.have + ')</legend>' +
          '<label class="check"><input type="radio" name="' + nm + '" value="local" data-bk-choice="' + o.g.id + '"' + (o.choice === 'local' ? ' checked' : '') + '> Use this computer’s copy</label>' +
          '<label class="check"><input type="radio" name="' + nm + '" value="download" data-bk-choice="' + o.g.id + '"' + (o.choice === 'download' ? ' checked' : '') + '> Download ' + o.srv + ' from the server</label></fieldset>';
      }).join('') + '</div>';
  }
  return '<div class="sub bk-src"><h4>Source</h4>' + h + '</div>';
}
function bkRenderDlg() {
  var dl = BK.dlg, el = $('bkdlg'); if (!dl) return;
  var ready = bkReady(), pl = bkPlan(dl), fresh = BK.drives.filter(function (d) { return d.connected && !d.init; });
  var drives;
  if (!ready.length) {
    drives = '<div class="empty-state"><b>No backup drive connected</b><span class="note">Plug in a drive you set up for GRID' +
      (fresh.length ? ', or set up ' + esc(fresh.map(function (d) { return d.label; }).join(' or ')) + ' (connected now)' : '') +
      '. Set up a drive once in Settings › Backup drives.</span><div class="form-actions"><button class="g-btn secondary" data-bk-goset>Set up a drive…</button></div></div>';
  } else {
    drives = '<div class="bk-opts" role="radiogroup" aria-label="Drive">' + ready.map(function (d) {
      var p = bkPlan({ ids: dl.ids, drive: d.id, emus: dl.emus, fw: dl.fw, local: dl.local, choices: dl.choices }), over = p.size > bkFree(d);
      return '<label class="bk-opt' + (dl.drive === d.id ? ' on' : '') + (over ? ' over' : '') + '"><input type="radio" name="bk-drive" value="' + d.id + '"' + (dl.drive === d.id ? ' checked' : '') + '>' +
        '<span class="bk-opt-main"><b>' + esc(d.name) + '</b><span class="note">' + esc(d.label) + ' · ' + d.fs + '</span></span>' +
        '<span class="bk-opt-free' + (over ? ' warn' : '') + '">' + (over ? 'Needs ' + bkFmt(p.size) + ' · ' + bkFmt(bkFree(d)) + ' free' : bkFmt(bkFree(d)) + ' free') + '</span>' +
        bkUsage(d, dl.drive === d.id ? p.size : 0) + '</label>';
    }).join('') + '</div>';
  }
  var notes = [];
  if (pl.d && pl.have.length) notes.push(bkPlural(pl.have.length, 'game is', 'games are').replace(/^(\d+) /, '$1 ') + ' already on ' + esc(pl.d.name) + ' and will be skipped' + (pl.have.length <= 3 ? ': ' + esc(pl.have.map(function (g) { return g.t; }).join(', ')) : '') + '.');
  if (pl.big.length) notes.push('<span class="bk-warn-line">' + bkPlural(pl.big.length, 'game has', 'games have') + ' a file over 4 GB, which ' + esc(pl.d.label) + ' (FAT32) can’t hold: ' + esc(pl.big.map(function (g) { return g.t; }).join(', ')) + '.</span>');
  var over = pl.d && pl.size > bkFree(pl.d), n = pl.todo.length, ask = pl.needsChoice;
  var lede = pl.useLocal && pl.local.length
    ? 'GRID copies games installed on this computer and downloads the rest from romm.home.lan, then writes each one, with its art and details, to the drive’s gridpack folder.'
    : 'GRID downloads each game from romm.home.lan and writes it, with its art and details, to the drive’s gridpack folder.';
  var foot = !n ? 'Nothing new to write' : bkFmt(pl.size) + ' to write' + (pl.local.length && pl.downloadBytes ? ' · ' + bkFmt(pl.downloadBytes) + ' to download' : '');
  var goLabel = ask ? 'Choose for ' + bkPlural(ask, 'game') : over ? 'Not enough space' : n ? 'Back up ' + bkPlural(n, 'game') : 'Nothing to back up';
  var emuLine = pl.emus.length ? esc(pl.emus.join(', ')) + (pl.d ? ' for ' + bkOsNames(pl.d.os) : '') + ' · ' + bkFmt(pl.emus.reduce(function (s, x) { return s + bkEmuSize(x, pl.d); }, 0)) : (pl.d ? 'Already on the drive' : 'None needed');
  var fwLine = pl.fw.length ? esc(pl.fw.map(function (k) { return SY.name[k] + ' (' + BK_FW[k] + ' files)'; }).join(', ')) : (pl.d ? 'Already on the drive, or none on the server' : 'None on the server');
  el.innerHTML = '<button class="g-icon-btn g-dialog-close" aria-label="Close" data-bk-dlg="cancel"><span data-icon="close" data-size="20"></span></button>' +
    '<h3 id="bk-dlg-title">' + esc(dl.title || ('Back up ' + bkPlural(dl.ids.length, 'game'))) + '</h3>' +
    '<p class="note">' + lede + '</p>' +
    '<div class="sub"><h4>Drive</h4>' + drives + '</div>' +
    (ready.length ? bkSourceSection(pl, dl) : '') +
    (ready.length ? '<div class="sub"><h4>Also copy</h4>' +
      '<div class="bk-also bk-auto"><span class="bk-auto-mark" data-icon="check" data-size="13"></span><span><b>Emulators</b><span class="note">' + emuLine + '</span></span></div>' +
      '<label class="check bk-also"><input type="checkbox" data-bk-opt="fw"' + (dl.fw ? ' checked' : '') + (pl.fw.length ? '' : ' disabled') + '><span><b>Firmware</b><span class="note">' + fwLine + '</span></span></label>' +
      '<p class="note">Covers, fanart, screenshots, logos and game details always go with each game, and so do the emulators.</p></div>' : '') +
    (notes.length ? '<div class="bk-notes">' + notes.map(function (t) { return '<p class="note">' + t + '</p>'; }).join('') + '</div>' : '') +
    '<div class="form-actions bk-foot">' + (ready.length ? '<span class="note">' + foot + '</span>' : '<span></span>') +
    '<button class="g-btn secondary" data-bk-dlg="cancel">Cancel</button>' +
    (ready.length ? '<button class="g-btn" data-bk-dlg="go"' + (n && !over && !ask ? '' : ' disabled') + '>' + goLabel + '</button>' : '') + '</div>';
  gIcons(el);
}
function bkCloseDlg() { BK.dlg = null; $('bkscrim').hidden = true; }

/* ---- the backup queue: one Downloads row per item, kind "Backup" ---- */
function bkGo() {
  var dl = BK.dlg, pl = bkPlan(dl), d = pl.d, bid = 'b' + Date.now();
  var items = pl.sources.map(function (r) { var g = r.g, loc = r.source === 'local', l = bkLocalOf(g);
    return { kind: 'game', id: g.id, t: g.t, sub: PLAT[g.p], size: loc ? bkLocalSize(g) : bkSize(g), local: loc, ver: loc ? l.ver : '', outdated: loc && !!l.srv }; });
  pl.emus.forEach(function (n) { items.push({ kind: 'emu', t: n, sub: (n === 'GE-Proton10-17' ? 'Compat tool' : 'Emulator') + ' · ' + bkOsNames(bkEmuOs(n, d)), size: bkEmuSize(n, d) }); });
  if (dl.fw) pl.fw.forEach(function (k) { items.push({ kind: 'fw', p: k, t: SY.name[k] + ' firmware', sub: BK_FW[k] + ' files', size: .002 }); });
  items.forEach(function (it) { it.drive = d.id; it.batch = bid; it.pct = 0; it.phase = 'queued'; });
  BK.batches[bid] = { drive: d.id, games: pl.todo.length, emus: pl.emus.length, fw: dl.fw ? pl.fw.length : 0, left: items.length, written: 0 };
  BK.q = BK.q.concat(items);
  bkCloseDlg();
  BK.sel = { view: null, ids: [] };
  toast((pl.todo.length === 1 ? pl.todo[0].t : bkPlural(pl.todo.length, 'game')) + ' added to the backup queue for ' + d.name);
  renderAll(); bkRun();
}
function bkRun() {
  if (BK.job || !BK.q.length) return;
  var it = BK.job = BK.q.shift();
  it.phase = it.local ? 'details' : 'download'; it.pct = 0;
  var tick = setInterval(function () {
    if (BK.job !== it) { clearInterval(tick); return; }
    var d = bkDrive(it.drive);
    if (!d.connected) { clearInterval(tick); bkFail(it.drive); return; }
    it.pct = Math.min(100, it.pct + (it.phase === 'details' ? 50 : it.kind === 'game' ? 7 + Math.random() * 7 : 30));
    if (it.pct >= 100 && (it.phase === 'download' || it.phase === 'details')) { it.phase = 'write'; it.pct = 0; }
    else if (it.pct >= 100 && it.phase === 'write') { clearInterval(tick); bkFinish(it); return; }
    renderDownloads();
  }, 260);
  renderDownloads();
}
function bkFinish(it) {
  var d = bkDrive(it.drive), b = BK.batches[it.batch];
  if (it.kind === 'game' && d.games.indexOf(it.id) < 0) d.games.push(it.id);
  if (it.kind === 'game') { BK.copies[d.id] = BK.copies[d.id] || {}; if (it.local) BK.copies[d.id][it.id] = { ver: it.ver, outdated: it.outdated }; else delete BK.copies[d.id][it.id]; }
  if (it.kind === 'emu' && d.emus.indexOf(it.t) < 0) d.emus.push(it.t);
  if (it.kind === 'fw' && d.fw.indexOf(it.p) < 0) d.fw.push(it.p);
  d.last = TODAY;
  addDone(it.t, 'Backup', it.sub + (it.local ? ' · Copied from this computer' : '') + ' · Written to ' + d.name);
  BK.job = null; b.left -= 1; b.written += 1;
  if (!b.left) {
    var extra = [b.emus ? bkPlural(b.emus, 'emulator') : '', b.fw ? 'firmware' : ''].filter(Boolean).join(' and ');
    toast((b.games ? bkPlural(b.games, 'game') : 'Files') + (extra ? ' and ' + extra : '') + ' backed up to ' + d.name);
    delete BK.batches[it.batch];
  }
  renderAll(); bkRun();
}
/* The drive went away mid-backup: the item being written fails, the rest of
   that drive's queue is cancelled, one error card says so. */
function bkFail(driveId) {
  var d = bkDrive(driveId), it = BK.job, left = BK.q.filter(function (x) { return x.drive === driveId; });
  var b = BK.batches[it.batch], wrote = b ? b.written : 0;
  addDone(it.t, 'Backup', it.sub + ' · Failed — ' + d.label + ' was removed');
  if (left.length) addDone(left.length === 1 ? left[0].t : bkPlural(left.length, 'more item'), 'Backup', 'Cancelled — ' + d.label + ' was removed · back up again to finish');
  BK.q = BK.q.filter(function (x) { return x.drive !== driveId; });
  BK.job = null; delete BK.batches[it.batch];
  toast('Backup to ' + d.name + ' stopped: ' + d.label + ' was removed. ' + wrote + ' of ' + (wrote + 1 + left.length) + ' items were written; the half-written one was cleaned up when the drive came back.', 'error');
  renderAll(); bkRun();
}
var bkOrigDl = renderDownloads;
renderDownloads = function () {
  bkOrigDl();
  var j = BK.job, from = S.dl && BK.from[S.dl.id] && bkDrive(BK.from[S.dl.id]);
  if (from) {
    $('strip-line').innerHTML = $('strip-line').innerHTML.replace(/ · [\d.]+ MB\/s/, ' · Copying from ' + esc(from.label));
    var s = $('dl-active').querySelector('.s'); if (s) s.textContent = s.textContent.replace(/Downloading (\d+)%/, 'Copying from ' + from.label + ' $1%');
  }
  if (!j && !BK.q.length) return;
  var active = (S.dl ? 1 : 0) + (j ? 1 : 0);
  $('dl-active-n').textContent = active;
  $('dl-queued-n').textContent = DLQ.length + BK.q.length;
  var row = function (it, live) {
    var d = bkDrive(it.drive), pct = Math.round(it.pct);
    var st = !live ? 'Waiting · to ' + esc(d.name) : it.phase === 'details' ? 'Getting details from server' : it.phase === 'download' ? 'Downloading from server ' + pct + '%'
      : it.local ? 'Copying from this computer to ' + esc(d.label) + ' ' + pct + '%' : 'Writing to ' + esc(d.label) + ' ' + pct + '%';
    return '<div class="dl"' + (live ? '' : ' style="margin-top:8px"') + '><div><span class="t">' + esc(it.t) + '</span><span class="kind">Backup</span><div class="s">' + esc(it.sub) + ' · ' + st + '</div>' +
      (live ? '<div class="bar"><b style="width:' + (it.phase === 'details' ? 0 : pct) + '%"></b></div>' : '') + '</div>' +
      (live ? '<svg width="120" height="38" viewBox="0 0 120 18" preserveAspectRatio="none"><polyline fill="none" stroke="' + (it.phase === 'write' ? 'var(--graph-disk)' : 'var(--primary)') + '" stroke-width="1.2" points="0,12 20,8 40,10 60,6 80,9 100,7 120,8"/></svg>' : '<span></span>') +
      '<button class="small-btn" data-bk-cancel="' + (live ? 'job' : BK.q.indexOf(it)) + '">Cancel</button></div>';
  };
  if (j) {
    if (!S.dl) $('dl-active').innerHTML = '';
    $('dl-active').insertAdjacentHTML(S.dl ? 'beforeend' : 'afterbegin', (S.dl ? '<div style="height:8px"></div>' : '') + row(j, true));
  }
  if (BK.q.length) {
    if (!DLQ.length) $('dl-queued').innerHTML = '';
    $('dl-queued').insertAdjacentHTML('beforeend', BK.q.map(function (it) { return row(it, false); }).join(''));
  }
  if (!S.dl && j) {
    $('strip').hidden = false;
    var d = bkDrive(j.drive);
    $('strip-line').innerHTML = 'Backing up ' + esc(j.t) + ' to ' + esc(d.name) + ' · ' + Math.round(j.pct) + '% · ' + (j.phase === 'details' ? 'getting details' : j.phase === 'download' ? 'downloading' : j.local ? 'copying from this computer' : 'writing');
  }
};

/* ---- install from a drive ---- */
function bkInstallFrom(g, d) {
  if (g.installed) return;
  BK.from[g.id] = d.id;
  var sys = bkSysOf(g.p), e = bkEmuFor(g.p);
  if (g.p !== 'LINUX' && g.p !== 'WIN' && !SY.pick[g.p] && d.emus.indexOf(e) >= 0) {
    toast(e + ' from ' + d.name + ' added to download queue');
    addDone(e, 'Emulator', 'Copied from ' + d.label + ' · Finished');
    if (SY.installed.indexOf(e) < 0) SY.installed.push(e);
    SY.pick[g.p] = e; EMU[g.p] = e;
  }
  if (g.p === 'WIN' && !SY.winSet && d.emus.indexOf(e) >= 0) { SY.winSet = true; toast(e + ' from ' + d.name + ' added to download queue'); }
  if (BK_FW[g.p] && d.fw.indexOf(g.p) >= 0 && SY.fw[g.p] !== 'installed') { SY.fw[g.p] = 'installed'; toast(SY.name[g.p] + ' firmware installed from ' + d.name); }
  startInstall(g);
}

/* ---- offline: GRID without a server ---- */
function bkSession() {
  var dot = document.querySelector('.g-session .g-status-dot'), chip = $('chip'), menu = document.querySelector('#menu .g-menu');
  if (!dot || !chip) return;
  dot.classList.toggle('online', !S.offline); dot.classList.toggle('bk-offline-dot', !!S.offline);
  chip.textContent = S.offline ? 'Offline · no server' : 'player@romm.home.lan';
  menu.innerHTML = S.offline
    ? '<button role="menuitem" data-bk-connect>Connect to a server…</button><span class="g-menu-host">Library and backup drives work offline</span>'
    : '<button role="menuitem">Disconnect</button><button role="menuitem">Open RomM in browser</button><span class="g-menu-host">romm.home.lan</span>';
}
var bkOrigFresh = freshShow;
freshShow = function (phase) {
  bkOrigFresh(phase);
  if (phase !== 'connect') return;
  var c = document.querySelector('[data-fr-connect]'); if (!c) return;
  c.insertAdjacentHTML('afterend', '<div class="bk-or" aria-hidden="true"><span>or</span></div><button class="g-btn secondary" type="button" data-bk-offline' + (FR.form.lib.trim() ? '' : ' disabled') + '>Use without a server</button>' +
    '<p class="help center">Install games from a backup drive and play what’s installed. Connect to a server any time from the top bar.</p>');
};

/* ---- Details: Back up to drive, and drive context ---- */
var bkOrigDet = renderDetails;
renderDetails = function () {
  bkOrigDet();
  var g = S.detail; if (!g) return;
  var left = $('d-left'), d = BK.detFrom && bkDrive(BK.detFrom);
  if (d && d.connected) {
    var inst = left.querySelector('[data-d=install]');
    if (inst) { inst.textContent = S.dl && S.dl.id === g.id ? 'Installing…' : 'Install from drive'; inst.setAttribute('data-bk-dinst', d.id); inst.removeAttribute('data-d'); }
    left.querySelector('.g-meta-line').insertAdjacentHTML('beforebegin', BK.confirm === 'drm' ? '<button class="g-btn destructive" data-bk-drm-go>Remove from ' + esc(d.name) + '</button>' : '<button class="g-btn secondary destructive" data-bk-drm>Remove from drive…</button>');
  } else if (!g.missing) {
    var on = bkReady().filter(function (x) { return x.games.indexOf(g.id) >= 0; });
    var btn = '<button class="g-btn secondary" data-bk-one' + (S.offline ? ' disabled title="Connect to a server to back up games"' : '') + '>Back up to drive…</button>';
    left.querySelector('.g-meta-line').insertAdjacentHTML('beforebegin', btn);
    if (on.length) left.insertAdjacentHTML('beforeend', '<p class="g-meta-line bk-on"><span data-icon="drive" data-size="12"></span>On ' + esc(on.map(function (x) { return x.name; }).join(', ')) + '</p>');
  }
  if (d) $('d-head').querySelector('.g-header-line').insertAdjacentHTML('beforeend', ' · <span class="bk-src">From ' + esc(d.name) + '</span>');
  var cp = d && BK.copies[d.id] && BK.copies[d.id][g.id];
  if (cp) {
    var wine = g.p === 'WIN' ? '<p class="g-meta-line">Its Wine prefix wasn’t copied, so saves kept there aren’t on the drive</p>' : '';
    left.querySelector('.g-meta-line').insertAdjacentHTML('beforebegin', '<p class="g-meta-line bk-copy">A copy of an installed game' + (cp.ver ? ' · ' + esc(cp.ver) : '') + (cp.outdated ? ' (the server had newer files)' : '') + '</p>' + wine);
  }
  gIcons(left);
};

/* ---- events ---- */
document.addEventListener('click', function (e) {
  var t = e.target, b;
  /* Select mode: a click on a card picks it instead of opening Details. */
  var card = t.closest('[data-game]');
  if (card && bkSelecting() && card.closest('.grid.selecting')) {
    e.stopPropagation(); e.preventDefault();
    var id = +card.getAttribute('data-game'), ids = BK.sel.ids, i = ids.indexOf(id);
    if (e.shiftKey && BK.sel.last !== undefined) {
      var vis = bkVisible(), a = vis.indexOf(BK.sel.last), z = vis.indexOf(id);
      vis.slice(Math.min(a, z), Math.max(a, z) + 1).forEach(function (x) { if (ids.indexOf(x) < 0) ids.push(x); });
    } else if (i >= 0) ids.splice(i, 1); else ids.push(id);
    BK.sel.last = id; BK.confirm = null;
    S.view === 'library' ? renderLibrary() : renderServer();
    return;
  }
  /* Drive grid: Install copies from the drive. */
  if (card && S.view === 'library' && bkLibDrive()) {
    var g0 = G[+card.getAttribute('data-game')], act = t.closest('[data-act]');
    if (act && act.getAttribute('data-act') === 'primary' && !g0.installed) { e.stopPropagation(); bkInstallFrom(g0, bkLibDrive()); return; }
    if (!act || act.getAttribute('data-act') !== 'primary') BK.detFrom = bkLibDrive().id;
  } else if (card) BK.detFrom = null;
}, true);

document.addEventListener('click', function (e) {
  var t = e.target, b;
  if (t.closest('[data-bk-select]')) { var v = bkGridView(); BK.sel = BK.sel.view === v ? { view: null, ids: [] } : { view: v, ids: [] }; BK.confirm = null; v === 'library' ? renderLibrary() : renderServer(); return; }
  if (t.closest('[data-bk-all]')) { var vis = bkVisible(), all = vis.every(function (x) { return BK.sel.ids.indexOf(x) >= 0; }); BK.sel.ids = all ? [] : vis; S.view === 'library' ? renderLibrary() : renderServer(); return; }
  if (t.closest('[data-bk-clear]')) { BK.sel.ids = []; BK.confirm = null; S.view === 'library' ? renderLibrary() : renderServer(); return; }
  if (t.closest('[data-bk-open]')) { bkOpen(BK.sel.ids); return; }
  if ((b = t.closest('[data-bk-plat]'))) {
    var view = b.getAttribute('data-bk-plat'), plat = view === 'server' ? S.platform : S.libFilter;
    var ids = G.filter(function (g) { return g.p === plat && (view === 'server' || g.installed); }).map(function (g) { return g.id; });
    bkOpen(ids, 'Back up ' + PLAT[plat] + ' · ' + bkPlural(ids.length, 'game')); return;
  }
  if (t.closest('[data-bk-one]')) { bkOpen([S.detail.id], 'Back up ' + S.detail.t); return; }
  if ((b = t.closest('[data-bk-bulk]'))) { var v2 = b.getAttribute('data-bk-bulk'); bkPlan(BK.dlg).outdated.forEach(function (o) { BK.dlg.choices[o.g.id] = v2; }); bkRenderDlg(); return; }
  if ((b = t.closest('[data-bk-dlg]'))) { if (b.getAttribute('data-bk-dlg') === 'go') bkGo(); else bkCloseDlg(); return; }
  if (t === $('bkscrim')) { bkCloseDlg(); return; }
  if (t.closest('[data-bk-goset]')) { bkCloseDlg(); closeDetails(); S.view = 'settings'; S.setPane = 'backup'; renderAll(); return; }
  if ((b = t.closest('[data-bk-cancel]'))) {
    var k = b.getAttribute('data-bk-cancel');
    var it = k === 'job' ? BK.job : BK.q[+k];
    if (k === 'job') BK.job = null; else BK.q.splice(+k, 1);
    var bt = BK.batches[it.batch]; if (bt) { bt.left -= 1; if (!bt.left) delete BK.batches[it.batch]; }
    addDone(it.t, 'Backup', it.sub + ' · Cancelled');
    renderAll(); bkRun(); return;
  }
  /* Settings › Backup drives */
  if ((b = t.closest('[data-bk-setup]'))) { BK.setup = b.getAttribute('data-bk-setup'); renderSettings(); var n = $('bk-setup-name'); if (n) n.select(); return; }
  if (t.closest('[data-bk-setup-cancel]')) { BK.setup = null; renderSettings(); return; }
  if ((b = t.closest('[data-bk-setup-go]'))) {
    var d = bkDrive(b.getAttribute('data-bk-setup-go')); d.name = ($('bk-setup-name').value || d.label).trim(); d.init = true;
    d.os = Array.prototype.filter.call(document.querySelectorAll('[data-bk-setup-os]'), function (c) { return c.checked; }).map(function (c) { return c.getAttribute('data-bk-setup-os'); }); if (!d.os.length) d.os = ['linux']; d.created = d.last = TODAY; BK.setup = null;
    toast(d.label + ' is ready for backups as “' + d.name + '”'); renderAll(); return;
  }
  if ((b = t.closest('[data-bk-os-edit]'))) { BK.osEdit = b.getAttribute('data-bk-os-edit'); renderSettings(); return; }
  if (t.closest('[data-bk-os-cancel]')) { BK.osEdit = null; renderSettings(); return; }
  if ((b = t.closest('[data-bk-os-go]'))) { bkOsSave(bkDrive(b.getAttribute('data-bk-os-go'))); return; }
  if ((b = t.closest('[data-bk-rename]'))) { BK.rename = b.getAttribute('data-bk-rename'); renderSettings(); return; }
  if (t.closest('[data-bk-rename-cancel]')) { BK.rename = null; renderSettings(); return; }
  if ((b = t.closest('[data-bk-rename-go]'))) { var d2 = bkDrive(b.getAttribute('data-bk-rename-go')); d2.name = ($('bk-rename').value || d2.name).trim(); BK.rename = null; renderAll(); return; }
  if ((b = t.closest('[data-bk-wipe]'))) { BK.confirm = 'wipe:' + b.getAttribute('data-bk-wipe'); renderSettings(); return; }
  if (t.closest('[data-bk-confirm-cancel]')) { BK.confirm = null; renderSettings(); return; }
  if ((b = t.closest('[data-bk-wipe-go]'))) {
    var d3 = bkDrive(b.getAttribute('data-bk-wipe-go')), used = bkGridUsed(d3);
    d3.init = false; d3.games = []; d3.emus = []; d3.fw = []; BK.confirm = null;
    toast('Deleted the gridpack folder from ' + d3.label + ' (' + bkFmt(used) + '). Nothing else on the drive was touched.'); renderAll(); return;
  }
  if ((b = t.closest('[data-bk-show]'))) { S.view = 'library'; S.libFilter = 'drive:' + b.getAttribute('data-bk-show'); renderAll(); return; }
  if ((b = t.closest('[data-bk-forget]'))) { BK.drives = BK.drives.filter(function (x) { return x.id !== b.getAttribute('data-bk-forget'); }); renderSettings(); return; }
  if ((b = t.closest('[data-bk-plug]'))) {
    var d4 = bkDrive(b.getAttribute('data-bk-plug')); d4.connected = !d4.connected; d4.seen = 'just now';
    if (d4.connected && d4.init) toast((d4.name) + ' connected · ' + bkPlural(d4.games.length, 'game'));
    renderAll(); return;
  }
  /* Drive grid select bar */
  if (t.closest('[data-bk-sel-install]')) { var dd = bkLibDrive(); BK.sel.ids.forEach(function (id) { bkInstallFrom(G[id], dd); }); BK.sel = { view: null, ids: [] }; renderAll(); return; }
  if (t.closest('[data-bk-sel-rm]')) { BK.confirm = 'selrm'; renderLibrary(); return; }
  if (t.closest('[data-bk-sel-rm-go]')) {
    var dr = bkLibDrive(), n2 = BK.sel.ids.length; dr.games = dr.games.filter(function (id) { return BK.sel.ids.indexOf(id) < 0; });
    BK.sel = { view: null, ids: [] }; BK.confirm = null; toast('Removed ' + bkPlural(n2, 'game') + ' from ' + dr.name); renderAll(); return;
  }
  /* Details on a drive */
  if ((b = t.closest('[data-bk-dinst]'))) { bkInstallFrom(S.detail, bkDrive(b.getAttribute('data-bk-dinst'))); renderDetails(); return; }
  if (t.closest('[data-bk-drm]')) { BK.confirm = 'drm'; renderDetails(); return; }
  if (t.closest('[data-bk-drm-go]')) { var d5 = bkDrive(BK.detFrom); d5.games = d5.games.filter(function (id) { return id !== S.detail.id; }); toast('Removed ' + S.detail.t + ' from ' + d5.name); BK.confirm = null; closeDetails(); renderAll(); return; }
  /* Offline */
  if (t.closest('[data-bk-offline]')) { S.offline = true; freshHide(); S.view = 'library'; var r = bkReady()[0]; S.libFilter = r ? 'drive:' + r.id : 'all'; bkSession(); renderAll(); return; }
  if (t.closest('[data-bk-connect]')) { $('menu').hidden = true; S.offline = false; bkSession(); freshShow('connect'); return; }
});
document.addEventListener('change', function (e) {
  var t = e.target;
  if (t.name === 'bk-drive') { BK.dlg.drive = t.value; bkRenderDlg(); return; }
  var ch = t.getAttribute && t.getAttribute('data-bk-choice'); if (ch) { BK.dlg.choices[+ch] = t.value; bkRenderDlg(); return; }
  var o = t.getAttribute && t.getAttribute('data-bk-opt'); if (o) { BK.dlg[o] = t.checked; bkRenderDlg(); return; }
  var df = t.getAttribute && t.getAttribute('data-bk-default'); if (df) { BK.opts[df] = t.checked; }
});
/* Keep "Show each game" open across re-renders. */
document.addEventListener('toggle', function (e) { if (BK.dlg && e.target.hasAttribute && e.target.hasAttribute('data-bk-each')) BK.dlg.each = e.target.open; }, true);
document.addEventListener('keydown', function (e) {
  if (e.key === 'Escape' && BK.dlg) { e.stopImmediatePropagation(); bkCloseDlg(); }
  else if (e.key === 'Escape' && BK.sel.view) { BK.sel = { view: null, ids: [] }; renderAll(); }
}, true);

function bkBoot() {
  S.view = 'library'; S.libFilter = 'drive:t7'; S.platform = 'PS';
  paintArt(G[5]); bkSession(); renderAll(); gIcons();
}
var bkOrigClose = closeDetails;
closeDetails = function () { BK.detFrom = null; if (BK.confirm === 'drm') BK.confirm = null; bkOrigClose(); };
