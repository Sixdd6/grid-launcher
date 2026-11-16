/* ------------------------------------------------------------------ RomM › Emulators (TV)
   2026-11-15. The server's Emulators platform is a row in Library › RomM, by name among
   the platform rows, while RomM is connected and the server has one. Its view is a list
   of rows, not covers: Emulators (named for GRID: installed and registered, updates
   offered), Other files (installed as plain files in the library's Emulators folder),
   and an Other builds row that opens a page of the builds GRID doesn't install here.
   Emulator files never join RomM › Installed, All games, Recent, Updates or Discover. */
var TRE = {
  has: true, load: 'ok', os: 'Linux x64',
  emus: [
    { id: 'pcsx2', name: 'PCSX2', sys: 'PlayStation 2', sid: 'PS2', ver: 'v2.0.2', size: '38 MB', mb: 38, st: 'update', have: 'v1.6.0' },
    { id: 'pico8', name: 'Pico-8', sys: 'Pico-8', ver: 'v0.2.7a6', size: '6.1 MB', mb: 6.1, st: 'none' },
    { id: 'retroarch', name: 'RetroArch', sys: 'Many systems, through its cores', ver: 'v1.19.1', size: '156 MB', mb: 156, st: 'installed', have: 'v1.19.1' },
    { id: 'ymir', name: 'Ymir', sys: 'Sega Saturn', sid: 'SAT', ver: 'v0.1.8', size: '24 MB', mb: 24, st: 'none' },
    { id: 'duck', name: 'DuckStation', sys: 'PlayStation', sid: 'PS', ver: 'v0.1.9226', size: '42 MB', mb: 42, st: 'none', bad: 'Linux ARM64', catalog: true }
  ],
  other: [
    { id: 'xemu', file: 'xemu-v0.8.5-x86_64.AppImage', folder: 'xbox', size: '18 MB', mb: 18, why: 'Not named for GRID', st: 'installed' },
    { id: 'cemu', file: 'Cemu (v2.4) (arch-linux).AppImage', folder: 'wiiu', size: '52 MB', mb: 52, why: 'Its arch tag has no processor', st: 'none' },
    { id: 'ares', file: 'Ares Nightly (v141) (arch-linux-x64).AppImage', folder: 'multi', size: '31 MB', mb: 31, why: 'GRID doesn\'t know an emulator called "Ares Nightly"', st: 'none' }
  ],
  builds: [
    ['PCSX2 (v2.0.2) (arch-windows-x64).zip', 'ps2 folder · 31 MB', 'For Windows x64'],
    ['PCSX2 (v1.6.0) (arch-linux-x64).AppImage', 'ps2 folder · 29 MB', 'Older than v2.0.2'],
    ['Pico-8 (v0.2.7a6) (arch-linux-armhf).zip', 'pico folder · 5.8 MB', 'For Linux 32-bit ARM'],
    ['Pico-8 (v0.2.7a6) (arch-windows-x64).zip', 'pico folder · 6.4 MB', 'For Windows x64'],
    ['Dolphin (v2412) (arch-windows-x64).7z', 'multi folder · 19 MB', 'For Windows x64'],
    ['Dolphin (v2412) (arch-macos-universal).7z', 'multi folder · 44 MB', 'For macOS (universal)']
  ]
};
function treAll() { return TRE.emus.concat(TRE.other); }
function treFind(id) { return treAll().filter(function (x) { return x.id === id; })[0]; }
function treCount() { return TRE.load === 'none' ? 0 : TRE.emus.length + TRE.other.length; }
function treLabel(x) { return x.name || x.file; }

/* ---- the rail: one more platform row, by name ---- */
var treOrigPlats = rPlats;
rPlats = function () {
  var l = treOrigPlats();
  if (rOn() && TRE.has) { l.push('emulators'); l.sort(function (a, b) { return (PLAT[a] || 'Emulators') < (PLAT[b] || 'Emulators') ? -1 : 1; }); }
  return l;
};
var treOrigGroup = rRailGroup;
rRailGroup = function () {
  return treOrigGroup().replace(/(data-key="platform:emulators"[^>]*>)undefined<span class="n">\d+<\/span>/, '$1Emulators<span class="n">' + treCount() + '</span>');
};
var treOrigKind = rKind;
rKind = function () { return S.libRail === 'platform:emulators' ? 'emus' : treOrigKind(); };
var treOrigFixRail = rFixRail;
rFixRail = function () {
  if (S.libRail === 'platform:emulators' && rOn() && !TRE.has) {
    S.libRail = 'romm:installed'; S.libQ = '';
    if (TABS.library._k === 'r-platform:emulators' || (TABS.library._k || '').indexOf('r-') !== 0) TABS.library._k = 'r-romm:installed';
    return;
  }
  treOrigFixRail();
};

/* ---- the view ---- */
function treMatch(x) { var q = S.libQ.toLowerCase(); return !q || [x.name, x.sys, x.file, x.folder].some(function (s) { return s && s.toLowerCase().indexOf(q) >= 0; }); }
function treRow(x, emu) {
  var k = 're-' + x.id, a = 'treA', al = '', xl = '', val = '', sub, off = '';
  var base = emu ? x.sys + ' · ' + x.ver + ' · ' + x.size : x.folder + ' folder · ' + x.size + ' · ' + x.why;
  sub = esc(base);
  if (x.st === 'none') { al = 'Install'; val = x.size && !emu ? '' : ''; if (emu && x.catalog) sub += '<br>GRID\'s catalog copy is installed. This build installs beside it as "' + esc(x.name) + ' (server)".'; }
  else if (x.st === 'installed') { al = emu ? 'Open in Systems' : ''; xl = 'Uninstall'; val = '<span class="st ok">Installed</span>'; if (!emu) { al = 'Uninstall'; xl = ''; sub += '<br>In your library\'s Emulators folder'; } }
  else if (x.st === 'update') { al = 'Update to ' + x.ver; xl = 'Uninstall'; val = '<span class="st warn">Update available</span>'; sub = esc(x.sys) + ' · installed ' + x.have + ' · ' + x.ver + ' is on RomM'; }
  else if (x.st === 'failed') { al = 'Retry'; val = '<span class="st warn">Didn\'t install</span>'; sub += '<br><span class="tre-err">Couldn\'t install: its program is built for ' + x.bad + ', not ' + TRE.os + '. Check the file\'s arch tag on RomM.</span>'; }
  else if (x.st === 'queued') { al = 'Cancel'; val = '<span class="st">Queued</span>'; }
  else if (x.st === 'dl') { al = 'Cancel'; val = '<span class="st">' + Math.round(x.pct) + '%</span>'; sub += '<span class="tre-bar"><b style="width:' + x.pct + '%"></b></span>'; }
  else if (x.st === 'unpack') { off = 'GRID is checking ' + esc(treLabel(x)) + '\'s program. It finishes in a moment.'; val = '<span class="st"><span class="sspin"></span> Installing…</span>'; }
  var title = emu ? esc(x.name) : '<span class="tre-file">' + esc(x.file) + '</span>';
  return '<div class="srow' + (off ? ' off' : '') + (x.st === 'failed' ? ' warnrow' : '') + '" data-f data-k="' + k + '" data-a="' + (off ? 'treWhy' : a) + '" data-id="' + x.id + '"' + (off ? ' data-why="' + off + '"' : '') + (al ? ' data-al="' + esc(al) + '"' : '') + (xl ? ' data-xl="' + xl + '" data-xa="treUn"' : '') + '>' +
    '<span class="tre-tile">' + (emu ? esc(x.name.charAt(0)) : '·') + '</span><div class="lab"><div class="t">' + title + '</div><div class="s">' + sub + '</div></div><div class="val">' + val + '</div></div>';
}
function treBuildsRow() { return '<div class="srow" data-f data-k="re-builds" data-a="treBuilds" data-al="Open"><span class="tre-tile">·</span><div class="lab"><div class="t">Other builds</div><div class="s">For other computers, or older. GRID doesn\'t install these here.</div></div><div class="val">' + TRE.builds.length + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>'; }
function trePaint() {
  var m = document.querySelector('#stage .split .main'); if (!m) return;
  if (!m.querySelector('[data-tre]')) { m.removeAttribute('id'); m.innerHTML = '<div class="vh two" id="lib-h"></div><div class="scroll"><div class="tre-list" id="lib-grid" data-nav="v" data-enter="first" data-tre></div></div>'; }
  var h = $('lib-h'), box = $('lib-grid');
  var emus = TRE.emus.filter(treMatch), other = TRE.other.filter(treMatch), inst = treAll().filter(function (x) { return x.st === 'installed' || x.st === 'update'; }).length;
  var cnt = TRE.load === 'loading' ? 'Loading…' : TRE.load === 'error' ? 'On ' + esc(rHost()) : TRE.load === 'none' ? 'No builds for this computer' :
    S.libQ ? (emus.length + other.length) + ' of ' + plural(TRE.emus.length + TRE.other.length, 'item') + ' match' : plural(TRE.emus.length, 'emulator') + ' · ' + plural(TRE.other.length, 'other file') + ' · ' + inst + ' installed';
  h.innerHTML = '<div class="vh-top"><h1>Emulators</h1><span class="cnt">' + cnt + '</span></div><div class="vh-row" data-nav="h"><span class="chip">Builds for <b>' + TRE.os + '</b></span>' + (S.libQ ? '<span class="chip">Search: <b>' + esc(S.libQ) + '</b></span>' : '') + '<span class="sp"></span>' + (TRE.load === 'ok' ? '<span class="chip">' + gl('Y') + ' Search</span>' : '') + '</div>';
  var html = '';
  if (TRE.load === 'loading') html = '<div class="panel rconnecting"><h2><span class="sspin"></span>Loading the Emulators platform from ' + esc(rHost()) + '…</h2></div><div class="tre-skel"><i></i><i></i><i></i><i></i></div>';
  else if (TRE.load === 'error') html = '<div class="panel tre-panel"><h2>Can\'t load the Emulators platform</h2><p class="note">' + esc(rHost()) + ' didn\'t answer. Installed emulators still work.</p><div class="btns" data-nav="h"><button class="btn" data-f data-k="re-retry" data-a="treRetry" data-al="Retry">Retry</button></div></div>';
  else if (TRE.load === 'none') html = '<div class="panel tre-panel"><h2>Nothing here for this computer</h2><p class="note">RomM\'s Emulators platform has ' + plural(TRE.builds.length, 'build') + ', all for other systems or processors. GRID installs builds for ' + TRE.os + '.</p></div>' + treBuildsRow();
  else if (S.libQ && !emus.length && !other.length) html = '<div class="empty"><h2>No emulators or files match “' + esc(S.libQ) + '”</h2>Press ' + gl('Y') + ' to change the search.</div>';
  else {
    if (emus.length) html += '<div class="sgroup">Emulators</div><p class="note tre-note">Named for GRID. Each installs as an emulator that Settings › Systems sets up, and RomM\'s newer builds show as updates.</p>' + emus.map(function (x) { return treRow(x, true); }).join('');
    if (other.length) html += '<div class="sgroup">Other files</div><p class="note tre-note">These don\'t follow GRID\'s naming, so GRID installs them as plain files in your library\'s Emulators folder. GRID doesn\'t set them up for a system or check them for updates.</p>' + other.map(function (x) { return treRow(x, false); }).join('');
    if (!S.libQ) html += '<div class="sgroup">Not for this computer</div>' + treBuildsRow();
  }
  var k = cur && box.contains(cur) && cur.dataset.k;
  box.innerHTML = html;
  if (k) { var el = box.querySelector('[data-k="' + k + '"]') || focusables(box)[0]; if (el) { INSTANT = true; setFocus(el); INSTANT = false; } else restoreFocus({ _k: 'r-platform:emulators' }); }
  paintHints();
}
var treOrigPaint = libPaint;
libPaint = function () {
  rFixRail();
  var m = document.querySelector('#stage .split .main');
  if (rKind() === 'emus') return trePaint();
  if (m && m.querySelector('[data-tre]')) { m.removeAttribute('id'); m.innerHTML = GRIDMAIN; }
  treOrigPaint();
};
var treOrigHints = TABS.library.hints, treOrigY = TABS.library.y, treOrigRt = TABS.library.rt, treOrigLt = TABS.library.lt;
TABS.library.hints = function (el) { return rKind() === 'emus' ? (TRE.load === 'ok' ? [['Y', 'Search']] : []) : treOrigHints(el); };
TABS.library.y = function () {
  if (rKind() !== 'emus') return treOrigY();
  if (TRE.load !== 'ok') return;
  openOSK({ title: 'Search Emulators', value: S.libQ, kind: 'search', onInput: function (v) { S.libQ = v; libPaint(); }, onDone: function (v) { S.libQ = v; libPaint(); } });
};
TABS.library.rt = function () { if (rKind() !== 'emus') treOrigRt(); };
TABS.library.lt = function () { if (rKind() !== 'emus') treOrigLt(); };

/* ---- actions ---- */
function treRefresh() { if (S.tab === 'library' && rKind() === 'emus' && !S.layers.length && !S.pages.length) trePaint(); }
function treStart(x) {
  if (x.st === 'failed') x.bad = null; /* the prototype's retry succeeds, as if the file was fixed on RomM */
  var busy = treAll().some(function (y) { return y !== x && (y.st === 'dl' || y.st === 'unpack'); });
  x.st = busy ? 'queued' : 'dl'; x.pct = 0;
  x.timer = setInterval(function () {
    if (x.st === 'queued') { if (treAll().some(function (y) { return y !== x && (y.st === 'dl' || y.st === 'unpack'); })) return; x.st = 'dl'; }
    else if (x.st === 'dl') { x.pct = Math.min(100, x.pct + Math.max(6, 900 / x.mb)); if (x.pct >= 100) x.st = 'unpack'; }
    else if (x.st === 'unpack') {
      clearInterval(x.timer);
      if (x.bad) { x.st = 'failed'; toast('Couldn\'t install ' + treLabel(x), { err: 1, s: 'Its program is built for ' + x.bad + '. Nothing was set up.' }); }
      else if (x.name) { var upd = !!x.have; x.st = 'installed'; x.have = x.ver; toast(upd ? x.name + ' updated to ' + x.ver : x.name + ' ' + x.ver + ' installed from RomM', { s: 'Set up for ' + (x.id === 'retroarch' ? 'its systems' : x.sys) + ' in Settings › Systems' }); }
      else { x.st = 'installed'; toast(x.file + ' installed', { s: 'In your library\'s Emulators folder' }); }
    }
    treRefresh();
  }, 400);
}
ACT.treA = function (el) {
  var x = treFind(el.dataset.id);
  if (x.st === 'none' || x.st === 'update' || x.st === 'failed') { treStart(x); treRefresh(); return; }
  if (x.st === 'queued' || x.st === 'dl') {
    confirmBox({ title: 'Cancel the ' + treLabel(x) + ' install?', body: 'The download stops and its files are removed. You can install it again later.', ok: 'Cancel install', danger: 1, fn: function () { clearInterval(x.timer); x.st = x.have ? 'update' : 'none'; toast('Cancelled ' + treLabel(x)); treRefresh(); } });
    return;
  }
  if (x.st === 'installed' && !x.name) return ACT.treUn(el);
  if (x.st === 'installed') { var s = x.sid && sysOf(x.sid); if (s) push(systemPage(s)); else { S.pages = []; S.tab = 'settings'; S.setPane = 'systems'; TABS.settings._k = 'r-systems'; rJump(); } }
};
ACT.treUn = function (el) {
  var x = treFind(el.dataset.id);
  var body = x.name ? 'GRID deletes it from the emulators folder in your library. Games and saves stay. ' + (x.sid ? x.sys + ' games set to it' : 'Games set to it') + ' can\'t start until another emulator is picked.' : 'GRID deletes it from the Emulators folder in your library.';
  confirmBox({ title: 'Uninstall ' + (x.name ? x.name + ' ' + x.have : x.file) + '?', body: body, ok: 'Uninstall', danger: 1, fn: function () { x.st = 'none'; x.have = null; toast(treLabel(x) + ' uninstalled'); treRefresh(); } });
};
ACT.treWhy = function (el) { toast(el.dataset.why, { err: 1 }); };
ACT.treRetry = function () { TRE.load = 'loading'; trePaint(); setTimeout(function () { TRE.load = 'ok'; treRefresh(); }, 1200); };
ACT.treBuilds = function () {
  push(rowsPage({ title: 'Other builds', key: 'tb', h1: function () { return 'Other builds'; },
    sub: function () { return 'On RomM\'s Emulators platform. GRID installs the newest build for this computer\'s system and processor (' + TRE.os + '), so it doesn\'t install these here.'; },
    /* Each row is focusable but dimmed: GRID can't install it here, and A says why. */
    rows: function () { return TRE.builds.map(function (b, i) { return { html: '<div class="srow off" data-f data-k="tb-b' + i + '" data-a="treWhy" data-why="' + esc(b[2]) + '. GRID installs builds for ' + TRE.os + ' on this computer."><div class="lab"><div class="t tre-file">' + esc(b[0]) + '</div><div class="s">' + esc(b[1]) + '</div></div><div class="val">' + esc(b[2]) + '</div></div>' }; }); } }));
};

/* ---- prototype: Menu › Prototype only › Show: RomM › Emulators… ---- */
function treState(v) {
  if (v === 'gone') { TRE.has = false; TRE.load = 'ok'; toast('No Emulators platform on RomM', { s: 'Prototype: the row is hidden' }); rApply(); return; }
  TRE.has = true; TRE.load = v; rState('connected');
  S.pages = []; S.tab = 'library'; S.libRail = 'platform:emulators'; S.folds.romm = true; S.libQ = ''; TABS.library._k = 'r-platform:emulators'; rJump();
}
var treGuide = guideMenu;
guideMenu = function () {
  treGuide();
  var L = S.layers[S.layers.length - 1], box = L && L.el.querySelector('.guide'); if (!box) return;
  var after = box.querySelector('.proto'); if (!after) return;
  var it = ['Show: RomM › Emulators…', function () {
    openSheet({ title: 'Show RomM › Emulators', sub: 'Prototype only', items: [['ok', 'Files loaded'], ['loading', 'Loading'], ['error', 'Can\'t load the list'], ['none', 'Nothing for this computer'], ['gone', 'No Emulators platform (row hidden)']].map(function (o) { return { label: o[1], checked: o[0] === 'gone' ? !TRE.has : TRE.has && TRE.load === o[0], fn: function () { treState(o[0]); } }; }) });
  }];
  var i = L.gp.push(it) - 1; after.insertAdjacentHTML('afterend', '<button class="opt" data-f data-k="gp' + i + '" data-gp="' + i + '">' + esc(it[0]) + '</button>');
  restoreFocus(L);
};
