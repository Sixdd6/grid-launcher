/* ==== RomM as an optional storefront (docs/romm-storefront-plan.md, step D1).

   RomM becomes one storefront beside Amazon, Epic, GOG and Steam:
   - No Server view, no top-bar chip or server menu, no offline mode. The
     views are Library, Downloads and Settings (Ctrl+1..3).
   - The Library rail: All games, Recent, Updates; Backup drives; then the
     storefront groups by name: Amazon, Epic, GOG, RomM, Steam. RomM's group
     is Installed, Discover and one row per platform while connected, and
     Installed only (with the "!" or "off" flag) otherwise.
   - Settings › Game stores (renamed from Stores) has a RomM card; the
     Connection pane is gone. The welcome replaces the Connect screen.
   - Settings › Systems works without RomM: catalog systems, Install from
     RomM or Locate files for firmware.
   In this prototype the Discover and platform screens are the old Server
   view's DOM (S.view 'server'), shown as part of the Library: the pill stays
   on Library and the rail is the Library rail. ==== */
var RM_ORDER = ['amazon', 'epic', 'gog', 'romm', 'steam'];
var RM = {
  conn: 'ok', url: 'https://romm.home.lan', user: 'player', kind: 'token', err: '', busy: false,
  proto: 'ok', confirm: false, dlg: null, next: 0, setupNeeded: false, snap: null, updSnap: null,
  fwSt: { PS: { 'scph5500.bin': 'ok', 'scph5501.bin': 'ok' } }, fwBusy: {}, openOn: 'automatic'
};
var RM_MARK = { name: 'RomM', mono: 'R', hue: 'hsl(258 50% 46%)' };
function rmHost() { return RM.url.replace(/^https?:\/\//, '').replace(/\/.*$/, ''); }
function rmOn() { return RM.conn === 'ok'; }
function rmInst() { return G.filter(function (g) { return g.installed; }); }
function rmInRail() { return RM.conn !== 'off' || rmInst().length > 0; }
function rmMark(cls) { return stMark(RM_MARK, cls); }
function rmSetConn(c) {
  RM.conn = c;
  /* Disconnect clears updates: RomM games show no UPDATE while RomM is off. */
  if (c === 'off' && !RM.updSnap) { RM.updSnap = G.filter(function (g) { return g.update; }).map(function (g) { return g.id; }); G.forEach(function (g) { g.update = false; }); }
  if (c !== 'off' && RM.updSnap) { RM.updSnap.forEach(function (id) { G[id].update = true; }); RM.updSnap = null; }
}

/* ---- data: installed games the server doesn't have, PS3 games, catalog systems ---- */
PLAT.PS3 = 'PlayStation 3'; EMU.PS3 = 'RPCS3'; BK_SIZE.PS3 = 14;
[['Ember Relay', 'hsl(12 50% 34%)', 'PS3', '2011-04-12', '2026-09-02'], ['Tin Choir', 'hsl(190 30% 30%)', 'PS3', '2013-10-01', '2026-08-11'],
 ['Hidden Kiln', 'hsl(30 35% 30%)', 'PS', '1999-02-01', '', 'unlinked'], ['Sky Lantern', 'hsl(215 45% 40%)', 'SNES', '1994-06-01', '', 'other']].forEach(function (x) {
  G.push({ id: G.length, t: x[0], c: x[1], p: x[2], rel: x[3], added: x[4] || '2000-01-01', installed: !!x[5], update: false, missing: false, cloud: false, local: x[5] || '' });
});
BK_LOCAL[G.length - 2] = { layout: 'file', ver: 'v1.0' }; BK_LOCAL[G.length - 1] = { layout: 'file', ver: 'v1.0' };
/* Catalog systems GRID ships itself (platforms.json): shown with or without RomM. */
SY.name.PS3 = 'PlayStation 3'; SY.name.SWITCH = 'Nintendo Switch'; SY.name.GBA = 'Game Boy Advance'; SY.name.PSP = 'PlayStation Portable'; SY.name.DC = 'Dreamcast';
SY.games.PS3 = 2; SY.games.SWITCH = 0; SY.games.GBA = 0; SY.games.PSP = 0; SY.games.DC = 0;
SY.order.push('PS3', 'SWITCH', 'GBA', 'PSP', 'DC');
SY.pick.PS3 = 'RPCS3'; SY.pick.SWITCH = 'Eden'; SY.pick.GBA = ''; SY.pick.PSP = ''; SY.pick.DC = '';
SY.rec.PS3 = 'RPCS3'; SY.rec.SWITCH = 'Eden'; SY.rec.GBA = 'RetroArch'; SY.rec.PSP = 'PPSSPP'; SY.rec.DC = 'RetroArch';
SY.can.RPCS3 = ['PS3']; SY.can.Eden = ['SWITCH']; SY.can.PPSSPP = ['PSP']; SY.can.RetroArch.push('GBA', 'DC');
SY.ver.RPCS3 = 'build-0.0.37'; SY.ver.Eden = 'v0.0.3'; SY.ver.PPSSPP = 'v1.19.3';
SY.core.GBA = 'mGBA'; SY.core.DC = 'Flycast';
SY.installed.push('RPCS3', 'Eden'); EMU.SWITCH = 'Eden';
/* Server-only platforms: on RomM, with no catalog entry or emulator. */
var RM_SERVER_ONLY = ['JAG'];
/* What each system's emulator needs (firmware-known.json, merged with the
   server's list while connected): [name, required, what it is, no checksum]. */
var RM_FW = {
  PS: { romm: true, files: [['scph5500.bin', 1, 'Japan'], ['scph5501.bin', 1, 'North America'], ['scph5502.bin', 0, 'Europe']] },
  SAT: { romm: true, files: [['sega_101.bin', 1, 'Japan'], ['mpr-17933.bin', 1, 'North America and Europe']] },
  PS2: { romm: true, files: [['scph39001.bin', 1, 'North America']] },
  PS3: { romm: true, pup: true, files: [['PS3UPDAT.PUP', 1, 'PS3 system software 4.92']] },
  SWITCH: { romm: false, files: [['prod.keys', 1, 'Your console’s keys', 1], ['title.keys', 0, 'Your console’s title keys', 1]] },
  GBA: { romm: false, files: [['gba_bios.bin', 0, 'mGBA plays without it']] },
  DC: { romm: false, files: [['dc_boot.bin', 1, 'Boot ROM'], ['dc_flash.bin', 1, 'Flash memory']] }
};
/* The stand-in file chooser's folder for Locate files: [file, size, result, installs as]. */
var RM_PICK = {
  PS: [['scph5501 (1).bin', '512 KB', 'match', 'scph5501.bin'], ['scph5502.bin', '512 KB', 'bad'], ['bios-pack.zip', '1.1 MB', 'zip', 'scph5502.bin'], ['scph1001.bin', '512 KB', 'unknown'], ['readme.txt', '2 KB', 'unknown']],
  SAT: [['sega_101.bin', '512 KB', 'match', 'sega_101.bin'], ['Sega Saturn BIOS (US).bin', '512 KB', 'match', 'mpr-17933.bin'], ['saturn-notes.txt', '1 KB', 'unknown']],
  PS2: [['SCPH-39001.bin', '4 MB', 'match', 'scph39001.bin'], ['ps2-bios-old.bin', '4 MB', 'unknown']],
  PS3: [['PS3UPDAT.PUP', '197 MB', 'match', 'PS3UPDAT.PUP'], ['PS3UPDAT (3.55).PUP', '183 MB', 'bad']],
  SWITCH: [['prod.keys', '6 KB', 'nohash', 'prod.keys'], ['title.keys', '1 KB', 'nohash', 'title.keys'], ['Firmware 19.0.1.zip', '312 MB', 'unknown']],
  GBA: [['gba_bios.bin', '16 KB', 'match', 'gba_bios.bin']],
  DC: [['dc_boot.bin', '2 MB', 'match', 'dc_boot.bin'], ['dc_flash.bin', '128 KB', 'match', 'dc_flash.bin']]
};
function rmFwSync() {
  Object.keys(RM_FW).forEach(function (k) {
    var st = RM.fwSt[k] || {};
    SY.fw[k] = RM_FW[k].files.some(function (x) { return x[1] && !st[x[0]]; }) ? 'install' : 'installed';
  });
}
rmFwSync();

/* ---- the top bar: three views, no chip ---- */
var rmOrigPills = renderPills;
renderPills = function () {
  rmOrigPills();
  if (S.view !== 'server') return;
  var b = document.querySelector('[data-view-btn=library]'); if (b) { b.classList.add('active'); b.setAttribute('aria-current', 'page'); }
};
document.addEventListener('click', function (e) {
  if (S.view === 'server' && e.target.closest('[data-view-btn=library]')) e.stopImmediatePropagation();
}, true);

/* ---- selection: Discover and platform rows exist only while connected ---- */
function rmFixSel() {
  var srvOk = rmOn() || (RM.conn === 'checking' && S.platform === 'DISCOVER');
  if (S.view === 'server' && !srvOk) { S.view = 'library'; S.libFilter = rmInRail() ? 'romm:installed' : 'all'; }
  if (S.view === 'server' && S.platform !== 'DISCOVER' && !G.some(function (g) { return g.p === S.platform; })) S.platform = 'DISCOVER';
  if (S.libFilter === 'romm:installed' && !rmInRail()) S.libFilter = 'all';
}
var rmOrigAll = renderAll;
renderAll = function () { rmFixSel(); rmOrigAll(); if (FR.phase === 'welcome') rwRender(); };
function rmGo(k) {
  if (k === 'romm:discover') { S.view = 'server'; S.platform = 'DISCOVER'; }
  else if (k.indexOf('platform:') === 0) { S.view = 'server'; S.platform = k.slice(9); }
  else { S.view = 'library'; S.libFilter = k; }
  renderAll();
}
function rmRailClick(e) { var b = e.target.closest('[data-key]'); if (b) rmGo(b.getAttribute('data-key')); }

/* ---- Library rail: the RomM group, alphabetical among the stores ---- */
/* A platform row's count is RomM's game count only (decision 14). Installed games
   RomM doesn't have still show in the platform's grid but are not counted. */
function rmPlats() {
  return Object.keys(PLAT).filter(function (p) { return G.some(function (g) { return g.p === p; }); })
    .sort(function (a, b) { return PLAT[a].localeCompare(PLAT[b]); });
}
function rmRailHTML() {
  if (!rmInRail()) return '';
  var rows = [['romm:installed', 'Installed', rmInst().length]];
  if (rmOn() || RM.conn === 'checking') rows.push(['romm:discover', 'Discover', null]);
  if (rmOn()) rmPlats().forEach(function (p) { rows.push(['platform:' + p, PLAT[p], G.filter(function (g) { return g.p === p && !g.local; }).length]); });
  var cur = S.view === 'server' ? (S.platform === 'DISCOVER' ? 'romm:discover' : 'platform:' + S.platform) : S.libFilter;
  var flag = RM.conn === 'checking' ? '<span class="st-flag rm-flag-spin" title="Connecting to RomM"><span class="st-spin"></span></span>'
    : RM.conn === 'unreachable' ? '<span class="st-flag" title="Can’t reach RomM">!</span>'
    : RM.conn === 'expired' ? '<span class="st-flag" title="RomM sign-in expired">!</span>'
    : RM.conn === 'off' ? '<span class="st-flag off" title="Not connected">off</span>' : '';
  return '<span class="g-rail-heading st-head">' + rmMark() + 'ROMM' + flag + '</span>' + rows.map(function (r) {
    var sel = r[0] === cur;
    return '<button class="g-rail-item st-row' + (sel ? ' active' : '') + '" data-key="' + r[0] + '"' + (sel ? ' aria-current="page"' : '') + '><span class="g-rail-label">' + esc(r[1]) + '</span>' +
      (r[2] == null ? '' : '<span class="g-rail-count">' + r[2] + '</span>') + '</button>';
  }).join('');
}
var rmOrigStRail = stRailHTML;
stRailHTML = function (sv) {
  var keep = ST.order, h = '';
  RM_ORDER.forEach(function (id) {
    if (id === 'romm') { h += rmRailHTML(); return; }
    ST.order = [id]; h += rmOrigStRail(sv);
  });
  ST.order = keep;
  return h;
};
/* The Discover and platform screens use the Library rail too. */
function rmSyncSrvRail() {
  var sr = $('srv-rail'), lr = $('lib-rail'); if (!sr || !lr) return;
  sr.innerHTML = lr.innerHTML; sr.setAttribute('aria-label', 'Library filters');
  var cur = S.platform === 'DISCOVER' ? 'romm:discover' : 'platform:' + S.platform;
  sr.querySelectorAll('.g-rail-item').forEach(function (b) {
    var on = b.getAttribute('data-key') === cur;
    b.classList.toggle('active', on); if (on) b.setAttribute('aria-current', 'page'); else b.removeAttribute('aria-current');
  });
  sr.onclick = rmRailClick; lr.onclick = rmRailClick;
}

/* ---- Library › RomM › Installed ---- */
function rmBanner() {
  if (RM.conn === 'unreachable') return '<div class="rm-banner" role="status"><span><b>Can’t reach RomM</b> · ' + esc(rmHost()) + ' didn’t answer. Installed games still play.</span><button class="small-btn" data-rm-retry' + (RM.busy ? ' disabled' : '') + '>' + (RM.busy ? 'Retrying…' : 'Retry') + '</button></div>';
  if (RM.conn === 'expired') return '<div class="rm-banner" role="status"><span><b>RomM sign-in expired</b> · Sign in again to browse, install and sync saves. Installed games still play.</span><button class="small-btn primary" data-rm-signin>Sign in again…</button></div>';
  return '';
}
function rmProtoSelect() {
  var o = [['ok', 'Prototype: connected'], ['checking', 'Connecting (at startup)'], ['unreachable', 'Can’t reach RomM'], ['expired', 'Sign-in expired'], ['off', 'Not connected (games installed)'], ['never', 'Never connected (nothing installed)']];
  return '<select class="small-btn bk-proto" data-rm-proto aria-label="Prototype: RomM state">' + o.map(function (x) { return '<option value="' + x[0] + '"' + (RM.proto === x[0] ? ' selected' : '') + '>' + esc(x[1]) + '</option>'; }).join('') + '</select>';
}
function rmRenderInstalled() {
  var list = rmInst(), q = S.q.library.trim(), shown = list.filter(function (g) { return match(g, S.q.library); });
  $('lib-title').innerHTML = 'RomM <span class="st-sub">· Installed</span>';
  var size = list.reduce(function (t, g) { return t + bkLocalSize(g); }, 0);
  $('lib-count').textContent = bkPlural(list.length, 'game') + (list.length ? ' · ' + bkFmt(size) : '') + (RM.url ? ' · from ' + rmHost() : '');
  var c = [];
  if (RM.conn === 'ok') c.push('<button class="g-chip-outline" data-rm-goset>Signed in as ' + esc(RM.user) + '</button>');
  if (RM.conn === 'checking') c.push('<span class="g-chip-outline"><span class="st-spin"></span>Connecting to ' + esc(rmHost()) + '…</span>');
  if (RM.conn === 'off') c.push('<button class="g-chip-outline" data-rm-goset>Not connected · Connect…</button>');
  if (list.length) c.push('<button class="small-btn bk-plat" data-rm-bkall>Backup all ' + list.length + '…</button>');
  var chips = $('lib-chips'); chips.hidden = false; chips.innerHTML = c.join('') + rmProtoSelect();
  var el = $('lib-grid');
  if (!list.length) {
    el.className = 'grid';
    el.innerHTML = rmBanner() + stState({ icon: rmMark('lg'), h: 'No RomM games installed yet',
      p: rmOn() ? 'Install games from Discover or a platform under it.' : 'Install games from RomM once it’s connected again.',
      acts: rmOn() ? '<button class="g-btn" data-rm-go="romm:discover">Open Discover</button>' : '' });
  } else if (!shown.length) {
    el.innerHTML = rmBanner() + '<p class="g-panel empty">No installed RomM games match “' + esc(q) + '”</p>';
  } else el.insertAdjacentHTML('afterbegin', rmBanner());
  gIcons(chips); gIcons(el);
}
var rmOrigLib = renderLibrary;
renderLibrary = function () {
  if (S.libFilter === 'romm:installed' && !rmInRail()) S.libFilter = 'all';
  rmOrigLib();
  if (S.view === 'library' && S.libFilter === 'romm:installed') rmRenderInstalled();
  /* All games and Updates count RomM and store games on every row, not only while they are open. */
  var stI = ST.games.filter(function (g) { return g.inst && stShown(stS(g.s)); });
  var setN = function (key, n) { var c = $('lib-rail').querySelector('[data-key="' + key + '"] .g-rail-count'); if (c) c.textContent = n; };
  setN('all', rmInst().length + stI.length);
  setN('updates', rmInst().filter(function (g) { return g.update; }).length + stI.filter(function (g) { return g.upd; }).length);
  rmSyncSrvRail();
};

/* ---- Library › RomM › Discover and the platform screens ---- */
var rmOrigSrv = renderServer;
renderServer = function () {
  rmOrigSrv();
  if (S.platform === 'DISCOVER' && RM.conn === 'checking') {
    $('discover').innerHTML = stState({ icon: rmMark('lg'), h: 'Connecting to ' + esc(rmHost()) + '…', p: 'Discover and the platforms show when RomM answers. Installed games play now.' }).replace('<h3>', '<h3><span class="st-spin"></span> ');
    DISC.sig = '';
  } else if (S.platform !== 'DISCOVER' && PLAT[S.platform]) {
    /* The count is RomM's games (as the rail row); installed games RomM doesn't have
       are named after it, not counted in it. */
    var all = G.filter(function (g) { return g.p === S.platform; }), loc = all.filter(function (g) { return g.local; }), rm = all.filter(function (g) { return !g.local; });
    $('srv-count').textContent = bkPlural(rm.length, 'game') + ' on RomM · ' + rm.filter(function (g) { return g.installed; }).length + ' installed' + (loc.length ? ' · plus ' + loc.length + ' only on this computer' : '');
  }
  rmSyncSrvRail();
};
/* Installed games the server doesn't have: no rom id (NOT LINKED), or from
   another server or gone from this one (NOT ON ROMM). */
var rmOrigCard = card;
card = function (g, sp) {
  var h = rmOrigCard(g, sp);
  if (!g.local) return h;
  return h.replace('<div class="g-shade">', '<div class="g-tag-stack"><span class="g-tag rm-local">' + (g.local === 'unlinked' ? 'NOT LINKED' : 'NOT ON ROMM') + '</span></div><div class="g-shade">');
};
var rmOrigNew = discNew; discNew = function () { return rmOrigNew().filter(function (g) { return !g.local; }); };
var rmOrigPool = discPool; discPool = function (n) { return rmOrigPool(n).filter(function (g) { return !g.local; }); };

/* ---- needs-RomM words, one place ---- */
function rmNeedsHTML(o) {
  return '<div class="rm-needs" role="note">' + rmMark('lg') + '<div><b>' + o.h + '</b><p>' + o.p + '</p></div>' + (o.act ? '<div class="form-actions">' + o.act + '</div>' : '') + '</div>';
}
/* The action that fixes the RomM state: Connect, Retry or Sign in again. */
function rmFixAct() {
  return RM.conn === 'unreachable' ? '<button class="g-btn secondary" data-rm-retry' + (RM.busy ? ' disabled' : '') + '>' + (RM.busy ? 'Retrying…' : 'Retry') + '</button>'
    : RM.conn === 'expired' ? '<button class="g-btn" data-rm-signin>Sign in again…</button>'
    : '<button class="g-btn secondary" data-rm-goset>Open Game stores</button>';
}
function rmWhy() {
  return RM.conn === 'unreachable' ? 'GRID can’t reach ' + esc(rmHost()) + ' right now.' : RM.conn === 'expired' ? 'Your RomM sign-in expired.' : RM.conn === 'checking' ? 'GRID is still connecting to ' + esc(rmHost()) + '.' : 'RomM isn’t connected.';
}

/* ---- Details: needs-RomM tabs, Update and Link ---- */
var rmOrigDet = renderDetails;
renderDetails = function () {
  rmOrigDet();
  var g = S.detail; if (!g || S.stDetail) return;
  var left = $('d-left'), p = $('d-panel'), first = left.querySelector('.g-meta-line');
  if (g.installed && g.update && !rmOn()) {
    left.querySelectorAll('.g-btn').forEach(function (b) { if (/^Update to/.test(b.textContent)) { b.disabled = true; b.title = 'Updates need RomM'; } });
    if (first) first.insertAdjacentHTML('beforebegin', '<p class="g-meta-line">Updates need RomM</p>');
  }
  if (g.local && first) first.insertAdjacentHTML('beforebegin', g.local === 'unlinked'
    ? '<p class="g-meta-line">Not linked to a RomM game · ' + (rmOn() ? '<button class="linkish" data-rm-link>Link…</button>' : 'Linking needs RomM') + '</p>'
    : '<p class="g-meta-line">From another RomM server (romm.old.lan) · not on ' + esc(rmHost() || 'yours') + '</p>');
  if (S.tab === 'saves' && !rmOn()) p.innerHTML = rmNeedsHTML({ h: 'Cloud saves need RomM', p: rmWhy() + ' This game’s saves stay on this computer and still work; GRID uploads and restores them when RomM is back.', act: rmFixAct() });
  else if (S.tab === 'saves' && g.local === 'unlinked') p.innerHTML = rmNeedsHTML({ h: 'Link this game to sync its saves', p: 'Cloud saves are kept per RomM game. Link ' + esc(g.t) + ' to its game on ' + esc(rmHost()) + ' first.', act: '<button class="g-btn secondary" data-rm-link>Link…</button>' });
  else if (S.tab === 'achievements' && !rmOn()) p.innerHTML = rmNeedsHTML({ h: 'Achievements need RomM', p: rmWhy() + ' Your progress shows here from RomM; achievements still unlock in the emulator.', act: rmFixAct() });
  gIcons($('dialog'));
};

/* ---- Settings: pane list, Cloud saves, RetroAchievements, Appearance ---- */
var rmOrigSet = renderSettings;
renderSettings = function () {
  rmOrigSet();
  var el = $('set-pane'), h2 = el.querySelector('h2');
  if (S.setPane === 'cloud' && !rmOn() && h2) h2.insertAdjacentHTML('afterend', rmNeedsHTML({ h: 'Cloud saves need RomM', p: rmWhy() + ' GRID keeps your emulator saves and states on your RomM server. These settings apply when it’s connected. GOG and Epic games sync with their own store’s cloud.', act: rmFixAct() }));
  if (S.setPane === 'ra' && !rmOn() && h2) h2.insertAdjacentHTML('afterend', rmNeedsHTML({ h: 'Achievement progress needs RomM', p: rmWhy() + ' Details › Achievements shows your progress from RomM. Signing in here still turns on achievements in RetroArch, DuckStation and Dolphin.', act: rmFixAct() }));
  if (S.setPane === 'appearance') {
    var row = el.querySelector('.setting');
    var opts = [['automatic', 'Automatic: Discover when RomM is connected, else Library'], ['discover', 'Discover (when RomM is connected)'], ['library', 'Library']];
    var fallback = RM.openOn !== 'library' && !rmOn() ? '<small>RomM isn’t connected, so GRID opens on All games.</small>' : '';
    var html = '<label class="setting"><span>Open GRID on<small>Here and in TV mode</small></span><span><span class="g-control"><select id="set-openon" data-rm-openon>' +
      opts.map(function (o) { return '<option value="' + o[0] + '"' + (RM.openOn === o[0] ? ' selected' : '') + '>' + o[1] + '</option>'; }).join('') + '</select></span>' + fallback + '</span></label>';
    if (row) row.insertAdjacentHTML('afterend', html);
  }
  gIcons(el);
  if (FR.phase === 'welcome') rwRender();
};

/* ---- Settings › Game stores: the RomM card ---- */
function rmCard() {
  var c = RM.conn, n = rmInst().length, host = rmHost();
  var pill = RM.busy || c === 'checking' ? '<span class="pill-s"><span class="st-spin"></span>Connecting…</span>'
    : c === 'ok' ? '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Connected</span>'
    : c === 'unreachable' ? '<span class="pill-s warn">Can’t reach</span>' : c === 'expired' ? '<span class="pill-s warn">Sign-in expired</span>' : '<span class="pill-s">Not connected</span>';
  var sub = c === 'off' ? (RM.url ? 'Not signed in · ' + esc(host) : 'Your own game server') : 'Signed in as ' + esc(RM.user) + ' · ' + esc(host);
  var head = '<header class="st-store-head">' + rmMark('lg') + '<div class="st-store-id"><b>RomM</b><span class="note">' + sub + '</span></div>' + pill + '</header>';
  var body;
  if (RM.confirm) {
    body = '<div class="bk-confirm"><p><b>Disconnect RomM?</b> GRID deletes your RomM sign-in from the OS keyring and stays open. Installed RomM games stay and still play. Cloud saves, firmware downloads and updates stop.</p>' +
      '<div class="form-actions"><button class="g-btn destructive" data-rm-disc-go>Disconnect</button><button class="g-btn secondary" data-rm-disc-cancel>Cancel</button></div></div>';
  } else if (c === 'off') {
    body = '<p class="note st-lede">Connect your RomM server to browse its games in the Library, install them with an emulator, and sync their saves and firmware.' + (n ? ' The ' + bkPlural(n, 'RomM game') + ' installed here still play.' : '') + '</p>' +
      (RM.url ? '<dl class="kv"><div><dt>Server</dt><dd>' + esc(RM.url) + '</dd></div><div><dt>User</dt><dd>' + esc(RM.user) + '</dd></div><div><dt>Status</dt><dd>Not connected</dd></div></dl>' : '') +
      '<div class="form-actions"><button class="g-btn" data-rm-connect>Connect RomM…</button>' + (RM.url ? '<button class="g-btn secondary" data-rm-open>Open in browser</button>' : '') + '</div>';
  } else {
    var status = RM.busy || c === 'checking' ? 'Connecting to ' + esc(host) + '…' : c === 'ok' ? 'Connected · checked 2 min ago' : c === 'unreachable' ? 'Can’t reach · ' + esc(RM.err || 'no answer') + ' · tried just now' : 'Sign-in expired · RomM answered 401';
    var warn = c === 'unreachable' ? '<p class="bk-warn"><b>Can’t reach RomM.</b> ' + esc(host) + ': ' + esc(RM.err || 'no answer') + '. Installed RomM games still play; browsing, installs, cloud saves and firmware downloads wait until it’s back.</p>'
      : c === 'expired' ? '<p class="bk-warn"><b>Your RomM sign-in expired.</b> Sign in again to browse, install and sync saves. Installed games still play.</p>' : '';
    var games = G.filter(function (g) { return !g.local; }).length;
    body = warn + '<dl class="kv"><div><dt>Server</dt><dd><button class="linkish" data-rm-open>' + esc(RM.url) + '</button></dd></div><div><dt>User</dt><dd>' + esc(RM.user) + '</dd></div>' +
      '<div><dt>Sign-in</dt><dd>' + (RM.kind === 'token' ? 'API token' : 'Password') + ' · kept in the OS keyring</dd></div><div><dt>Status</dt><dd>' + status + '</dd></div>' +
      '<div><dt>Games</dt><dd>' + (c === 'ok' ? games + ' on the server · ' : '') + n + ' installed here</dd></div></dl>' +
      '<div class="form-actions">' +
      (c === 'ok' ? '<button class="g-btn" data-rm-show>Show in Library</button>' : c === 'unreachable' ? '<button class="g-btn" data-rm-retry' + (RM.busy ? ' disabled' : '') + '>' + (RM.busy ? 'Retrying…' : 'Retry') + '</button>' : c === 'expired' ? '<button class="g-btn" data-rm-signin>Sign in again…</button>' : '') +
      '<button class="g-btn secondary" data-rm-edit>Edit…</button>' + (c === 'ok' ? '<button class="g-btn secondary" data-rm-retry' + (RM.busy ? ' disabled' : '') + '>' + (RM.busy ? 'Reconnecting…' : 'Reconnect') + '</button>' : '') +
      '<button class="g-btn secondary" data-rm-open>Open in browser</button><button class="g-btn secondary destructive" data-rm-disc>Disconnect…</button></div>';
  }
  return '<article class="st-store rm-card' + (c === 'off' ? ' off' : '') + '">' + head + body + '<span class="bk-proto-corner">' + rmProtoSelect() + '</span></article>';
}
renderStoresPane = function (el) {
  el.innerHTML = '<h2>Game stores</h2><p class="note st-lede">Connect the places your games come from. Each one gets a group in the Library. RomM is your own game server: GRID browses it, installs its games with an emulator and syncs their saves. GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder. Steam games install and start through the Steam client.</p>' +
    '<div class="rm-proto-row"><button class="small-btn bk-proto" data-rm-welcome="new">Prototype: open the welcome</button><button class="small-btn bk-proto" data-rm-welcome="import">Prototype: welcome after a Python import</button><button class="small-btn bk-proto" data-rm-restart>Prototype: start again</button></div>' +
    RM_ORDER.map(function (id) { return id === 'romm' ? rmCard() : id === 'steam' ? stSteamCard(stS(id)) : stStoreCard(stS(id)); }).join('');
  gIcons(el);
};

/* ---- the RomM sign-in dialog: Connect, Edit, Sign in again ---- */
var RM_RESULTS = ['works', 'can’t reach the server', 'rejected (401)', 'another user’s token'];
function rmOpenSignIn(mode) {
  RM.dlg = { kind: 'signin', mode: mode, url: RM.url || '', user: RM.user || '', token: RM.kind === 'token', secret: '', err: '', busy: false };
  rmRenderDlg();
}
function rmSignInFields(d, attr) {
  var again = d.mode === 'again';
  return '<label class="st-field"><span>Server URL</span><input class="field" ' + attr + '="url" placeholder="https://romm.example" value="' + syEsc(d.url) + '"' + (again ? ' disabled' : '') + '></label>' +
    (again ? '' : '<label class="check"><input type="checkbox" ' + attr + '-token' + (d.token ? ' checked' : '') + '> Use an API token</label>') +
    (d.token && !again ? '' : '<label class="st-field"><span>Username</span><input class="field" ' + attr + '="user" autocomplete="off" value="' + syEsc(d.user) + '"' + (again ? ' disabled' : '') + '></label>') +
    '<label class="st-field"><span>' + (d.token ? 'API token' : 'Password') + '</span><input class="field" type="password" ' + attr + '="secret" autocomplete="off" value="' + syEsc(d.secret) + '"></label>' +
    (d.token ? '<span class="help">Make a token in RomM under your user settings. GRID keeps it in the OS keyring, never in a file.</span>' : '<span class="help">GRID keeps your password in the OS keyring, never in a file.</span>') +
    (d.err ? '<p class="st-errline" role="alert">' + d.err + '</p>' : '');
}
function rmReady(d) { return d.url.trim() && d.secret.trim() && (d.token || d.user.trim()); }
function rmSignInHTML(d) {
  var t = d.mode === 'again' ? 'Sign in to RomM again' : d.mode === 'edit' ? 'Edit RomM sign-in' : 'Connect to RomM';
  var lede = d.mode === 'again' ? 'RomM no longer accepts GRID’s ' + (d.token ? 'token' : 'password') + ' for ' + esc(d.user) + '. Enter a new one.'
    : d.mode === 'edit' ? 'Change the server or the sign-in. Installed RomM games stay; GRID links them to the server’s games when it connects.'
    : 'GRID browses your RomM server, installs its games with an emulator and syncs their saves.';
  var go = d.busy ? 'Connecting…' : d.mode === 'again' ? 'Sign in' : d.mode === 'edit' ? 'Save and connect' : 'Connect';
  return '<h3>' + t + '</h3><p class="note">' + lede + '</p>' + rmSignInFields(d, 'data-rm-f') +
    '<div class="form-actions bk-foot"><button class="small-btn bk-proto" data-rm-dlg="next">Prototype: next result (' + RM_RESULTS[RM.next] + ')</button>' +
    '<button class="g-btn secondary" data-rm-dlg="close">Cancel</button><button class="g-btn" data-rm-dlg="submit"' + (rmReady(d) && !d.busy ? '' : ' disabled') + '>' + go + '</button></div>';
}
function rmErr(res, d) {
  if (res === 1) return 'Couldn’t reach ' + esc(d.url.trim()) + '. Check the address, and that the server is running.';
  if (res === 2) return d.token ? 'RomM rejected this token (401). Check it, or make a new one.' : 'RomM rejected the username or password (401).';
  return 'This token belongs to “guest”, not “' + esc(d.user || RM.user || 'player') + '”. Use a token for ' + esc(d.user || RM.user || 'player') + ', or disconnect RomM first to sign in as someone else.';
}
/* One connect for the dialog and the welcome row. On success the caller says what follows. */
function rmTry(d, done) {
  d.busy = true; d.err = '';
  var res = RM.next;
  setTimeout(function () {
    d.busy = false;
    if (res) { d.err = rmErr(res, d); done(false); return; }
    RM.url = d.url.trim().replace(/\/$/, ''); if (!/^https?:\/\//.test(RM.url)) RM.url = 'https://' + RM.url;
    if (!d.token) RM.user = d.user.trim(); else if (!RM.user) RM.user = 'player';
    RM.kind = d.token ? 'token' : 'password'; RM.proto = 'ok'; RM.busy = false; RM.err = '';
    rmSetConn('ok');
    done(true);
  }, 800);
}
function rmSubmit() {
  var d = RM.dlg, first = RM.setupNeeded;
  rmTry(d, function (ok) {
    if (!ok) { rmRenderDlg(); return; }
    rmClose();
    stToast(d.mode === 'again' ? 'Signed in to RomM again as ' + RM.user : 'Connected to RomM at ' + rmHost() + ' as ' + RM.user);
    if (first) { RM.setupNeeded = false; rmChooseSystems('settings'); } else renderAll();
  });
  rmRenderDlg();
}

/* ---- Locate files: the stand-in file chooser, the check and the report ---- */
function rmPickHTML(d) {
  var k = d.k, files = RM_PICK[k], pup = RM_FW[k].pup, emu = SY.pick[k];
  return '<h3>' + (pup ? 'Locate PS3UPDAT.PUP' : 'Locate ' + esc(SY.name[k]) + ' firmware') + '</h3>' +
    '<p class="note">' + (pup ? 'Pick the PS3 system software update file you downloaded. GRID checks it, then ' + esc(emu) + ' installs it.' : 'Pick firmware files you have, or a folder. GRID checks each file’s size and MD5, then puts the ones it knows into ' + esc(emu) + '’s folders.') +
    ' The system’s file chooser opens here in the app; this is a stand-in for it.</p>' +
    '<div class="st-fp-path"><span data-icon="drive" data-size="13"></span>/home/player/Downloads/bios</div>' +
    '<ul class="st-files rm-pick">' + files.map(function (f, i) {
      return '<li><label><input type="' + (pup ? 'radio' : 'checkbox') + '" name="rm-pick" data-rm-pickfile="' + i + '"' + (d.sel[i] ? ' checked' : '') + '>' + esc(f[0]) + '<span class="note">' + f[1] + '</span></label></li>';
    }).join('') + '</ul>' +
    '<div class="form-actions bk-foot">' + (pup ? '<span></span>' : '<button class="g-btn secondary" data-rm-dlg="folder">Use the whole folder</button>') +
    '<button class="g-btn secondary" data-rm-dlg="close">Cancel</button><button class="g-btn" data-rm-dlg="open"' + (Object.keys(d.sel).some(function (i) { return d.sel[i]; }) ? '' : ' disabled') + '>Open</button></div>';
}
function rmCheck(d) {
  var k = d.k, all = RM_PICK[k], picked = d.folder ? all.map(function (f, i) { return i; }) : Object.keys(d.sel).filter(function (i) { return d.sel[i]; }).map(Number);
  var rep = { ok: [], nv: [], bad: [], skipped: 0, n: picked.length };
  var known = function (n) { return RM_FW[k].files.some(function (x) { return x[0] === n; }); };
  picked.forEach(function (i) {
    var f = all[i];
    if (f[2] === 'match') rep.ok.push({ file: f[0], as: f[3] });
    else if (f[2] === 'zip') rep.ok.push({ file: f[3], from: f[0], as: f[3] });
    else if (f[2] === 'nohash') rep.nv.push({ file: f[0], as: f[3] });
    else if (f[2] === 'bad') rep.bad.push({ file: f[0], why: RM_FW[k].pup ? 'This PS3UPDAT.PUP isn’t the one GRID knows (size or MD5 differs)' : 'This isn’t the ' + f[0] + ' GRID knows (size or MD5 differs)' });
    else if (d.folder) rep.skipped += 1;
    else rep.bad.push({ file: f[0], why: 'GRID doesn’t know this file for ' + SY.name[k] });
  });
  RM.fwSt[k] = RM.fwSt[k] || {};
  rep.ok.forEach(function (r) { if (known(r.as)) RM.fwSt[k][r.as] = 'ok'; });
  rep.nv.forEach(function (r) { if (known(r.as)) RM.fwSt[k][r.as] = 'nv'; });
  rmFwSync();
  return rep;
}
function rmReportHTML(d) {
  var r = d.rep, k = d.k, emu = SY.pick[k], pup = RM_FW[k].pup;
  var sec = function (cls, title, rows) {
    return rows.length ? '<div class="rm-rep ' + cls + '"><h4>' + title + ' · ' + rows.length + '</h4><ul>' + rows.join('') + '</ul></div>' : '';
  };
  var ok = r.ok.map(function (x) { return '<li><span><code>' + esc(x.file) + '</code>' + (x.from ? ' from <code>' + esc(x.from) + '</code>' : '') + (x.as !== x.file ? ' → <code>' + esc(x.as) + '</code>' : '') + '</span><span class="note">' + (pup ? esc(emu) + ' installed PS3 system software 4.92' : 'Size and MD5 match' + (x.as !== x.file ? '; installed under the name ' + esc(emu) + ' looks for' : '')) + '</span></li>'; });
  var nv = r.nv.map(function (x) { return '<li><span><code>' + esc(x.file) + '</code></span><span class="note">GRID has no checksum for this file, so it can’t check it. If games don’t start, dump it from your console again.</span></li>'; });
  var bad = r.bad.map(function (x) { return '<li><span><code>' + esc(x.file) + '</code></span><span class="note">' + esc(x.why) + '</span></li>'; });
  var none = !r.ok.length && !r.nv.length;
  return '<h3>' + (pup ? 'PS3 system software' : esc(SY.name[k]) + ' firmware') + '</h3>' +
    '<p class="note">' + (d.folder ? 'Searched /home/player/Downloads/bios and the folders in it' : bkPlural(r.n, 'file') + ' from /home/player/Downloads/bios') + (none ? '. Nothing was installed.' : '. ' + (pup ? esc(emu) + ' installed it.' : 'Installed into ' + esc(emu) + '’s folders.')) + '</p>' +
    sec('ok', 'Installed', ok) + sec('nv', 'Installed · not verified', nv) + sec('bad', 'Not installed', bad) +
    (r.skipped ? '<p class="note">' + bkPlural(r.skipped, 'other file') + ' in the folder matched nothing.</p>' : '') +
    '<div class="form-actions bk-foot"><span></span>' + (none ? '<button class="g-btn secondary" data-rm-dlg="again">Choose other files…</button>' : '') + '<button class="g-btn" data-rm-dlg="close">Done</button></div>';
}
function rmRenderDlg() {
  var d = RM.dlg, el = $('rmdlg');
  if (!d) { $('rmscrim').hidden = true; return; }
  var h = d.kind === 'signin' ? rmSignInHTML(d) : d.kind === 'pick' ? rmPickHTML(d)
    : d.kind === 'checking' ? '<h3>Checking ' + bkPlural(d.folder ? RM_PICK[d.k].length : Object.keys(d.sel).filter(function (i) { return d.sel[i]; }).length, 'file') + '…</h3><div class="st-reading"><span class="st-spin"></span>Comparing sizes and MD5s with the files ' + esc(SY.pick[d.k]) + ' needs</div>'
    : rmReportHTML(d);
  el.innerHTML = '<button class="g-icon-btn g-dialog-close" aria-label="Close" data-rm-dlg="close"><span data-icon="close" data-size="20"></span></button>' + h;
  $('rmscrim').hidden = false; gIcons(el);
}
function rmClose() { RM.dlg = null; $('rmscrim').hidden = true; }

/* ---- Settings › Systems: a general tool, with or without RomM ---- */
function rmSysInst(k) { return G.filter(function (g) { return g.installed && g.p === k; }).length; }
function rmSysShown(k) { return rmOn() || RM_SERVER_ONLY.indexOf(k) < 0; }
function rmSysHasGames(k) { return rmSysInst(k) + (rmOn() ? SY.games[k] || 0 : 0) > 0; }
function rmSysCount(k) {
  var inst = rmSysInst(k), srv = rmOn() ? SY.games[k] || 0 : 0;
  if (srv) return srv + ' on RomM' + (inst ? ' · ' + inst + ' installed' : '');
  return inst ? inst + ' installed' : 'No games yet';
}
syNeeds = function () { return SY.order.filter(function (k) { return !SY.pick[k] && SY.rec[k] && rmSysShown(k) && rmSysHasGames(k); }); };
function rmFwBlock(k) {
  var f = RM_FW[k], st = RM.fwSt[k] || {}, e = SY.pick[k], busy = RM.fwBusy[k];
  var inst = f.files.filter(function (x) { return st[x[0]]; }).length, miss = f.files.some(function (x) { return x[1] && !st[x[0]]; });
  var why = !e ? '' : !rmOn() ? 'Install from RomM needs RomM' : !f.romm ? 'Not on your RomM server' : '';
  var can = e && !why;
  var romm = '<button class="small-btn' + (can && miss ? ' primary' : '') + '" data-rm-fwromm="' + k + '"' + (can && !busy ? '' : ' disabled') + (why ? ' title="' + why + '"' : '') + '>' + (busy ? 'Installing…' : 'Install from RomM') + '</button>';
  var loc = '<button class="small-btn' + (e && !can && miss ? ' primary' : '') + '" data-rm-locate="' + k + '"' + (e && !busy ? '' : ' disabled') + '>' + (f.pup ? 'Locate PS3UPDAT.PUP…' : 'Locate files…') + '</button>';
  var note = !e ? '<p class="rm-fw-why">Install an emulator first. GRID puts firmware into its folders.</p>'
    : why === 'Install from RomM needs RomM' ? '<p class="rm-fw-why">Install from RomM needs RomM · <button class="linkish" data-rm-goset>Connect RomM…</button> · or locate files you have.</p>'
    : why ? '<p class="rm-fw-why">Not on your RomM server · locate files you have.</p>' : '';
  return '<div class="rm-fw"><div class="rm-fw-head"><b>Firmware</b><span class="muted-s">' + bkPlural(f.files.length, 'file') + ' · ' + (inst === f.files.length ? 'all installed' : inst + ' installed') + '</span><span class="acts">' + romm + loc + '</span></div>' + note +
    '<ul class="rm-fw-files">' + f.files.map(function (x) {
      var s = st[x[0]];
      var ico = s === 'ok' ? '<span class="st ok" data-icon="check" data-size="12"></span>' : s === 'nv' ? '<span class="st info" title="Not verified">?</span>' : x[1] ? '<span class="st warn">!</span>' : '<span class="st off"></span>';
      var txt = s === 'ok' ? 'Installed · verified' : s === 'nv' ? 'Installed · not verified' : x[1] ? 'Missing' : 'Missing · optional';
      return '<li>' + ico + '<code>' + esc(x[0]) + '</code><span class="muted-s">' + (x[1] ? 'Required' : 'Optional') + ' · ' + esc(x[2]) + (x[3] ? ' · no fixed checksum' : '') + '</span><span class="rm-fw-st' + (!s && x[1] ? ' warn' : '') + '">' + txt + '</span></li>';
    }).join('') + '</ul></div>';
}
var rmOrigRow = syRow;
syRow = function (k) {
  /* Systems with no games and no emulator stay calm: closed, "Not set up". */
  if (!SY.pick[k] && !rmSysHasGames(k) && !SY.busy[k]) {
    var open = !!SY.open[k];
    var body = !open ? '' : '<div class="sys-body">' + (SY.rec[k] ? '<div class="fix"><span>' + SY.rec[k] + ' is recommended for ' + SY.name[k] + '.</span><button class="small-btn" data-sy-install="' + SY.rec[k] + '" data-for="' + k + '">Install ' + SY.rec[k] + '</button></div>'
      : '<p class="note">No emulator GRID installs plays this system. If you have one, use Add an emulator manually.</p>') + (RM_FW[k] ? rmFwBlock(k) : '') + '</div>';
    return '<li class="sysrow rm-calm' + (open ? ' open' : '') + '"><div class="sys-head"><button class="sys-toggle" data-sy-toggle="' + k + '" aria-expanded="' + open + '"><span class="chev" data-icon="chevronRight" data-size="14"></span><b>' + SY.name[k] + '</b><span class="muted-s">No games yet</span></button>' +
      syPicker(k) + '<span class="pill-s">Not set up</span></div>' + body + '</li>';
  }
  var h = rmOrigRow(k);
  /* The old "firmware · from your server" line gives way to the firmware block. */
  h = h.replace(/<div class="check-row">(?:(?!<\/div>).)*from your server(?:(?!<\/div>).)*<\/div>/, '');
  h = h.replace('<span class="muted-s">' + SY.games[k] + ' games</span>', '<span class="muted-s">' + rmSysCount(k) + '</span>');
  if (RM_FW[k] && h.indexOf('<div class="sys-body">') >= 0) {
    var blk = rmFwBlock(k), i = h.indexOf('<details class="tips">');
    h = i >= 0 ? h.slice(0, i) + blk + h.slice(i) : h.replace(/<\/div><\/li>$/, blk + '</div></li>');
  }
  return h;
};
var rmOrigWin = syWindows;
syWindows = function () { return rmOrigWin().replace(SY.games.PC + ' games · runs through Proton', (rmOn() ? SY.games.PC + ' on RomM · runs' : 'Runs') + ' through Proton'); };
var rmOrigSys = renderSystems;
renderSystems = function (el) {
  rmFwSync();
  var keep = SY.order, shown = keep.filter(rmSysShown);
  var yours = shown.filter(function (k) { return SY.pick[k] || rmSysHasGames(k) || SY.busy[k]; });
  var more = shown.filter(function (k) { return yours.indexOf(k) < 0; }).sort(function (a, b) { return SY.name[a].localeCompare(SY.name[b]); });
  SY.order = yours; rmOrigSys(el); SY.order = keep;
  var lede = el.querySelector('.pane-head .lede');
  if (lede) lede.textContent = 'Pick an emulator for each system you play. GRID installs, updates and sets it up, with or without RomM.' + (rmOn() ? ' Systems on your RomM server are listed with their game counts.' : '');
  if (lede) lede.insertAdjacentHTML('afterend', '<div style="margin-top:8px">' + rmProtoSelect() + '</div>');
  if (more.length) {
    var other = el.querySelectorAll('.pane > .sub, #set-pane > .sub'), anchor = other[other.length - 1];
    var html = '<div class="sub rm-more"><h3>More systems</h3><p class="note">GRID can set these up too. They move up when you have games for them or pick an emulator.</p><ul class="sys-list">' + more.map(syRow).join('') + '</ul></div>';
    if (anchor) anchor.insertAdjacentHTML('beforebegin', html); else el.insertAdjacentHTML('beforeend', html);
  }
  gIcons(el); syPlaceMenu();
};

/* ---- Choose your systems: after the first RomM connect ---- */
function rmChooseSystems(ret) {
  FR.ret = ret; FR.ticked = {}; FR.choice = {}; FR.proton = false; FR.protonChoice = '';
  freshShow('setup');
}
var rmOrigHide = freshHide;
freshHide = function () {
  var ret = FR.ret; FR.ret = null;
  if (ret === 'settings') { S.view = 'settings'; S.setPane = 'stores'; }
  else if (ret === 'welcome') { if (rmOn()) { S.view = 'server'; S.platform = 'DISCOVER'; } else { S.view = 'library'; S.libFilter = 'all'; } }
  rmOrigHide();
};

/* ---- the welcome (replaces Connect) ---- */
var RW = { open: null, lib: '~/Games/GRID', romm: null, imported: false };
var rmOrigFresh = freshShow;
freshShow = function (phase) {
  if (phase === 'welcome') { FR.phase = 'welcome'; $('fresh').hidden = false; $('theme-start').hidden = false; rwRender(); return; }
  /* Choose your systems lists the systems with games (server or installed). */
  var keep = SY.order; SY.order = keep.filter(function (k) { return rmSysShown(k) && rmSysHasGames(k); });
  rmOrigFresh(phase); SY.order = keep;
};
function rwShow(imported) {
  freshReset();
  RM.proto = imported ? 'off' : 'never'; RM.snap = null; RM.busy = false; RM.confirm = false; RM.err = ''; RM.setupNeeded = true; RM.fwSt = {};
  RM.url = imported ? 'https://romm.home.lan' : ''; RM.user = imported ? 'player' : ''; RM.kind = 'token';
  rmSetConn('off');
  ['amazon', 'epic', 'gog', 'steam'].forEach(function (id) { stApplyProto(id, 'off'); if (id !== 'steam') stS(id).tool = 'missing'; });
  ST.games.forEach(function (g) { g.inst = false; }); ST.jobs.length = 0; ST.steamForm = null; ST.confirm = null; stS('steam').key = 'none';
  RW.imported = !!imported; RW.lib = imported ? '~/Games/RomM' : '~/Games/GRID'; RW.open = imported ? 'romm' : null;
  RW.romm = { mode: 'connect', url: RM.url, user: RM.user, token: true, secret: '', err: '', busy: false };
  if (imported) {
    [0, 1, 4, 6, 9].forEach(function (i) { G[i].installed = true; });
    SY.installed = ['DuckStation', 'Dolphin', 'RetroArch']; SY.pick.PS = 'DuckStation'; SY.pick.GC = 'Dolphin'; EMU.PS = 'DuckStation'; EMU.GC = 'Dolphin';
  }
  S.view = 'library'; S.libFilter = 'all';
  renderAll();
  freshShow('welcome');
  if (imported) setTimeout(function () { toast('Imported 3 emulators and 5 games from the previous version. Enter your RomM token to reconnect.'); }, 300);
}
function rwConnected() { return RM_ORDER.filter(function (id) { return id === 'romm' ? rmOn() : stS(id).conn === 'ok'; }); }
function rwRow(id) {
  var open = RW.open === id, s = id === 'romm' ? null : stS(id), on = id === 'romm' ? rmOn() : s.conn === 'ok';
  var mark = id === 'romm' ? rmMark('lg') : stMark(s, 'lg'), name = id === 'romm' ? 'RomM' : s.name;
  var sub = on ? (id === 'romm' ? 'Connected as ' + esc(RM.user) + ' · ' + esc(rmHost()) : id === 'steam' ? 'SteamID64 ' + esc(s.steamId) + ' · ' + esc(s.account) : 'Signed in as ' + esc(s.account))
    : id === 'romm' ? 'Your own game server. GRID browses it and installs its games with an emulator.'
    : id === 'steam' ? 'Installed Steam games show as soon as Steam is set up here.'
    : 'Sign in on ' + esc(s.name) + '’s own page. GRID never sees your password.';
  var right = on ? '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Connected</span>' + (id === 'romm' ? '<button class="small-btn" data-rw-open="romm">Edit</button>' : '')
    : open ? '' : id === 'romm' || id === 'steam' ? '<button class="small-btn" data-rw-open="' + id + '">' + (id === 'romm' ? 'Connect…' : 'Set up…') + '</button>'
    : '<button class="small-btn" data-st-connect="' + id + '">Connect…</button>';
  var body = '';
  if (open && id === 'romm') {
    var d = RW.romm;
    body = '<div class="rw-body">' + rmSignInFields(d, 'data-rw-f') + '<div class="form-actions"><button class="g-btn" data-rw-connect' + (rmReady(d) && !d.busy ? '' : ' disabled') + '>' + (d.busy ? 'Connecting…' : 'Connect') + '</button>' +
      '<button class="g-btn secondary" data-rw-close>Cancel</button><button class="small-btn bk-proto" data-rm-dlg="next">Prototype: next result (' + RM_RESULTS[RM.next] + ')</button></div></div>';
  }
  if (open && id === 'steam') body = '<div class="rw-body">' + stSteamCard(s) + '<div class="form-actions"><button class="g-btn secondary" data-rw-close>Close</button></div></div>';
  return '<li class="rw-row' + (open ? ' open' : '') + (on ? ' on' : '') + '"><div class="rw-head">' + mark + '<span class="rw-id"><b>' + name + '</b><span class="note">' + sub + '</span></span>' + right + '</div>' + body + '</li>';
}
function rwRender() {
  if (FR.phase !== 'welcome') return;
  var el = $('fresh'), lib = RW.lib.trim(), any = rwConnected().length, focus = document.activeElement && document.activeElement.getAttribute && (document.activeElement.getAttribute('data-rw-f') || (document.activeElement.hasAttribute('data-rw-lib') ? 'lib' : ''));
  el.innerHTML = '<section class="fr-card rw"><header class="fr-head"><span class="g-logo" data-icon="grid" data-size="28"></span><h1>Welcome to GRID</h1><p class="lede">One Library for the games on your RomM server and the stores you buy from.</p></header>' +
    (RW.imported ? '<p class="bk-warn rw-import"><b>Your settings from the previous version are filled in.</b> Enter your RomM token to reconnect, or skip it for now.</p>' : '') +
    '<div class="rw-step"><h2><span class="rw-n">1</span>Library folder</h2><p class="note">Games, emulators and saves go here.</p><span class="inline"><input class="field" data-rw-lib aria-label="Library folder" value="' + syEsc(RW.lib) + '"><button class="g-btn secondary" type="button" data-toast="Opens the folder chooser">Browse…</button></span>' +
    (lib ? '' : '<p class="st-errline">Choose a folder to continue.</p>') + '</div>' +
    '<div class="rw-step"><h2><span class="rw-n">2</span>Connect your games <span class="muted-s">· optional</span></h2><p class="note">Connect the ones you use. Add or change them any time in Settings › Game stores.</p>' +
    '<ul class="rw-list">' + RM_ORDER.map(rwRow).join('') + '</ul></div>' +
    '<footer class="fr-foot"><div class="form-actions">' + (any ? '' : '<button class="g-btn secondary" data-rw-skip' + (lib ? '' : ' disabled') + '>Skip for now</button>') +
    '<button class="g-btn" data-rw-go' + (any && lib ? '' : ' disabled') + '>Continue</button></div>' +
    '<p class="help">' + (any ? (rmOn() && RM.setupNeeded ? 'Next: choose the systems you play.' : 'GRID opens on your Library.') : 'Nothing connected yet. Skip for now opens GRID with your library folder.') + '</p></footer></section>';
  gIcons(el);
  if (focus) { var f = el.querySelector(focus === 'lib' ? '[data-rw-lib]' : '[data-rw-f="' + focus + '"]'); if (f) { f.focus(); f.setSelectionRange(f.value.length, f.value.length); } }
}
function rwFinish() {
  if (rmOn() && RM.setupNeeded) { RM.setupNeeded = false; rmChooseSystems('welcome'); return; }
  FR.ret = 'welcome'; freshHide();
}

/* ---- Backups without RomM: from the installed copy only ---- */
var rmOrigPlan = bkPlan;
bkPlan = function (dl) {
  if (rmOn()) return rmOrigPlan(dl);
  var choices = {}; dl.ids.forEach(function (id) { choices[id] = 'local'; });
  var pl = rmOrigPlan({ ids: dl.ids, drive: dl.drive, emus: dl.emus, fw: dl.fw, local: true, choices: choices, each: dl.each });
  /* A row with no rom id has no pack identity (REASON_NO_ROM_ID), with or without RomM. */
  var un = pl.sources.filter(function (r) { return r.source !== 'local' || r.g.local === 'unlinked'; });
  pl.unavail = un.map(function (r) { var l = bkLocalOf(r.g); return { g: r.g, why: r.g.local === 'unlinked' ? 'It isn’t linked to a RomM game' : l && l.why ? l.why : 'Not installed on this computer' }; });
  pl.sources = pl.sources.filter(function (r) { return un.indexOf(r) < 0; });
  pl.local = pl.local.filter(function (r) { return un.indexOf(r) < 0; }); pl.localBytes = pl.local.reduce(function (t, r) { return t + bkLocalSize(r.g); }, 0);
  pl.todo = pl.todo.filter(function (g) { return !un.some(function (r) { return r.g === g; }); });
  pl.skipFw = pl.fw; pl.fw = []; pl.outdated = []; pl.needsChoice = 0; pl.downloadBytes = 0;
  pl.size = pl.localBytes + pl.emus.reduce(function (s, n) { return s + bkEmuSize(n, pl.d); }, 0);
  return pl;
};
var rmOrigBkDlg = bkRenderDlg;
bkRenderDlg = function () {
  rmOrigBkDlg();
  if (rmOn() || !BK.dlg) return;
  var el = $('bkdlg'), pl = bkPlan(BK.dlg);
  var lede = el.querySelector('h3 + .note'); if (lede) lede.textContent = 'RomM isn’t connected, so GRID copies the games installed on this computer, with the art and details it has, to the drive’s gridpack folder. Nothing downloads.';
  var loc = el.querySelector('[data-bk-opt=local]');
  if (loc) { loc.checked = true; loc.disabled = true; var n = loc.parentNode.querySelector('.note'); if (n) n.textContent = (pl.local.length ? bkPlural(pl.local.length, 'game') + ' from this computer (' + bkFmt(pl.localBytes) + ')' : 'No game here can be copied') + ' · without RomM, only installed copies'; }
  if (pl.unavail.length) {
    var subs = Array.prototype.slice.call(el.querySelectorAll('.sub')), src = subs.filter(function (x) { var h = x.querySelector('h4'); return h && h.textContent === 'Source'; })[0] || subs[0];
    var html = '<div class="sub rm-unavail"><h4>Can’t backup without RomM · ' + pl.unavail.length + '</h4><ul class="bk-src-list">' + pl.unavail.map(function (u) {
      return '<li><span class="bk-src-name"><b>' + esc(u.g.t) + '</b><span class="note">' + esc(PLAT[u.g.p]) + ' · ' + esc(u.why) + '. ' + (u.g.local === 'unlinked' ? 'Link it to its RomM game first.' : 'Connect RomM to back it up from the server.') + '</span></span></li>';
    }).join('') + '</ul></div>';
    if (src) src.insertAdjacentHTML('afterend', html);
  }
  var fw = el.querySelector('[data-bk-opt=fw]');
  if (fw) { fw.checked = false; fw.disabled = true; var fn = fw.parentNode.querySelector('.note'); if (fn) fn.textContent = 'Needs RomM · skipped' + (pl.skipFw.length ? ': ' + pl.skipFw.map(function (k) { return SY.name[k]; }).join(', ') : ''); }
  var emu = el.querySelector('.bk-auto .note'); if (emu) emu.textContent += ' · from GRID’s catalog; builds hosted on your RomM server are skipped';
};

/* ---- events ---- */
function rmReconnect() {
  RM.busy = true; renderAll();
  setTimeout(function () {
    var was = RM.conn; RM.busy = false; rmSetConn('ok'); RM.proto = 'ok'; RM.err = '';
    stToast(was === 'ok' ? 'Reconnected to ' + rmHost() : 'Connected to RomM again · ' + rmHost());
    renderAll();
  }, 900);
}
function rmApplyProto(v) {
  RM.proto = v; RM.busy = false; RM.confirm = false; RM.err = '';
  if (RM.snap) { RM.snap.forEach(function (x) { G[x[0]].installed = x[1]; }); RM.snap = null; }
  if (v !== 'never' && !RM.url) { RM.url = 'https://romm.home.lan'; RM.user = 'player'; }
  RM.setupNeeded = v === 'never';
  if (v === 'never') { RM.snap = G.map(function (g) { return [g.id, g.installed]; }); G.forEach(function (g) { g.installed = false; }); RM.url = ''; RM.user = ''; rmSetConn('off'); }
  else rmSetConn(v);
  if (v === 'unreachable') RM.err = 'connection refused';
}
document.addEventListener('click', function (e) {
  var t = e.target, b;
  if (t.closest('[data-rm-goset]')) { rmClose(); bkCloseDlg(); closeDetails(); S.view = 'settings'; S.setPane = 'stores'; renderAll(); return; }
  if (t.closest('[data-rm-retry]')) { rmReconnect(); return; }
  if (t.closest('[data-rm-signin]')) { closeDetails(); rmOpenSignIn('again'); return; }
  if (t.closest('[data-rm-edit]')) { rmOpenSignIn('edit'); return; }
  if (t.closest('[data-rm-connect]')) { rmOpenSignIn('connect'); return; }
  if (t.closest('[data-rm-open]')) { toast('Opens ' + (RM.url || 'RomM') + ' in your browser'); return; }
  if (t.closest('[data-rm-link]')) { toast('Opens the link picker: search ' + rmHost() + ' for this game'); return; }
  if (t.closest('[data-rm-disc]')) { RM.confirm = true; renderSettings(); return; }
  if (t.closest('[data-rm-disc-cancel]')) { RM.confirm = false; renderSettings(); return; }
  if (t.closest('[data-rm-disc-go]')) { RM.confirm = false; RM.proto = 'off'; rmSetConn('off'); stToast('RomM disconnected. Installed RomM games stay and still play.'); renderAll(); return; }
  if (t.closest('[data-rm-show]')) { rmGo('romm:installed'); return; }
  if ((b = t.closest('[data-rm-go]'))) { rmGo(b.getAttribute('data-rm-go')); return; }
  if (t.closest('[data-rm-bkall]')) { var ids = rmInst().map(function (g) { return g.id; }); bkOpen(ids, 'Backup RomM · ' + bkPlural(ids.length, 'game')); return; }
  if ((b = t.closest('[data-rm-welcome]'))) { rwShow(b.getAttribute('data-rm-welcome') === 'import'); return; }
  if (t.closest('[data-rm-restart]')) { location.reload(); return; }
  if ((b = t.closest('[data-rm-fwromm]'))) {
    var k = b.getAttribute('data-rm-fwromm'); RM.fwBusy[k] = 1; toast('Installing ' + SY.name[k] + ' firmware from RomM…'); syRerender();
    setTimeout(function () { delete RM.fwBusy[k]; RM.fwSt[k] = RM.fwSt[k] || {}; RM_FW[k].files.forEach(function (x) { RM.fwSt[k][x[0]] = 'ok'; }); rmFwSync(); toast(SY.name[k] + ' firmware installed from RomM · ' + bkPlural(RM_FW[k].files.length, 'file')); syRerender(); }, 1300);
    return;
  }
  if ((b = t.closest('[data-rm-locate]'))) { RM.dlg = { kind: 'pick', k: b.getAttribute('data-rm-locate'), sel: {}, folder: false }; rmRenderDlg(); return; }
  if ((b = t.closest('[data-rm-dlg]'))) {
    var a = b.getAttribute('data-rm-dlg'), d = RM.dlg;
    if (a === 'close') { rmClose(); syRerender(); return; }
    if (a === 'next') { RM.next = (RM.next + 1) % RM_RESULTS.length; if (d) rmRenderDlg(); rwRender(); return; }
    if (a === 'submit') { rmSubmit(); return; }
    if (a === 'again') { d.kind = 'pick'; d.sel = {}; d.folder = false; rmRenderDlg(); return; }
    if (a === 'open' || a === 'folder') {
      d.folder = a === 'folder'; d.kind = 'checking'; rmRenderDlg();
      setTimeout(function () { if (RM.dlg !== d) return; d.rep = rmCheck(d); d.kind = 'report'; rmRenderDlg(); syRerender(); }, 700);
      return;
    }
    return;
  }
  if (t === $('rmscrim')) { rmClose(); return; }
  /* the welcome */
  if ((b = t.closest('[data-rw-open]'))) { RW.open = b.getAttribute('data-rw-open'); if (RW.open === 'romm') RW.romm.err = ''; rwRender(); return; }
  if (t.closest('[data-rw-close]')) { RW.open = null; rwRender(); return; }
  if (t.closest('[data-rw-connect]')) {
    var w = RW.romm;
    rmTry(w, function (ok) { if (ok) { RW.open = null; stToast('Connected to RomM at ' + rmHost() + ' as ' + RM.user); } rwRender(); });
    rwRender(); return;
  }
  if (t.closest('[data-rw-skip]') || t.closest('[data-rw-go]')) { rwFinish(); return; }
});
document.addEventListener('input', function (e) {
  var t = e.target, k = t.getAttribute && t.getAttribute('data-rm-f');
  if (k && RM.dlg) { RM.dlg[k] = t.value; var go = document.querySelector('[data-rm-dlg=submit]'); if (go) go.disabled = !rmReady(RM.dlg); return; }
  k = t.getAttribute && t.getAttribute('data-rw-f');
  if (k) { RW.romm[k] = t.value; var c = document.querySelector('[data-rw-connect]'); if (c) c.disabled = !rmReady(RW.romm); return; }
  if (t.hasAttribute && t.hasAttribute('data-rw-lib')) { var was = !!RW.lib.trim(); RW.lib = t.value; if (was !== !!RW.lib.trim()) rwRender(); }
});
document.addEventListener('change', function (e) {
  var t = e.target;
  if (t.hasAttribute && t.hasAttribute('data-rm-proto')) { rmApplyProto(t.value); renderAll(); return; }
  if (t.hasAttribute && t.hasAttribute('data-rm-f-token')) { RM.dlg.token = t.checked; RM.dlg.err = ''; rmRenderDlg(); return; }
  if (t.hasAttribute && t.hasAttribute('data-rw-f-token')) { RW.romm.token = t.checked; RW.romm.err = ''; rwRender(); return; }
  if (t.hasAttribute && t.hasAttribute('data-rm-openon')) { RM.openOn = t.value; renderSettings(); return; }
  var p = t.getAttribute && t.getAttribute('data-rm-pickfile');
  if (p != null && RM.dlg) { var i = +p; if (RM_FW[RM.dlg.k].pup) RM.dlg.sel = {}; RM.dlg.sel[i] = t.checked; rmRenderDlg(); }
});
document.addEventListener('keydown', function (e) {
  if (e.key === 'Escape' && RM.dlg) { e.stopImmediatePropagation(); rmClose(); }
}, true);

function rmBoot() {
  stBoot();
  /* Open GRID on › Automatic with RomM connected: Library › RomM › Discover. */
  S.view = 'server'; S.platform = 'DISCOVER';
  renderAll();
}
