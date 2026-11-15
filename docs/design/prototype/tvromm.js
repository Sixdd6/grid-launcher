/* ------------------------------------------------------------------ RomM as a storefront
   docs/romm-storefront-plan.md, Step D2: TV mode with RomM as one optional storefront
   beside Amazon Games, Epic Games, GOG and Steam. Loaded after tvstores.js and
   tvstores_bk.js; it wraps their functions and never edits them.
   - No Server tab: three tabs (Library, Downloads, Settings) and GRID opens on the Library.
   - The Library rail has a RomM group, in name order among the stores: Installed, then
     Discover and one row per platform while RomM is connected.
   - Settings › Game stores (was Stores) has a RomM row and page; Connection is gone.
   - A neutral welcome replaces Connect. Systems and firmware work without RomM.
   No top-bar connection text, no offline hint line, no Menu › Reconnect.
   Placeholder titles; the RomM mark is a monogram tile, never a logo. */
var ROMM = { conn: 'connected', url: 'https://romm.home.lan', user: 'sam', auth: 'token', err: '', busy: false, startupProbe: false, next: 'ok', setupDone: true, hue: 'hsl(258 50% 46%)' };
var STOREFRONT_ORDER = ['amazon', 'epic', 'gog', 'romm', 'steam'];
/* connection: connected | unreachable | expired | not_connected. While the startup probe
   runs, the stored "connected" waits for its answer: Discover shows "Connecting to RomM…". */
function rOn() { return ROMM.conn === 'connected' && !ROMM.startupProbe; }
function rLive() { return rOn() || ROMM.startupProbe; }
function rHost(u) { return String(u == null ? ROMM.url : u).replace(/^https?:\/\//, '').replace(/\/$/, ''); }
function rmark(cls) { return smark({ hue: ROMM.hue, mono: 'R' }, cls); }
function rInstalled() { return G.filter(function (g) { return g.inst; }); }
/* Installed rows RomM doesn't have: no RomM link (NOT LINKED, Brass Comet), or from
   another RomM server or removed from this one (NOT ON ROMM, Sky Lantern). They show
   in their platform's grid but never count as RomM's games (decisions 14 and 15). */
function rLocal(g) { return !!(g.unlinked || g.offRomm); }
function rOnRomm(p) { return G.filter(function (g) { return g.p === p && !rLocal(g); }); }
G.push({ id: 26, t: 'Sky Lantern', c: 'hsl(215 45% 40%)', p: 'SNES', inst: 1, rel: 1300, offRomm: 'romm.old.lan' });
function rInRail() { return ROMM.conn !== 'not_connected' || ROMM.startupProbe || rInstalled().length > 0; }
function rPlats() { return Object.keys(PLAT).filter(function (p) { return G.some(function (g) { return g.p === p; }); }).sort(function (a, b) { return PLAT[a] < PLAT[b] ? -1 : 1; }); }
var plural = function (n, w) { return n + ' ' + w + (n === 1 ? '' : 's'); };
/* "Not connected, no RomM games": a fresh profile. The prototype hides its installed
   RomM games and brings them back when the state changes. */
var rSaved = null;
function rSetNoGames(on) {
  if (on && !rSaved) { rSaved = {}; G.forEach(function (g) { rSaved[g.id] = g.inst; g.inst = 0; }); }
  if (!on && rSaved) { G.forEach(function (g) { if (g.id in rSaved) g.inst = rSaved[g.id]; }); rSaved = null; }
}
/* What a needs-RomM line says after its first sentence, per connection state. */
function rFix() { return ROMM.conn === 'expired' ? 'Your RomM sign-in expired. Sign in again in Settings › Game stores.' : ROMM.conn === 'unreachable' ? 'Can\'t reach RomM right now. GRID tries again when you press Retry.' : 'Connect RomM in Settings › Game stores.'; }

/* ---- tabs and chrome: no Server tab, no connection text, no offline line ---- */
TAB_ORDER.splice(TAB_ORDER.indexOf('server'), 1);
var tvrTop = paintTop;
paintTop = function () { tvrTop(); var d = document.querySelector('#top .status .dot'); if (d && d.parentNode) d.parentNode.remove(); };

/* ---- Library rail: no Platforms folder; the RomM group sits between GOG and Steam ---- */
S.folds.romm = true;
function rFlag() {
  if (ROMM.busy) return '<span class="sflag rbusy" aria-label="Connecting to RomM"><span class="sspin"></span></span>';
  if (ROMM.conn === 'unreachable') return '<span class="sflag" aria-label="Can\'t reach RomM">!</span>';
  if (ROMM.conn === 'expired') return '<span class="sflag" aria-label="RomM sign-in expired">!</span>';
  if (ROMM.conn === 'not_connected') return '<span class="sflag off" aria-label="Not connected">off</span>';
  return '';
}
function rRailGroup() {
  if (!rInRail()) return '';
  var row = function (key, label, n) { return railItem(key, label, n, S.libRail === key, 'lib').replace('class="ri', 'class="ri sub'); };
  var html = row('romm:installed', 'Installed', rInstalled().length);
  if (rLive()) html += row('romm:discover', 'Discover', null);
  if (rOn()) html += rPlats().map(function (p) { return row('platform:' + p, PLAT[p], rOnRomm(p).length); }).join('');
  return folder('romm', rmark() + 'RomM', 0, '', html).replace('</button>', rFlag() + '</button>');
}
var tvrRail = libRailHTML;
libRailHTML = function () {
  var h = tvrRail();
  var a = h.indexOf('<button class="ri fold" data-f data-k="fold-plat"'), b = h.indexOf('<button class="ri fold" data-f data-k="fold-drives"');
  if (a >= 0 && b > a) h = h.slice(0, a) + h.slice(b);
  var g = rRailGroup(), s = h.indexOf('<button class="ri fold" data-f data-k="fold-st-steam"');
  return s >= 0 ? h.slice(0, s) + g + h.slice(s) : h + g;
};
/* When a selected row goes away (RomM drops while Discover or a platform is open), the
   selection falls back to RomM › Installed, or All games when the group is gone. */
function rFixRail() {
  var r = S.libRail, to = null;
  if (r.indexOf('romm:') !== 0 && r.indexOf('platform:') !== 0) return;
  if (!rInRail()) to = 'all';
  else if ((r === 'romm:discover' && !rLive()) || (r.indexOf('platform:') === 0 && !rOn())) to = 'romm:installed';
  if (!to) return;
  S.libRail = to; S.libQ = ''; if (S.selMode) { S.selMode = false; S.sel = []; }
  if (TABS.library._k === 'r-' + r || (TABS.library._k || '').indexOf('r-') !== 0) TABS.library._k = 'r-' + to;
}
/* A state change: redraw what is on screen, keeping focus, or moving it to the row now
   in place of the one that went away. */
function rApply() {
  var was = S.libRail; rFixRail();
  if (S.libRail !== was && S.tab === 'library' && !S.pages.length && !S.layers.length) {
    if (cur) cur.classList.remove('f'); cur = null;
    renderStage(); INSTANT = true; restoreFocus({ _k: 'r-' + S.libRail }); INSTANT = false; paintTop(); paintHints(); return;
  }
  var k = cur && cur.dataset.k;
  repaint();
  /* A rail row that went away hands focus to the row now selected (RomM › Installed). */
  if (k && !scope().querySelector('[data-k="' + k + '"]')) restoreFocus(S.libRail !== was && !S.pages.length && !S.layers.length ? { _k: 'r-' + S.libRail } : S.layers.length ? S.layers[S.layers.length - 1] : S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab]);
  paintTop(); paintHints();
}

/* ---- the Library views: Installed, Discover, a platform ---- */
var RSORT = { 'Recently played': function (a, b) { return (a.played == null ? 999 : a.played) - (b.played == null ? 999 : b.played); }, 'Title': function (a, b) { return a.t < b.t ? -1 : 1; }, 'Platform': function (a, b) { return a.p < b.p ? -1 : a.p > b.p ? 1 : 0; }, 'Recently installed': function (a, b) { return b.id - a.id; } };
function rKind() { var r = S.libRail; return r === 'romm:discover' ? 'disc' : r === 'romm:installed' ? 'inst' : r.indexOf('platform:') === 0 ? 'plat' : ''; }
var tvrList = libList;
libList = function () {
  var k = rKind(), L;
  if (!k) return tvrList();
  if (k === 'disc') return [];
  L = k === 'inst' ? rInstalled() : G.filter(function (g) { return g.p === S.libRail.slice(9); });
  if (S.libQ) L = L.filter(function (g) { return g.t.toLowerCase().indexOf(S.libQ.toLowerCase()) >= 0; });
  return k === 'inst' ? L.sort(RSORT[S.libSort]) : L;
};
var GRIDMAIN = '<div class="vh" id="lib-h"></div><div class="scroll"><div class="grid" id="lib-grid" data-nav="grid" data-enter="first"></div></div><div class="selbar" id="selbar" hidden></div>';
var tvrPaint = libPaint;
libPaint = function () {
  rFixRail();
  var m = document.querySelector('#stage .split .main'); if (!m) return;
  var k = rKind();
  if (k === 'disc') { m.id = 'srv-main'; m.innerHTML = rDiscoverHTML(); var f = m.querySelector('.cv'); if (f) hero(byId(f.dataset.id)); return; }
  if (!$('lib-grid')) { m.removeAttribute('id'); m.innerHTML = GRIDMAIN; }
  if (k === 'inst') return rPaintInstalled();
  if (k === 'plat') return rPaintPlatform(S.libRail.slice(9));
  tvrPaint();
  var e = $('lib-grid') && $('lib-grid').querySelector('.empty');
  if (e) e.innerHTML = e.innerHTML.replace('Install games from the Server tab.', 'Install games from RomM, a store or a Backup drive.');
};
function rDiscoverHTML() {
  if (!rOn()) return '<div class="list"><div class="list-in"><div class="panel rconnecting"><h2><span class="sspin"></span>Connecting to ' + esc(rHost()) + '…</h2><p class="note" style="margin:0">Discover and the platforms show when RomM answers. Installed games play now.</p></div><div class="sskel"><i></i><i></i><i></i><i></i><i></i></div></div></div>';
  /* Installed rows RomM doesn't have (NOT LINKED, NOT ON ROMM) never show in Discover. */
  var all = G, h; G = G.filter(function (g) { return !rLocal(g); });
  try { h = discoverHTML(); } finally { G = all; }
  return h.replace('to your server', 'on RomM');
}
/* The Installed view's one-line banner: unreachable or sign-in expired. Not connected
   has none: the rail's "off" flag says it, as for a store. */
function rBanner() {
  if (ROMM.conn === 'unreachable') return '<div class="rbanner" data-nav="h"><span class="rbx">!</span><span class="rbt"><b>Can\'t reach RomM</b> · ' + esc(rHost()) + ' didn\'t answer. Installed games still play.</span><button class="btn" data-f data-k="rb-act" data-a="rReconnect" data-al="Retry"' + (ROMM.busy ? ' disabled data-why="GRID is trying RomM now."' : '') + '>' + (ROMM.busy ? '<span class="sspin"></span>Retrying…' : 'Retry') + '</button></div>';
  if (ROMM.conn === 'expired') return '<div class="rbanner" data-nav="h"><span class="rbx">!</span><span class="rbt"><b>RomM sign-in expired</b> · Sign in again to browse, install and sync saves. Installed games still play.</span><button class="btn" data-f data-k="rb-act" data-a="rSignIn" data-al="Sign in again">Sign in again…</button></div>';
  return '';
}
function rPaintInstalled() {
  var L = libList(), h = $('lib-h'), n = rInstalled().length;
  h.className = 'vh two';
  var chip = rOn() ? '<span class="chip">Signed in as <b>' + esc(ROMM.user) + '</b> · ' + esc(rHost()) + '</span>' : ROMM.conn === 'not_connected' ? '<span class="chip">Not connected</span>' : '';
  h.innerHTML = '<div class="vh-top"><h1>' + rmark() + 'RomM <span class="muted">· Installed</span></h1><span class="cnt">' + (S.libQ ? L.length + ' of ' + plural(n, 'game') + ' match' : plural(n, 'game')) + '</span></div>' + rBanner() +
    '<div class="vh-row" data-nav="h">' + chip + (S.libQ ? '<span class="chip">Search: <b>' + esc(S.libQ) + '</b></span>' : '') + '<span class="sp"></span>' + (n ? '<button class="btn" data-f data-k="rh-bk" data-a="rBackupInst" data-al="Backup">Backup all ' + n + '…</button>' : '') + '<span class="chip">' + gl('RT') + ' Select</span><span class="chip">' + gl('LT') + ' Sort: <b>' + S.libSort + '</b></span><span class="chip">' + gl('Y') + ' Search</span></div>';
  $('lib-grid').innerHTML = L.length ? L.map(function (g) { return cover(g, { k: 'rg', plat: true }); }).join('') :
    '<div class="empty">' + (S.libQ ? '<h2>No installed RomM games match “' + esc(S.libQ) + '”</h2>Press ' + gl('Y') + ' to change the search.' : '<h2>No RomM games installed yet</h2>' + (rOn() ? 'Install games from Discover or a platform under it.' : rFix())) + '</div>';
  selPaint(L);
}
/* Firmware summary for a platform header chip and the Systems rows. */
var FW = {
  PS: { files: [{ n: 'scph5500.bin', size: '512 KB', st: 'romm' }, { n: 'scph5501.bin', size: '512 KB', req: 1, st: 'romm' }, { n: 'scph5502.bin', size: '512 KB', st: 'romm' }, { n: 'scph1001.bin', size: '512 KB', st: '' }], romm: 1 },
  PS2: { files: [{ n: 'scph39001.bin', size: '4 MB', req: 1, st: '' }, { n: 'scph70004.bin', size: '4 MB', st: '' }], romm: 1 },
  SAT: { files: [{ n: 'sega_101.bin', size: '512 KB', req: 1, st: '' }, { n: 'mpr-17933.bin', size: '512 KB', st: '' }], romm: 1 },
  PS3: { files: [{ n: 'PS3UPDAT.PUP', size: '198 MB', req: 1, st: '' }], pup: 1 },
  SWITCH: { files: [{ n: 'prod.keys', size: '', req: 1, st: '', nohash: 1 }, { n: 'title.keys', size: '', st: '', nohash: 1 }] }
};
function rFwChip(s) {
  var f = FW[s.id]; if (!f) return '<span class="chip">No firmware needed</span>';
  var have = f.files.filter(function (x) { return x.st; }).length, all = f.files.length;
  var txt = have === all ? 'Firmware: ' + plural(all, 'file') + ' · installed' : have ? 'Firmware: ' + have + ' of ' + plural(all, 'file') : 'Firmware: ' + plural(all, 'file') + ' needed';
  return '<button class="chip' + (rFwMissing(s) ? ' warn' : '') + '" data-f data-k="ch-fw" data-a="system" data-id="' + s.id + '" data-al="Open in Systems">' + txt + '</button>';
}
function rFwMissing(s) { var f = FW[s.id]; return !!f && f.files.some(function (x) { return x.req && !x.st; }); }
function rPaintPlatform(p) {
  var all = G.filter(function (g) { return g.p === p; }), rm = rOnRomm(p), loc = all.length - rm.length, L = libList(), inst = rm.filter(function (g) { return g.inst; }).length, s = sysOf(p), nat = p === 'WIN' || p === 'LINUX', h = $('lib-h');
  h.className = 'vh two';
  var chips = !nat && s ? '<button class="chip' + (s.emu ? '' : ' warn') + '" data-f data-k="ch-emu" data-a="system" data-id="' + p + '" data-al="Open in Systems">' + (s.emu ? 'Emulator: <b>' + s.emu + '</b>' : 'No default emulator') + '</button>' + rFwChip(s) : '';
  /* The count is RomM's games (as the rail row); installed games RomM doesn't have are
     named after it, not counted in it. */
  var cnt = S.libQ ? L.length + ' of ' + plural(all.length, 'game') + ' match' : plural(rm.length, 'game') + ' on RomM · ' + inst + ' installed' + (loc ? ' · plus ' + loc + ' only on this computer' : '');
  h.innerHTML = '<div class="vh-top"><h1>' + PLAT[p] + '</h1><span class="cnt">' + cnt + '</span></div><div class="vh-row" data-nav="h">' + chips + (S.libQ ? '<span class="chip">Search: <b>' + esc(S.libQ) + '</b></span>' : '') + '<span class="sp"></span>' +
    (all.length ? '<button class="btn" data-f data-k="bk-all" data-a="backupPlat" data-id="' + p + '" data-al="Backup">Backup all ' + all.length + '…</button>' : '') + '<span class="chip">' + gl('RT') + ' Select</span><span class="chip">' + gl('Y') + ' Search</span></div>';
  $('lib-grid').innerHTML = L.length ? L.map(function (g) { return cover(g, { k: 'pg' }); }).join('') : '<div class="empty"><h2>No games match “' + esc(S.libQ) + '”</h2>Press ' + gl('Y') + ' to change the search.</div>';
  selPaint(L);
}
/* Installed rows RomM doesn't have show in their platform with a tag: NOT LINKED (no
   RomM link) or NOT ON ROMM (from another RomM server, or removed from this one). */
var tvrCover = cover;
cover = function (g, o) {
  var h = tvrCover(g, o), t = g.missing ? '' : g.unlinked ? 'NOT LINKED' : g.offRomm ? 'NOT ON ROMM' : '';
  return t ? h.replace('<span class="cv-art">', '<span class="cv-art"><span class="tag nl">' + t + '</span>') : h;
};
/* Arriving on the Library while Discover is selected focuses Discover's content (the
   cover last focused there, else the first), as the Server tab did. */
var tvrRender = render;
render = function () {
  tvrRender();
  if (S.tab === 'library' && S.libRail === 'romm:discover' && !S.pages.length && !S.layers.length && cur && cur.closest('.rail')) {
    var dk = TABS.server._dk, dc = (dk && document.querySelector('#disc [data-k="' + dk + '"]')) || document.querySelector('#disc .cv');
    if (dc) { INSTANT = true; setFocus(dc); INSTANT = false; }
  }
};
var tvrHints = TABS.library.hints, tvrY = TABS.library.y, tvrRt = TABS.library.rt, tvrLt = TABS.library.lt;
TABS.library.hints = function (el) { var k = rKind(); if (k === 'disc') return rOn() ? [['Y', 'Search']] : []; if (k === 'inst') return [['Y', 'Search'], ['LT', 'Sort'], ['RT', 'Select']]; if (k === 'plat') return [['Y', 'Search'], ['RT', 'Select']]; return tvrHints(el); };
TABS.library.y = function () {
  var k = rKind();
  if (k === 'disc') { if (rOn()) searchAll(); return; }
  if (k === 'plat') { var p = S.libRail.slice(9); openOSK({ title: 'Search ' + PLAT[p], value: S.libQ, kind: 'search', onInput: function (v) { S.libQ = v; libPaint(); }, onDone: function (v) { S.libQ = v; libPaint(); } }); return; }
  if (k === 'inst') { openOSK({ title: 'Search RomM · Installed', value: S.libQ, kind: 'search', onInput: function (v) { S.libQ = v; libPaint(); }, onDone: function (v) { S.libQ = v; libPaint(); } }); return; }
  tvrY();
};
TABS.library.rt = function () { var k = rKind(); if (k === 'disc') return; if (k) { selToggle(); return; } tvrRt(); };
TABS.library.lt = function () { var k = rKind(); if (k === 'disc' || k === 'plat') return; tvrLt(); };
ACT.rBackupInst = function () { var L = rInstalled(); openBackup(L.map(function (g) { return g.id; }), 'Backup RomM · ' + plural(L.length, 'game')); };

/* ---- Where GRID opens (desktop and TV alike) ----
   Automatic and Discover: RomM › Discover when RomM is connected at startup (the config
   says so; the probe runs after the shell is up), else All games. Library: All games.
   Never a Backup drive, even when RomM was never connected (decision 16). */
openStart = function () {
  S.pages = []; S.layers.forEach(function (l) { l.el.remove(); }); S.layers = []; S.selMode = false; S.startup = false;
  var conf = ROMM.conn === 'connected' || ROMM.startupProbe, where = CFG.startOn;
  S.tab = 'library'; S.libQ = '';
  if (where !== 'library' && conf) { S.libRail = 'romm:discover'; S.folds.romm = true; }
  else S.libRail = 'all';
  TABS.library._k = 'r-' + S.libRail;
  /* A restart lands where the setting says, never on the row focused before it. */
  if (cur) cur.classList.remove('f'); cur = null;
  render();
};

/* ---- Settings: Connection goes; Stores becomes Game stores, in name order ---- */
(function () {
  var ids = function () { return PANES.map(function (p) { return p[0]; }); };
  var i = ids().indexOf('connection'); if (i >= 0) PANES.splice(i, 1);
  var st = PANES.splice(ids().indexOf('stores'), 1)[0]; st[1] = 'Game stores';
  PANES.splice(ids().indexOf('cloud') + 1, 0, st);
})();
function rPill() {
  if (ROMM.busy) return '<span class="st"><span class="sspin"></span> Connecting…</span>';
  return { connected: '<span class="st ok">Connected</span>', unreachable: '<span class="st warn">Can\'t reach</span>', expired: '<span class="st warn">Sign-in expired</span>', not_connected: '<span class="st">Not connected</span>' }[ROMM.conn];
}
function rSub() { return ROMM.conn === 'not_connected' ? (ROMM.url ? 'Not signed in · ' + rHost() : 'Your own game server') : 'Signed in as ' + ROMM.user + ' · ' + rHost(); }
function rStoreRow() { return '<div class="srow" data-f data-k="sto-romm" data-a="rCard" data-al="Open"><span style="margin-right:.2rem">' + rmark('lg') + '</span><div class="lab"><div class="t">RomM</div><div class="s">' + esc(rSub()) + '</div></div><div class="val">' + rPill() + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>'; }
/* Cloud saves and RetroAchievements say where they need RomM. */
function rNeed(t, p) {
  var c = ROMM.conn, act = c === 'not_connected' ? ['rCard', 'Set up RomM…', 'Open'] : c === 'expired' ? ['rSignIn', 'Sign in again…', 'Sign in again'] : ['rReconnect', ROMM.busy ? 'Retrying…' : 'Retry', 'Retry'];
  return '<div class="rneed' + (c === 'not_connected' ? '' : ' warn') + '"><div class="rneed-t"><b>' + t + '</b><span>' + p + '</span></div><button class="btn" data-f data-k="rneed" data-a="' + act[0] + '" data-al="' + act[2] + '"' + (ROMM.busy ? ' disabled data-why="GRID is trying RomM now."' : '') + '>' + act[1] + '</button></div>';
}
/* The panels' first sentence says why, per state (desktop's words, decision 17). */
function rWhy() { return ROMM.startupProbe ? 'GRID is still connecting to ' + esc(rHost()) + '.' : ROMM.conn === 'unreachable' ? 'GRID can\'t reach ' + esc(rHost()) + ' right now.' : ROMM.conn === 'expired' ? 'Your RomM sign-in expired.' : 'RomM isn\'t connected.'; }
function rNeedCloud() { return rNeed('Cloud saves need RomM', rWhy() + ' GRID keeps your emulator saves and states on your RomM server. These settings apply when it\'s connected. GOG and Epic games sync with their own store\'s cloud.'); }
function rNeedRa() { return rNeed('Achievement progress needs RomM', rWhy() + ' Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin.'); }
var tvrSetPaint = setPaint;
setPaint = function () {
  tvrSetPaint();
  var pane = $('pane'); if (!pane) return;
  var h = pane.querySelector('.pane-h');
  if (S.setPane === 'stores') {
    var h1 = pane.querySelector('.pane-h h1'); if (h1) h1.textContent = 'Game stores';
    var n = pane.querySelector('.note'); if (n) n.textContent = 'Each one you connect gets a group in the Library, in this order.';
    var steam = pane.querySelector('[data-k="sto-steam"]');
    if (steam) steam.insertAdjacentHTML('beforebegin', rStoreRow()); else pane.insertAdjacentHTML('beforeend', rStoreRow());
  }
  if (!rOn() && h && S.setPane === 'cloud') h.insertAdjacentHTML('afterend', rNeedCloud());
  if (!rOn() && h && S.setPane === 'ra') h.insertAdjacentHTML('afterend', rNeedRa());
};
var tvrRows = paneRows;
paneRows = function (id) {
  var R = tvrRows(id);
  if (id === 'appearance') R[0].sub = 'Here and in desktop mode' + (!rOn() && CFG.startOn !== 'library' ? '. RomM isn\'t connected, so GRID opens on All games.' : '');
  if (id === 'appearance') R[0].opts = [['auto', 'Automatic: Discover when RomM is connected, else Library'], ['discover', 'Discover (when RomM is connected)'], ['library', 'Library']];
  if (id === 'ra') R.forEach(function (r) { if (r.label === 'Save') r.fn = function () { toast('RetroAchievements login saved', { s: rOn() ? 'Sent to 3 emulators and your RomM account' : 'Sent to 3 emulators' }); }; });
  return R;
};

/* ---- the RomM page (Settings › Game stores › RomM) ---- */
/* Jump to a place: drop the old focus first, so a redraw of the same tab can't put it back. */
function rJump() { if (cur) cur.classList.remove('f'); cur = null; render(); }
function rToLibrary() { S.pages = []; S.startup = false; S.tab = 'library'; S.libRail = 'romm:installed'; S.folds.romm = true; S.libQ = ''; TABS.library._k = 'r-romm:installed'; rJump(); }
function rDisconnectBox() {
  confirmBox({ title: 'Disconnect RomM?', body: 'GRID deletes your RomM sign-in from the OS keyring and stays open. Installed RomM games stay and still play. Cloud saves, firmware downloads and updates stop.', ok: 'Disconnect', danger: 1, fn: function () {
    ROMM.conn = 'not_connected'; ROMM.err = ''; ROMM.busy = false;
    toast('RomM disconnected', { s: 'Installed RomM games stay and still play.' }); rApply();
  } });
}
function rCardPage() {
  push(rowsPage({
    title: 'RomM', key: 'rc', h1: function () { return rmark('lg') + ' RomM'; },
    sub: function () {
      var c = ROMM.conn, u = '<b>' + esc(ROMM.user) + '</b>';
      return c === 'connected' ? 'Signed in as ' + u + ' · ' + esc(rHost()) : c === 'unreachable' ? 'Signed in as ' + u + ' · can\'t reach ' + esc(rHost()) : c === 'expired' ? 'Sign-in expired · ' + esc(rHost()) : ROMM.url ? 'Not signed in · ' + esc(rHost()) : 'Your own game server';
    },
    rows: function () {
      var R = [], c = ROMM.conn, n = rInstalled().length;
      if (c === 'unreachable') R.push({ warn: 1, note: '<b>Can\'t reach RomM.</b> ' + esc(rHost()) + ': ' + esc(ROMM.err || 'connection refused') + '. Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it\'s back.' });
      if (c === 'expired') R.push({ warn: 1, note: '<b>Your RomM sign-in expired.</b> Sign in again to browse, install and sync saves. Installed games still play.' });
      if (c === 'not_connected') R.push({ note: 'Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware.' + (n ? ' The ' + plural(n, 'RomM game') + ' installed here still play.' : '') });
      if (c !== 'not_connected' || ROMM.url) { R.push({ info: 1, label: 'Server', val: esc(ROMM.url) }); R.push({ info: 1, label: 'User', val: esc(ROMM.user) }); }
      if (c !== 'not_connected') R.push({ info: 1, label: 'Sign-in', val: (ROMM.auth === 'token' ? 'API token' : 'Password') + ' · kept in the OS keyring' });
      R.push({ info: 1, label: 'Installed RomM games', val: n });
      if (c !== 'not_connected') R.push({ label: 'Edit sign-in…', sub: 'Server, user and token', al: 'Edit', fn: function () { rSignInPage('edit', 'settings'); } });
      if (n || rOn()) R.push({ label: 'Show in Library', fn: rToLibrary });
      if (ROMM.url) R.push({ label: 'Open in browser', sub: 'Opens ' + esc(rHost()) + ' in your web browser, over TV mode', fn: function () { toast('Opening ' + rHost() + ' in your browser…', { s: 'It opens over TV mode. Close it to come back.' }); } });
      if (c !== 'not_connected') R.push({ label: 'Disconnect…', danger: 1, sub: 'Installed RomM games stay', fn: rDisconnectBox });
      return R;
    },
    main: function () {
      var c = ROMM.conn;
      if (ROMM.busy) return { label: c === 'not_connected' ? 'Connecting…' : 'Reconnecting…', disabled: 1, why: 'GRID is trying RomM now.' };
      if (c === 'unreachable') return { label: 'Reconnect', fn: rReconnect };
      if (c === 'expired') return { label: 'Sign in again…', fn: function () { rSignInPage('again', 'settings'); } };
      if (c === 'not_connected') return { label: 'Connect RomM…', fn: function () { rSignInPage('connect', 'settings'); } };
      return null;
    },
    get mainHint() { return ROMM.conn === 'unreachable' ? 'Reconnect' : ROMM.conn === 'expired' ? 'Sign in again' : 'Connect'; }
  }));
}
function rReconnect() {
  if (ROMM.busy) return;
  ROMM.busy = true; rApply(); toast('Reconnecting to RomM…', { s: rHost() });
  setTimeout(function () {
    ROMM.busy = false;
    if (ROMM.next === 'ok') { ROMM.conn = 'connected'; ROMM.err = ''; toast('Connected to RomM as ' + ROMM.user, { s: rHost() }); }
    else if (ROMM.next === 'rejected') { ROMM.conn = 'expired'; rExpiredToast(); }
    else { ROMM.conn = 'unreachable'; ROMM.err = 'connection refused'; toast('Can\'t reach RomM', { err: 1, s: rHost() + ' didn\'t answer. Installed games still play.' }); }
    rApply();
  }, 1200);
}
function rExpiredToast() { toast('RomM sign-in expired', { err: 1, s: 'Sign in again to browse, install and sync saves. Installed games still play.', act: ['Sign in again…', function () { rSignInPage('again', 'settings'); }] }); }

/* ---- the RomM sign-in page: the welcome's RomM row and the page's Edit / Connect /
   Sign in again. Fields open the on-screen keyboard; View pastes into the focused field. */
function rSignInPage(mode, from) {
  var F = { url: mode === 'connect' && !ROMM.url ? '' : ROMM.url, how: ROMM.auth || 'token', user: ROMM.user, secret: '', err: '', busy: false };
  var title = mode === 'edit' ? 'Edit RomM sign-in' : mode === 'again' ? 'Sign in to RomM again' : 'Connect to RomM';
  var P = {
    title: title, F: F, from: from, rsign: true,
    render: function () {
      var ready = F.url && F.secret && (F.how === 'token' || F.user);
      var why = !F.url ? 'Enter your RomM server address first.' : F.how !== 'token' && !F.user ? 'Enter your RomM username first.' : 'Enter your ' + (F.how === 'token' ? 'API token' : 'password') + ' first.';
      /* Sign in again opens on the token: the server and user are already right. */
      var field = function (k, lab, val, mask, ph) { return '<div class="field" data-f data-k="rs-' + k + '" data-a="rSignField" data-key="' + k + '" data-al="Edit"' + (mode === 'again' && k === 'secret' ? ' data-autofocus' : '') + '><span class="lab">' + lab + '</span><span class="v' + (val ? '' : ' ph') + '">' + (val ? (mask ? '••••••••••••' : esc(val)) : ph) + '</span></div>'; };
      var sub = mode === 'again' ? 'RomM no longer accepts GRID\'s token for ' + esc(F.user || 'your user') + '. Enter a new one.' : mode === 'edit' ? 'Change the server or the sign-in. Installed RomM games stay; GRID links them to the server\'s games when it connects.' : 'GRID browses your RomM server, installs its games with an emulator and syncs their saves.';
      this._main = 1;
      return '<div class="page"><div class="page-in rsign" data-nav="v"><h1>' + rmark('lg') + ' ' + esc(title) + '</h1><p class="sub">' + sub + '</p>' + (F.err ? '<p class="errline">' + esc(F.err) + '</p>' : '') +
        field('url', 'Server', F.url, 0, 'https://…') +
        '<div class="field" data-f data-k="rs-how" data-a="rSignField" data-key="how" data-al="Change"><span class="lab">Sign in with</span><span class="v">' + (F.how === 'token' ? 'API token' : 'Username and password') + '</span></div>' +
        (F.how === 'token' ? field('secret', 'API token', F.secret, 1, 'Paste or type your token') : field('user', 'Username', F.user, 0, 'Your RomM user') + field('secret', 'Password', F.secret, 1, 'Your password')) +
        '<div class="bk-foot"><span class="muted">' + (F.how === 'token' ? 'Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file.' : 'GRID keeps your password in the OS keyring, never in a file.') + '</span><button class="btn pri" data-f data-k="rs-go" data-a="rSignGo"' + (F.busy ? ' disabled data-why="GRID is signing in to RomM."' : ready ? '' : ' disabled data-why="' + why + '"') + '>' + (F.busy ? '<span class="sspin"></span>Connecting…' : gl('MENU') + (mode === 'edit' ? 'Save' : 'Connect')) + '</button></div></div></div>';
    },
    hints: function () { return [['VIEW', 'Paste'], ['MENU', mode === 'edit' ? 'Save' : 'Connect']].concat(S.startup ? [['B', 'Back']] : []); },
    menu: function () { var b = document.querySelector('[data-k="rs-go"]'); if (b && b.disabled) { toast(b.dataset.why, { err: 1 }); return; } ACT.rSignGo(); }
  };
  push(P);
}
function rSignTop() { var p = S.pages[S.pages.length - 1]; return p && p.rsign ? p : null; }
ACT.rSignField = function (el) {
  var P = rSignTop(), F = P.F, k = el.dataset.key;
  if (k === 'how') { openSheet({ title: 'Sign in with', items: [{ label: 'API token', sub: 'Recommended', checked: F.how === 'token', fn: function () { F.how = 'token'; F.secret = ''; renderStage(); } }, { label: 'Username and password', checked: F.how !== 'token', fn: function () { F.how = 'pw'; F.secret = ''; renderStage(); } }] }); return; }
  openOSK({ title: { url: 'RomM server address', secret: F.how === 'token' ? 'API token' : 'Password', user: 'Username' }[k], value: k === 'secret' ? '' : F[k], mask: k === 'secret', kind: k === 'url' ? 'url' : 'text', onDone: function (v) { F[k] = v; F.err = ''; renderStage(); } });
};
ACT.rSignGo = function () {
  var P = rSignTop(); if (!P) return; var F = P.F;
  F.busy = true; F.err = ''; renderStage(); paintHints();
  setTimeout(function () {
    F.busy = false;
    if (ROMM.next === 'rejected') { F.err = F.how === 'token' ? 'RomM rejected this token (401). Check it, or make a new one.' : 'RomM rejected the username or password (401).'; F.secret = ''; renderStage(); restoreFocus({ _k: 'rs-secret' }); return; }
    if (ROMM.next === 'unreachable') { F.err = 'Couldn\'t reach ' + F.url + '. Check the address, and that the server is running.'; renderStage(); restoreFocus({ _k: 'rs-url' }); return; }
    ROMM.url = F.url; if (F.how !== 'token') ROMM.user = F.user; else if (!ROMM.user) ROMM.user = 'sam';
    ROMM.auth = F.how; ROMM.conn = 'connected'; ROMM.err = ''; ROMM.startupProbe = false;
    toast('Connected to RomM as ' + ROMM.user, { s: rHost() });
    if (P.from === 'welcome') { pop(); return; }
    if (!ROMM.setupDone) { S.setupReturn = 'settings'; S.pages = []; S.startup = true; push(rChoosePage()); return; }
    pop(); rApply();
  }, 1000);
};
/* View pastes the clipboard into the focused field (the prototype's clipboard holds an
   address on the Server field and a token on the token field). */
var tvrPress = press;
press = function (b) {
  var P = rSignTop(), W = rWelTop(), top = S.layers[S.layers.length - 1];
  if (b === 'VIEW' && P && !top && !newestAction()) {
    var k = cur && cur.dataset.key;
    if (k === 'url' || k === 'secret') { P.F[k] = k === 'url' ? 'https://romm.home.lan' : 'rmm_pasted_token'; renderStage(); toast('Pasted from the clipboard'); }
    else toast('Nothing to paste', { s: 'Select the Server or token field first.' });
    return;
  }
  if (b === 'B' && W && W.W.step === 2 && !top) { W.W.step = 1; renderStage(); restoreFocus({ _k: 'w-lib' }); return; }
  return tvrPress(b);
};

/* ---- the welcome (first run; replaces Connect) ----
   Step 1: the library folder (required). Step 2: "Connect your games", optional, the
   storefronts in name order. Its button reads Skip for now until one is connected, then
   Continue. With RomM connected and setup needed, Choose your systems follows. Start
   opens the short Menu (Switch to desktop mode, Quit GRID), as on Connect before. */
var RWSUB = { amazon: 'Your Amazon Games library. Sign in on Amazon\'s page.', epic: 'Your Epic Games library. Sign in on Epic\'s page.', gog: 'Your GOG library. Sign in on GOG\'s page.', steam: 'Games in Steam on this computer. A Web API key lists the rest.' };
function rWelTop() { var p = S.pages[S.pages.length - 1]; return p && p.W ? p : null; }
function rConnected(id) { return id === 'romm' ? ROMM.conn === 'connected' : sst(id).conn === 'ok'; }
function rWelcome(imported) {
  S.layers.forEach(function (l) { l.el.remove(); }); S.layers = [];
  S.startup = true; S.pages = []; S.selMode = false;
  var W = { step: 1, lib: imported ? '/home/sam/Games/GRID' : '', imported: imported };
  push({
    title: 'Welcome', W: W,
    render: function () {
      if (W.step === 1) {
        return '<div class="start"><div class="start-box rwel" data-nav="v"><div class="rstep">Step 1 of 2</div><h1>Welcome to GRID</h1><p class="sub">Choose where GRID keeps your games, emulators and saves. A folder with plenty of free space is best.</p>' +
          (W.imported ? '<p class="note">Filled in from your old GRID settings.</p>' : '') +
          '<div class="start-body" data-nav="v"><div class="field" data-f data-k="w-lib" data-a="wLib" data-al="Choose"><span class="lab">Library folder</span><span class="v' + (W.lib ? '' : ' ph') + '">' + (W.lib ? esc(W.lib) : 'Choose a folder') + '</span></div><p class="note" style="margin:.4rem 1.2rem 0">GRID makes games, emulators and saves folders inside it.</p></div>' +
          '<div class="btns start-foot" data-nav="h"><button class="btn pri" data-f data-k="w-next" data-a="wNext"' + (W.lib ? '' : ' disabled data-why="Choose a library folder first."') + '>Next</button></div></div></div>';
      }
      var any = STOREFRONT_ORDER.some(rConnected);
      var rows = STOREFRONT_ORDER.map(function (id) {
        var on = rConnected(id), s = id === 'romm' ? null : sst(id), name = s ? s.name : 'RomM';
        var sub = on ? (id === 'romm' ? ROMM.user + ' · ' + rHost() : id === 'steam' ? 'Steam account ' + s.account : 'Signed in as ' + s.account) : id === 'romm' ? (ROMM.url ? 'From your old GRID: ' + ROMM.user + ' · ' + rHost() + '. Sign in again.' : 'Your own RomM server: its games, cloud saves and firmware.') : RWSUB[id];
        return '<div class="srow wrow" data-f data-k="w-' + id + '" data-a="wStore" data-id="' + id + '" data-al="' + (on ? 'Change' : 'Connect') + '"><span style="margin-right:.2rem">' + (s ? smark(s, 'lg') : rmark('lg')) + '</span><div class="lab"><div class="t">' + esc(name) + '</div><div class="s">' + esc(sub) + '</div></div><div class="val">' + (on ? '<span class="st ok">Connected</span>' : '<span class="st">Not connected</span>') + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>';
      }).join('');
      return '<div class="start"><div class="start-box wide rwel" data-nav="v"><div class="rstep">Step 2 of 2</div><h1>Connect your games</h1><p class="sub">Optional. Each one you connect gets a group in your Library. You can connect more later in Settings › Game stores.</p>' +
        '<div class="start-body" data-nav="v">' + rows + '</div><div class="btns start-foot" data-nav="h"><button class="btn pri" data-f data-k="w-go" data-a="wDone">' + (any ? 'Continue' : 'Skip for now') + '</button></div></div></div>';
    },
    hints: function () { return W.step === 2 ? [['B', 'Back'], ['MENU', 'Menu']] : [['MENU', 'Menu']]; },
    menu: function () { rShortMenu(); }
  });
  if (imported) setTimeout(function () { openLayer({ html: '<div class="dialog" role="alertdialog"><h2>Settings imported from your old GRID</h2><p>Your library folder, RomM server and username are filled in. Sign in to RomM again: GRID doesn\'t copy tokens or passwords.</p><div class="btns" data-nav="h"><button class="btn pri" data-f data-autofocus data-cl="1">OK</button></div></div>' }); }, 400);
}
function rShortMenu() {
  var d = new Date(), clock = ('0' + d.getHours()).slice(-2) + ':' + ('0' + d.getMinutes()).slice(-2);
  var items = [['Switch to desktop mode', toDesktop], ['Quit GRID', quitBox]];
  openLayer({ cls: 'side', gi: items, hints: function () { return [['A', 'Select'], ['B', 'Close']]; }, html: '<aside class="guide" data-nav="v"><div class="gh"><b>' + clock + '</b></div>' + items.map(function (it, i) { return '<button class="opt" data-f data-k="gm' + i + '" data-gi="' + i + '">' + it[0] + '</button>'; }).join('') + '</aside>' });
}
ACT.wLib = function () { var W = rWelTop().W; openBrowser({ title: 'Library folder', start: '/home/sam/Games', onPick: function (p) { W.lib = p; renderStage(); restoreFocus({ _k: 'w-next' }); } }); };
ACT.wNext = function () { var W = rWelTop().W; CFG.lib = W.lib; W.step = 2; renderStage(); restoreFocus({ _k: 'w-amazon' }); };
ACT.wStore = function (el) {
  var id = el.dataset.id, on = rConnected(id);
  if (id === 'romm') { if (on) openSheet({ title: 'RomM', sub: ROMM.user + ' · ' + rHost(), items: [{ label: 'Edit sign-in…', fn: function () { rSignInPage('edit', 'welcome'); } }, { label: 'Disconnect RomM', danger: 1, fn: function () { ROMM.conn = 'not_connected'; renderStage(); } }] }); else rSignInPage('connect', 'welcome'); return; }
  var s = sst(id);
  if (on) { openSheet({ title: s.name, sub: id === 'steam' ? 'Steam account ' + s.account : 'Signed in as ' + s.account, items: [{ label: 'Disconnect ' + s.name, danger: 1, fn: function () { s.conn = 'off'; renderStage(); } }] }); return; }
  if (id === 'steam') sStorePage(s); else sLogin(s);
};
ACT.wDone = function () {
  if (ROMM.conn === 'connected' && !ROMM.setupDone) { S.setupReturn = 'welcome'; S.pages = []; push(rChoosePage()); return; }
  S.startup = false; openStart();
};

/* ---- Choose your systems, after a RomM connect (from the welcome or the RomM page) ----
   Rows: the systems with games on RomM, plus Windows. From the RomM page, Skip and
   Install return to Settings › Game stores. */
function rChoosePage() {
  var list = SYS.filter(function (s) { return s.proton || G.some(function (g) { return g.p === s.id; }); });
  var pick = {}; list.forEach(function (s) { pick[s.id] = { on: false, emu: recFor(s.id) }; });
  return {
    title: 'Choose your systems', pick: pick,
    render: function () {
      var n = Object.keys(pick).filter(function (k) { return pick[k].on; }).length;
      return '<div class="start"><div class="start-box wide"><h1>Choose your systems</h1><p class="sub">' + (S.setupReturn === 'settings' ? 'RomM is connected. ' : '') + 'Tick the systems you play. GRID installs the recommended emulator for each; ' + gl('X') + ' changes it. You can change this later in Settings › Systems.</p><div class="start-body" data-nav="v">' +
        list.map(function (s) { var p = pick[s.id], c = rOnRomm(s.id).length; return '<div class="srow" data-f data-k="ch-' + s.id + '" data-a="chTick" data-id="' + s.id + '" data-al="' + (p.on ? 'Untick' : 'Tick') + '" data-xl="Change emulator" data-xa="chEmu"><span class="sw' + (p.on ? ' on' : '') + '"></span><div class="lab"><div class="t">' + s.name + '</div><div class="s">' + (s.proton ? 'Proton for Windows games' : plural(c, 'game') + ' on RomM') + '</div></div><div class="val">' + esc(p.emu || 'None') + '</div></div>'; }).join('') +
        '</div><div class="btns start-foot" data-nav="h"><button class="btn pri" data-f data-k="ch-go" data-a="chGo"' + (n ? '' : ' disabled data-why="Tick a system first, or skip for now."') + '>' + (n ? 'Install ' + plural(n, 'emulator') : 'Install emulators') + '</button><button class="btn" data-f data-k="ch-skip" data-a="chSkip">' + gl('MENU') + 'Skip for now</button></div></div></div>';
    },
    hints: function () { return [['MENU', 'Skip for now']]; },
    menu: function () { ACT.chSkip(); }
  };
}
function rToStores() { S.startup = false; S.pages = []; S.tab = 'settings'; S.setPane = 'stores'; TABS.settings._k = 'sto-romm'; rJump(); }
var tvrChGo = ACT.chGo, tvrChSkip = ACT.chSkip;
ACT.chGo = function () {
  var ret = S.setupReturn; S.setupReturn = null; ROMM.setupDone = true;
  if (ret !== 'settings') return tvrChGo();
  var P = S.pages[S.pages.length - 1], names = [];
  Object.keys(P.pick).forEach(function (k) { var e = P.pick[k].emu; if (P.pick[k].on && e && names.indexOf(e) < 0) names.push(e); });
  rToStores(); setupCard(names);
};
ACT.chSkip = function () { var ret = S.setupReturn; S.setupReturn = null; ROMM.setupDone = true; if (ret === 'settings') rToStores(); else tvrChSkip(); };

/* ---- Settings › Systems: a general emulation setup tool, with or without RomM ----
   Your systems: those with an emulator, installed games, or (with RomM) games on RomM.
   More systems: what GRID's own platform list can set up, on its own page.
   Install recommended counts your systems only. */
SYS.splice(SYS.length - 1, 0, { id: 'PS3', name: 'PlayStation 3', emu: 'RPCS3', ver: '0.0.34', status: 'Firmware needed' }, { id: 'SWITCH', name: 'Nintendo Switch', emu: 'Eden', ver: '0.0.3', status: 'Firmware needed' });
EMUS.PS3 = [['RPCS3', 'Installed', 1]]; EMUS.SWITCH = [['Eden', 'Installed', 1]];
[['DC', 'Dreamcast', 'Redream'], ['GBA', 'Game Boy Advance', 'RetroArch · mGBA'], ['NDS', 'Nintendo DS', 'RetroArch · melonDS DS'], ['PSP', 'PlayStation Portable', 'PPSSPP'], ['GEN', 'Sega Genesis', 'RetroArch · Genesis Plus GX'], ['XBOX', 'Xbox', 'Xemu']].forEach(function (c) {
  SYS.push({ id: c[0], name: c[1], emu: null, rec: c[2], status: 'No emulator', cat: 1 }); EMUS[c[0]] = [[c[2], 'Download', 1]];
});
function rGames(s) { var L = G.filter(function (g) { return g.p === s.id; }); return { all: rOnRomm(s.id).length, inst: L.filter(function (g) { return g.inst; }).length }; }
function rYours(s) { var n = rGames(s); return !!(s.emu || s.proton || n.inst || (rOn() && n.all) || s.status === 'Installing…'); }
function rSysStatus(s) { if (!s.emu) return s.status === 'Installing…' ? s.status : 'No emulator'; if (s.upd) return 'Update available'; if (rFwMissing(s)) return 'Firmware needed'; return 'Ready'; }
function rSysRow(s, k) {
  var st = s.status === 'Ready' ? '<span class="st ok">Ready</span>' : '<span class="st warn">' + s.status + '</span>', n = rGames(s);
  var count = s.proton ? '' : rOn() && n.all ? ' · ' + n.all + ' on RomM, ' + n.inst + ' installed' : n.inst ? ' · ' + n.inst + ' installed' : '';
  var sub = s.emu ? s.emu + (s.core ? ' · ' + s.core : '') + count : 'No emulator yet. ' + s.rec + ' is recommended.' + count;
  return '<div class="srow" data-f data-k="' + k + s.id + '" data-a="system" data-id="' + s.id + '" data-al="Open" data-xl="' + (s.emu ? 'Change emulator' : 'Install ' + s.rec) + '" data-xa="' + (s.emu ? 'pickEmu' : 'installEmu') + '"><div class="lab"><div class="t">' + s.name + '</div><div class="s">' + esc(sub) + '</div></div><div class="val">' + st + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>';
}
systemsPane = function () {
  SYS.forEach(function (s) { if (!s.proton) s.status = rSysStatus(s); });
  var mine = SYS.filter(rYours), more = SYS.filter(function (s) { return !rYours(s); });
  var miss = mine.filter(function (s) { return !s.emu && !s.proton; }), todo = miss.filter(function (s) { return s.status !== 'Installing…'; });
  var sorted = miss.concat(mine.filter(function (s) { return s.emu && !s.proton; })).concat(mine.filter(function (s) { return s.proton; }));
  return '<div class="pane-h"><h1>Systems</h1>' + (todo.length ? '<button class="btn pri" data-f data-k="sy-rec" data-a="installRec">Install recommended (' + todo.length + ')</button>' : miss.length ? '<button class="btn pri" data-f data-k="sy-rec" disabled>Installing ' + plural(miss.length, 'emulator') + '…</button>' : '') + '<button class="btn" data-f data-k="sy-chk" data-a="checkUpd">Check for updates</button><button class="btn" data-f data-k="sy-add" data-a="addEmu">Add emulator</button></div>' +
    (rOn() ? '' : '<p class="note" style="margin:0 1.2rem 1rem">Systems works without RomM. With RomM connected, its platforms join this list.</p>') +
    '<div class="sgroup">Your systems</div>' + sorted.map(function (s) { return rSysRow(s, 'sy-'); }).join('') +
    (more.length ? '<div class="srow act" data-f data-k="sy-more" data-a="rMoreSys" data-al="Open"><div class="lab"><div class="t">More systems</div><div class="s">Systems GRID can set up that you have no games for yet</div></div><div class="val">' + more.length + ' <span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>' : '') +
    '<div class="sgroup">Other emulators</div><p class="note" style="margin-left:1.2rem">No other emulators</p>';
};
ACT.installRec = function () { SYS.filter(function (s) { return rYours(s) && !s.emu && !s.proton && s.status !== 'Installing…'; }).forEach(function (s) { installEmu(s); }); };
ACT.rMoreSys = function () {
  push({ title: 'More systems', render: function () {
    var more = SYS.filter(function (s) { return !rYours(s); });
    return '<div class="page"><div class="page-in" data-nav="v"><h1>More systems</h1><p class="sub">Systems GRID can set up from its own list. Set one up now, or when you have games for it.</p>' + (more.length ? more.map(function (s) { return rSysRow(s, 'ms-'); }).join('') : '<p class="note">Every system GRID knows is set up.</p>') + '</div></div>';
  } });
};
/* The System page: the emulator rows as before, then the firmware block. */
systemPage = function (s) {
  return {
    title: s.name, render: function () {
      var rows = [], html = '';
      var add = function (r) {
        rows.push(r); var h = srow(r, 'sp' + rows.length);
        if (r.off) h = h.replace('<div class="srow ', '<div class="srow off ').replace(' data-f ', ' data-f data-why="' + esc(r.off) + '" ');
        html += r.val ? h.replace('<div class="val">', '<div class="val">' + esc(r.val) + ' ') : h;
      };
      s.status = s.proton ? s.status : rSysStatus(s);
      if (s.proton) {
        add({ type: 'action', label: 'Default Proton version', sub: 'Runs Windows games on Linux', val: s.emu, fn: function () { pickEmu(s); } });
        add({ type: 'info', label: 'Proton launcher', value: 'Installed by GRID · 1.2.6' });
        add({ type: 'action', label: 'Reinstall umu-launcher', fn: function () { toast('Installing umu-launcher…'); } });
      } else if (!s.emu) {
        add({ type: 'action', label: 'Install ' + s.rec + ' (recommended)', fn: function () { installEmu(s); } });
        add({ type: 'action', label: 'Choose another emulator', fn: function () { pickEmu(s); } });
      } else {
        add({ type: 'action', label: 'Emulator', val: s.emu, sub: 'Installed and Download groups, like the desktop picker', fn: function () { pickEmu(s); } });
        add({ type: 'info', label: 'Version', value: s.ver + (s.upd ? '' : ' · up to date') });
        if (s.upd) add({ type: 'action', label: 'Update to ' + s.upd, fn: function () { toast('Updating ' + s.emu + '…'); s.ver = s.upd; s.upd = null; renderStage(); } });
        if (s.core) add({ type: 'action', label: 'RetroArch core', val: s.core, fn: function () { openSheet({ title: 'Core for ' + s.name, items: [s.core, 'Parallel-N64', 'Mupen64Plus'].map(function (c) { return { label: c, checked: c === s.core, fn: function () { s.core = c; renderStage(); } }; }) }); } });
      }
      var f = FW[s.id];
      if (f && !s.proton) {
        html += '<div class="sgroup">Firmware</div>';
        f.files.forEach(function (x) {
          var v = x.st === 'romm' ? 'Installed' : x.st === 'grid' ? 'Installed' : x.st === 'nohash' ? 'Installed · not verified' : x.req ? 'Missing · required' : 'Missing';
          var sub = x.nohash ? 'Different on every console, so GRID can\'t check it' : x.size + (x.req ? ' · required' : ' · optional');
          if (x.st === 'romm') sub += ' · checked against RomM'; if (x.st === 'grid') sub += ' · checked by GRID';
          add({ type: 'info', label: x.n, sub: sub, value: v });
        });
        html += rFwNote(s, f); rFwRows(s, f).forEach(add);
      }
      if (s.emu && !s.proton) {
        add({ type: 'action', label: 'Open ' + s.emu, sub: 'Opens the emulator on its own to change its settings, over TV mode', fn: function () { toast('Opening ' + s.emu + '…'); } });
        add({ type: 'action', label: 'Emulator settings', sub: 'Program, launch options, cloud save folders', fn: function () { push(emuForm(s.emu)); } });
        add({ type: 'action', label: 'Remove ' + s.emu, danger: 1, fn: function () { confirmBox({ title: 'Remove ' + s.emu + '?', body: 'Games and saves stay. ' + s.name + ' games can\'t start until another emulator is picked.', ok: 'Remove', danger: 1, fn: function () { toast(s.emu + ' removed'); s.emu = null; s.status = 'No emulator'; s.rec = recFor(s.id); pop(); } }); } });
      }
      this._rows = rows;
      return '<div class="page"><div class="page-in" data-nav="v"><h1>' + s.name + '</h1><p class="sub">' + (s.emu ? s.emu + ' · ' : '') + s.status + '</p>' + html + '</div></div>';
    },
    after: function () { var pi = document.querySelector('.page-in'); pi._rows = this._rows; }
  };
};
/* The firmware actions are rows, like every action on a TV page. One that can't run
   now is dimmed: A on it says why, and the hint bar shows no A. */
function rFwNote(s, f) {
  var note = !s.emu ? 'Install an emulator for ' + s.name + ' first. GRID puts firmware in its folders.' : f.pup ? 'RPCS3 installs the PlayStation 3 system update file, PS3UPDAT.PUP. GRID checks it first.' : f.files.some(function (x) { return x.nohash; }) ? 'GRID checks every file it knows by size and MD5. Key files are different on every console, so they install as not verified.' : 'GRID checks each file\'s size and MD5 before it goes into ' + s.emu + '\'s folders.';
  return '<p class="note" style="margin:.4rem 1.2rem .6rem">' + esc(note) + '</p>';
}
function rFwRows(s, f) {
  var onServer = G.some(function (g) { return g.p === s.id; }), allIn = f.files.every(function (x) { return x.st; });
  var noEmu = !s.emu ? 'Install an emulator for ' + s.name + ' first. GRID puts firmware in its folders.' : '';
  var left = f.files.filter(function (x) { return !x.st; }).length;
  var rSub = 'Downloads ' + plural(left, 'file') + ' from ' + rHost() + ' and checks each one', rWhy = '';
  if (noEmu) { rWhy = noEmu; rSub = 'Needs an emulator first'; }
  else if (!rOn()) { rWhy = 'Install from RomM needs RomM. ' + rFix(); rSub = 'Needs RomM'; }
  else if (!onServer) { rWhy = s.name + ' isn\'t on your RomM server.'; rSub = 'Not on your RomM'; }
  else if (!f.romm) { rWhy = 'Your RomM has no ' + s.name + ' firmware.'; rSub = 'Your RomM has no firmware for it'; }
  else if (allIn) { rWhy = 'Every ' + s.name + ' firmware file is installed.'; rSub = 'Every file is installed'; }
  var fire = function (a) { return function () { ACT[a]({ dataset: { id: s.id } }); }; };
  var why = function (w) { return function () { toast(w, { err: 1 }); }; };
  return [
    { type: 'action', label: 'Install from RomM', sub: rSub, off: rWhy, fn: rWhy ? why(rWhy) : fire('rFwRomm') },
    f.pup ? { type: 'action', label: 'Locate PS3UPDAT.PUP…', sub: noEmu ? 'Needs an emulator first' : 'Pick the update file on this computer or a drive', off: noEmu, fn: noEmu ? why(noEmu) : fire('rFwPup') }
      : { type: 'action', label: 'Locate files…', sub: noEmu ? 'Needs an emulator first' : 'Pick files on this computer or a drive. GRID installs the ones it knows.', off: noEmu, fn: noEmu ? why(noEmu) : fire('rFwLocate') }
  ];
}
var tvrHintsNow = hintsNow;
hintsNow = function () { var h = tvrHintsNow(); return cur && cur.classList.contains('off') && cur.dataset.why ? h.filter(function (x) { return x[0] !== 'A'; }) : h; };
ACT.rFwRomm = function (el) {
  var s = sysOf(el.dataset.id), f = FW[s.id];
  toast('Installing ' + s.name + ' firmware from RomM for ' + s.emu + '…');
  setTimeout(function () { var n = 0; f.files.forEach(function (x) { if (!x.st) { x.st = 'romm'; n++; } }); s.status = rSysStatus(s); toast(s.name + ' firmware installed for ' + s.emu, { s: plural(n, 'file') + ' · checked against RomM' }); if (S.pages.length) renderStage(); }, 1500);
};

/* ---- Locate files: the TV file browser in file mode ----
   A opens a folder or picks a file (a tick); Y checks the whole folder shown (three
   levels deep); Start checks the picked files. B goes back. */
FS['/run/media/USB-1TB'] = ['GRID', 'bios'];
FS['/run/media/USB-1TB/bios'] = ['Saturn', 'BIOS pack.zip', 'disc image.iso', 'PS3UPDAT.PUP', 'prod.keys', 'readme.txt', 'scph1001.bin', 'scph5501 (1).bin', 'scph5502.bin', 'title.keys'];
FS['/run/media/USB-1TB/bios/Saturn'] = ['saturn_bios.bin', 'sega_101.bin'];
FS['/home/sam/Downloads'] = FS['/home/sam/Downloads'].concat(['PS3UPDAT.PUP', 'scph39001.bin']);
var FSIZE = { 'BIOS pack.zip': '1.6 MB', 'disc image.iso': '4.4 GB', 'PS3UPDAT.PUP': '198 MB', 'prod.keys': '8 KB', 'title.keys': '2 KB', 'readme.txt': '1 KB', 'scph1001.bin': '512 KB', 'scph5501 (1).bin': '512 KB', 'scph5502.bin': '512 KB', 'saturn_bios.bin': '512 KB', 'sega_101.bin': '512 KB', 'scph39001.bin': '4 MB' };
var FINFO = { 'scph5501 (1).bin': { as: 'scph5501.bin' }, 'scph5502.bin': { as: 'scph5502.bin' }, 'scph1001.bin': { bad: 'scph1001.bin' }, 'BIOS pack.zip': { zip: ['scph5500.bin'], zipOther: 3 }, 'prod.keys': { nohash: 'prod.keys' }, 'title.keys': { nohash: 'title.keys' }, 'sega_101.bin': { as: 'sega_101.bin' }, 'saturn_bios.bin': { as: 'mpr-17933.bin' }, 'scph39001.bin': { as: 'scph39001.bin' }, 'disc image.iso': { big: 1 } };
function rBase(p) { return p.split('/').pop(); }
function rBrowser(o) {
  var path = o.start, picked = [];
  push({
    title: o.title, render: function () {
      var kids = FS[path] || [];
      this._main = picked.length ? 1 : null; this._why = picked.length ? null : 'x';
      return '<div class="page"><div class="page-in"><h1>' + esc(o.title) + '</h1><p class="sub">' + esc(o.sub) + '</p><div class="path">' + esc(path) + '</div>' +
        (o.single ? '' : '<p class="note rpicked">' + (picked.length ? plural(picked.length, 'file') + ' picked: ' + esc(picked.map(rBase).join(', ')) : 'No files picked. Pick files with ' + gl('A') + ', or check this whole folder with ' + gl('Y') + '.') + '</p>') + '<div data-nav="v">' +
        (path !== '/' ? '<div class="lrow" data-f data-k="up" data-a="rfGo" data-p=".." data-dir="1" data-al="Up">' + gIcon('arrowLeft', 20) + '<div class="grow t">Up one folder</div></div>' : '') +
        kids.map(function (k) {
          var p = (path === '/' ? '' : path) + '/' + k, dir = !!FS[p], on = picked.indexOf(p) >= 0;
          return '<div class="lrow" data-f data-k="rf-' + esc(k) + '" data-a="rfGo" data-p="' + esc(p) + '" data-dir="' + (dir ? 1 : '') + '" data-al="' + (dir ? 'Open' : o.single ? 'Choose' : on ? 'Unpick' : 'Pick') + '">' +
            (dir ? '<span class="rck dir"></span>' : o.single ? '<span class="rck dir"></span>' : '<span class="rck' + (on ? ' on' : '') + '">' + (on ? gIcon('check', 16) : '') + '</span>') + '<div class="grow t">' + esc(k) + '</div>' + (dir ? gIcon('chevronRight', 18) : '<span class="muted">' + (FSIZE[k] || '') + '</span>') + '</div>';
        }).join('') + '</div></div></div>';
    },
    hints: function () { return o.single ? [] : [['Y', 'Check this folder']].concat(picked.length ? [['MENU', 'Check ' + plural(picked.length, 'file')]] : []); },
    y: function () { if (!o.single) { pop(); o.onDone({ folder: path }); } },
    menu: function () { if (o.single) return false; if (picked.length) { pop(); o.onDone({ files: picked.slice() }); return; } toast('Pick files first, or press ' + glyphText('Y') + ' to check this folder.', { err: 1 }); },
    go: function (p, dir) {
      if (dir) { path = p === '..' ? (path.split('/').slice(0, -1).join('/') || '/') : p; this._k = null; renderStage(); restoreFocus(); return; }
      if (o.single) { pop(); o.onPick(p); return; }
      var i = picked.indexOf(p); if (i >= 0) picked.splice(i, 1); else picked.push(p);
      renderStage(); paintHints();
    }
  });
}
ACT.rfGo = function (el) { S.pages[S.pages.length - 1].go(el.dataset.p, !!el.dataset.dir); };
function rWalk(dir, depth, out) { (FS[dir] || []).forEach(function (k) { var p = dir + '/' + k; if (FS[p]) { if (depth < 3) rWalk(p, depth + 1, out); } else out.push(p); }); return out; }
ACT.rFwLocate = function (el) {
  var s = sysOf(el.dataset.id);
  rBrowser({ title: 'Locate ' + s.name + ' firmware', sub: 'For ' + s.emu + '. GRID checks each file and puts the ones it knows in ' + s.emu + '\'s folders.', start: '/run/media/USB-1TB/bios', onDone: function (r) { rCheck(s, r); } });
};
/* The check: size and MD5 against the expected set (RomM's list when connected, plus
   GRID's own table). A renamed dump lands under its known name. */
function rCheck(s, r) {
  var f = FW[s.id], exp = function (n) { return f.files.filter(function (x) { return x.n === n; })[0]; };
  var paths = r.files || rWalk(r.folder, 1, []), out = { ok: [], nv: [], no: [], other: 0 };
  paths.forEach(function (p) {
    var b = rBase(p), i = FINFO[b] || {};
    if (i.big) { out.no.push([b, 'Over 2 GB, so GRID didn\'t read it']); return; }
    if (i.zip) { var hit = i.zip.filter(exp); hit.forEach(function (m) { out.ok.push([m, 'From ' + b + ' · size and MD5 match']); }); if (!hit.length && r.folder) out.other++; else if (!hit.length) out.no.push([b, 'Nothing in it is ' + s.name + ' firmware GRID knows']); return; }
    if (i.as && exp(i.as)) { out.ok.push([i.as, (b !== i.as ? 'From ' + b + ' · ' : '') + 'size and MD5 match']); return; }
    if (i.bad && exp(i.bad)) { out.no.push([b, 'This isn\'t the ' + i.bad + ' GRID knows (size or MD5 differs)']); return; }
    if (i.nohash && exp(i.nohash)) { out.nv.push([i.nohash, 'GRID can\'t check this file: it\'s different on every console']); return; }
    if (r.folder) out.other++; else out.no.push([b, 'GRID doesn\'t know this file for ' + s.name]);
  });
  toast('Checking ' + plural(paths.length, 'file') + '…', { s: r.folder ? r.folder : s.name + ' firmware' });
  setTimeout(function () {
    out.ok.forEach(function (x) { exp(x[0]).st = 'grid'; }); out.nv.forEach(function (x) { exp(x[0]).st = 'nohash'; });
    s.status = rSysStatus(s);
    var n = out.ok.length + out.nv.length;
    if (n) toast(plural(n, 'firmware file') + ' installed for ' + s.emu, { s: [out.nv.length ? out.nv.length + ' not verified' : '', out.no.length ? out.no.length + ' refused' : ''].filter(Boolean).join(' · ') });
    else toast('No firmware installed', { err: 1, s: out.no.length ? plural(out.no.length, 'file') + ' refused' : 'Nothing matched ' + s.name + ' firmware' });
    rReport(s, r, paths.length, out);
  }, 900);
}
function rReport(s, r, n, out) {
  push(rowsPage({
    title: 'Firmware check', key: 'lr', h1: function () { return 'Firmware check'; },
    sub: function () { return 'Checked ' + plural(n, 'file') + (r.folder ? ' in ' + esc(r.folder) : ' you picked') + ' for ' + esc(s.name) + ' · ' + esc(s.emu); },
    rows: function () {
      var R = [];
      if (out.ok.length) { R.push({ heading: 'Installed · ' + out.ok.length }); out.ok.forEach(function (x) { R.push({ info: 1, label: esc(x[0]), sub: esc(x[1]), val: '<span class="st ok">Installed</span>' }); }); }
      if (out.nv.length) { R.push({ heading: 'Installed · not verified · ' + out.nv.length }); out.nv.forEach(function (x) { R.push({ info: 1, label: esc(x[0]), sub: esc(x[1]), val: '<span class="st warn">Installed · not verified</span>' }); }); }
      if (out.no.length) { R.push({ heading: 'Refused · ' + out.no.length }); out.no.forEach(function (x) { R.push({ info: 1, label: esc(x[0]), sub: esc(x[1]), val: '<span class="st rno">Refused</span>' }); }); }
      if (out.other) R.push({ note: plural(out.other, 'other file') + ' matched nothing GRID knows for ' + esc(s.name) + '.' });
      if (!out.ok.length && !out.nv.length) R.push({ note: 'Nothing was installed. The ' + esc(s.name) + ' page lists the files ' + esc(s.emu) + ' needs.', warn: 1 });
      R.push({ label: 'Locate more files…', fn: function () { pop(); ACT.rFwLocate({ dataset: { id: s.id } }); } });
      return R;
    },
    main: function () { return { label: 'Done', fn: function () { pop(); } }; }, mainHint: 'Done'
  }));
}
/* PS3: RPCS3 installs the system update file itself (rpcs3 --installfw). */
ACT.rFwPup = function (el) {
  var s = sysOf(el.dataset.id);
  rBrowser({ single: 1, title: 'Locate PS3UPDAT.PUP', sub: 'The PlayStation 3 system update file. RPCS3 installs it.', start: '/home/sam/Downloads', onPick: function (p) {
    var b = rBase(p), refuse = function (t, body) { openLayer({ html: '<div class="dialog" role="alertdialog"><h2>' + t + '</h2><p>' + body + '</p><div class="btns" data-nav="h"><button class="btn" data-f data-ok="1">Choose another file…</button><button class="btn pri" data-f data-autofocus data-cl="1">OK</button></div></div>', ok: function () { ACT.rFwPup(el); } }); };
    if (b !== 'PS3UPDAT.PUP') return refuse('Can\'t use this file', esc(b) + ' isn\'t a PlayStation 3 system update. Choose PS3UPDAT.PUP.');
    toast('Checking PS3UPDAT.PUP…');
    setTimeout(function () {
      if (p.indexOf('/run/media/') === 0) return refuse('Can\'t use this file', 'This isn\'t the PS3UPDAT.PUP GRID knows (size or MD5 differs). Download the update again.');
      toast('Installing the PS3 firmware in RPCS3…', { s: 'This takes a minute' });
      setTimeout(function () { FW.PS3.files[0].st = 'grid'; s.status = rSysStatus(s); toast('PS3 firmware installed in RPCS3', { s: 'PS3UPDAT.PUP · size and MD5 match' }); if (S.pages.length) renderStage(); }, 1600);
    }, 800);
  } });
};

/* ---- Details: what needs RomM says so ---- */
var tvrDetBody = detBody;
detBody = function (g) {
  if (!rOn() && (S.detTab === 'Saves' || S.detTab === 'Achievements')) {
    var sv = S.detTab === 'Saves', c = ROMM.conn;
    var t = sv ? 'Cloud saves need RomM' : 'Achievements need RomM';
    var p = rWhy() + (sv ? ' This game\'s saves stay on this computer and still work; GRID uploads and restores them when RomM is back.' : ' Your progress shows here from RomM; achievements still unlock in the emulator.');
    return rNeed(t, p).replace('data-k="rneed"', 'data-k="rneed-d"');
  }
  return tvrDetBody(g);
};
var tvrDetails = details;
details = function (g, drv) {
  var o = tvrDetails(g, drv), rr = o.render;
  o.render = function () {
    var h = rr.call(this).replace('Link to server game', 'Link to a RomM game').replace('<b>Not linked to a server game.</b> Link it to your server to play.', '<b>Not linked to a RomM game.</b> Link it to play.');
    if (g.offRomm) h = h.replace('Last played <b>', 'From another RomM server (' + esc(g.offRomm) + ') · not on ' + esc(rHost() || 'yours') + '<br>Last played <b>');
    if (!rOn()) h = h.replace(/(data-a="update" data-id="\d+")/, '$1 disabled data-why="Updates need RomM. ' + rFix() + '"').replace(/(data-a="link" data-id="\d+")/, '$1 disabled data-why="Linking needs RomM. ' + rFix() + '"');
    return h;
  };
  return o;
};

/* ---- actions ---- */
ACT.rCard = function () { rCardPage(); };
ACT.rReconnect = function () { rReconnect(); };
ACT.rSignIn = function () { rSignInPage('again', 'settings'); };

/* ---- Menu: no Reconnect, no server line; Discover only while RomM is connected ---- */
var tvrGuide = guideMenu;
guideMenu = function () {
  tvrGuide();
  var L = S.layers[S.layers.length - 1], box = L && L.el.querySelector('.guide'); if (!box) return;
  var head = box.querySelector('.gh .muted'); if (head) head.remove();
  box.querySelectorAll('[data-gi]').forEach(function (b) {
    var it = L.gi[+b.dataset.gi];
    if (it[0] === 'Reconnect') b.remove();
    if (it[0] === 'Discover') { if (!rLive()) b.remove(); else it[1] = function () { S.pages = []; S.tab = 'library'; S.libRail = 'romm:discover'; S.folds.romm = true; S.libQ = ''; TABS.library._k = 'r-romm:discover'; rJump(); }; }
  });
  box.querySelectorAll('[data-gp]').forEach(function (b) {
    var t = L.gp[+b.dataset.gp][0];
    if (/^Show: (first run|token rejected|server unreachable|back online|restart GRID)/.test(t)) b.remove();
    if (t === 'Show: Settings › Stores') b.textContent = 'Show: Settings › Game stores';
  });
  var mine = [
    ['Show: welcome (first run)', function () { rFresh(false); rWelcome(false); }],
    ['Show: welcome after a Python import', function () { rFresh(true); rWelcome(true); }],
    ['Show: a RomM state…', rStateSheet],
    ['Show: next RomM sign-in result…', rNextSheet],
    ['Show: restart GRID (RomM answers)', function () { rRestart(true); }],
    ['Show: restart GRID (RomM doesn\'t answer)', function () { rRestart(false); }],
    ['Show: Settings › Game stores › RomM', function () { rToStores(); rCardPage(); }],
    ['Show: Settings › Systems', function () { S.pages = []; S.tab = 'settings'; S.setPane = 'systems'; TABS.settings._k = 'r-systems'; rJump(); }],
    ['Show: Choose your systems (from the RomM page)', function () { S.setupReturn = 'settings'; S.pages = []; S.startup = true; push(rChoosePage()); }]
  ];
  var after = box.querySelector('.proto');
  mine.reverse().forEach(function (it) { var i = L.gp.push(it) - 1; after.insertAdjacentHTML('afterend', '<button class="opt" data-f data-k="gp' + i + '" data-gp="' + i + '">' + esc(it[0]) + '</button>'); });
  restoreFocus(L);
};
/* ---- prototype: RomM states ---- */
function rFresh(imported) {
  STV.order.forEach(function (id) { var s = sst(id), b = STV_BASE[id]; Object.keys(b).forEach(function (k) { s[k] = JSON.parse(JSON.stringify(b[k])); }); delete s.loading; s.conn = 'off'; if (s.tool) s.tool = 'missing'; });
  sst('steam').key = 'none'; sst('steam').tool = 'ok';
  ROMM.conn = 'not_connected'; ROMM.busy = false; ROMM.startupProbe = false; ROMM.setupDone = false; ROMM.err = '';
  ROMM.url = imported ? 'https://romm.home.lan' : ''; ROMM.user = imported ? 'sam' : '';
  rSetNoGames(true);
}
function rState(c, o) {
  o = o || {};
  rSetNoGames(!!o.none); ROMM.conn = c; ROMM.busy = false; ROMM.startupProbe = false; ROMM.err = c === 'unreachable' ? 'connection refused' : '';
  if (c !== 'not_connected' || !o.none) { ROMM.url = ROMM.url || 'https://romm.home.lan'; ROMM.user = ROMM.user || 'sam'; }
  if (o.none) { ROMM.url = ''; ROMM.user = ''; ROMM.setupDone = false; } else ROMM.setupDone = true;
  if (c === 'unreachable') toast('Can\'t reach RomM', { err: 1, s: rHost() + ' didn\'t answer. Installed games still play.', act: ['Retry', rReconnect] });
  if (c === 'expired') rExpiredToast();
  rApply();
}
function rStateSheet() {
  openSheet({ title: 'Show a RomM state', sub: 'Prototype only', items: [
    { label: 'Connected', checked: rOn(), fn: function () { rState('connected'); } },
    { label: 'Can\'t reach RomM', checked: ROMM.conn === 'unreachable', fn: function () { rState('unreachable'); } },
    { label: 'Sign-in expired (a 401 mid-session)', checked: ROMM.conn === 'expired', fn: function () { rState('expired'); } },
    { label: 'Not connected, with installed RomM games', checked: ROMM.conn === 'not_connected' && !rSaved, fn: function () { rState('not_connected'); } },
    { label: 'Not connected, no RomM games (no group)', checked: ROMM.conn === 'not_connected' && !!rSaved, fn: function () { rState('not_connected', { none: 1 }); } }] });
}
function rNextSheet() {
  openSheet({ title: 'The next RomM sign-in or Reconnect', sub: 'Prototype only', items: [['ok', 'Succeeds'], ['rejected', 'RomM rejects the token'], ['unreachable', 'Can\'t reach RomM']].map(function (x) { return { label: x[1], checked: ROMM.next === x[0], fn: function () { ROMM.next = x[0]; } }; }) });
}
function rRestart(ok) {
  rSetNoGames(false); ROMM.conn = 'connected'; ROMM.url = ROMM.url || 'https://romm.home.lan'; ROMM.user = ROMM.user || 'sam'; ROMM.startupProbe = true; ROMM.busy = true; ROMM.setupDone = true;
  openStart();
  toast('GRID started', { s: 'Opened on ' + (S.libRail === 'romm:discover' ? 'Library › RomM › Discover' : S.libRail.indexOf('drv:') === 0 ? 'Library › ' + driveOf(S.libRail.slice(4)).name : 'Library › All games') });
  setTimeout(function () {
    ROMM.startupProbe = false; ROMM.busy = false;
    if (ok) ROMM.conn = 'connected';
    else { ROMM.conn = 'unreachable'; ROMM.err = 'connection refused'; toast('Can\'t reach RomM', { err: 1, s: rHost() + ' didn\'t answer. Installed games still play.', act: ['Retry', rReconnect] }); }
    rApply();
  }, 2400);
}
/* The card opens on Library › RomM › Discover (Automatic, RomM connected), with the
   store jobs of the Game stores card running in Downloads. */
function rBoot() { CFG.startOn = 'auto'; openStart(); }
