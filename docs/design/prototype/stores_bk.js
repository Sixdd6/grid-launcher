/* ==== GOG games on backup drives (user decision 2026-11-13). A pack holds a GOG
   game as its installer files and saves when GRID has the installer (installing
   from the drive runs it again), else as a copy of the installed game with its
   saves, like an installed RomM game's copy. Epic, Amazon and Steam games can't be
   backed up. Loaded after stores.js. ==== */
var ST_NOBK = {
  epic: 'Epic Games games can’t be backed up: legendary installs only from Epic’s servers.',
  amazon: 'Amazon Games games can’t be backed up: nile installs only from Amazon’s servers.',
  steam: 'Steam games can’t be backed up here. Steam has its own Backup and restore.'
};
/* Where each installed GOG game's backup comes from. Games installed with gogdl
   have no installer files (gogdl downloads straight into the game folder); games
   installed from an offline installer keep the installer's path. */
var ST_BKSRC = {
  g3: { file: '~/Downloads/brightwater_1_3_58810.sh', isize: 2.9 }
};
function stBkSrc(g) {
  var x = ST_BKSRC[g.id] || (g.offline ? { file: '~/Downloads/' + g.t.toLowerCase().replace(/[^a-z0-9]+/g, '_') + '_installer.sh', isize: g.size } : null);
  if (x && !x.gone) return { how: 'installer', file: x.file, size: x.isize + .01, note: x.file.split('/').pop() + ' · ' + stFmt(x.isize) + ', and its saves' };
  return { how: 'copy', size: g.disk + .01, note: (x && x.gone ? 'Its installer isn’t at ' + x.file + ' any more, so the installed files are copied' : 'Installed with gogdl, which keeps no installer, so the installed files are copied') + ' · ' + stFmt(g.disk) + ', and its saves', wine: g.os === 'windows' };
}
function stPackOf(d, g) { return (d.sgames || []).filter(function (x) { return x.id === g.id; })[0]; }
function stPackSize(x) { var g = stG(x.id); return x.how === 'installer' ? g.size : g.disk; }
bkDrive('t7').sgames = [{ id: 'g6', how: 'installer', ver: '1.2' }, { id: 'g7', how: 'copy', ver: '1.1.0' }];
bkDrive('wd').sgames = [{ id: 'g1', how: 'copy', ver: '1.0.4' }];
var stOrigUsed = bkGridUsed;
bkGridUsed = function (d) { return stOrigUsed(d) + (d.sgames || []).reduce(function (t, x) { return t + stPackSize(x); }, 0); };

/* ---- select mode in GOG › Installed (the Game backups card's, unchanged) ---- */
function stSelPaint(sv) {
  var gogInst = sv && sv.s.id === 'gog' && sv.part === 'inst' && stInst(sv.s).length;
  var btn = document.querySelector('[data-view=library] [data-bk-select]'), bar = $('lib-selbar');
  if (btn) { btn.hidden = !gogInst; btn.classList.toggle('active', !!ST.sel); btn.setAttribute('aria-pressed', !!ST.sel); btn.textContent = ST.sel ? 'Done' : 'Select'; }
  if (!gogInst || !ST.sel) { if (bar) bar.hidden = true; if (!gogInst) ST.sel = null; return; }
  var grid = $('lib-grid'); grid.classList.add('selecting');
  grid.querySelectorAll('[data-sgame]').forEach(function (c) {
    var picked = ST.sel.ids.indexOf(c.getAttribute('data-sgame')) >= 0;
    c.classList.toggle('picked', picked); c.setAttribute('aria-selected', picked);
    c.querySelector('.g-cover').insertAdjacentHTML('beforeend', '<span class="bk-check" aria-hidden="true">' + (picked ? gIcon('check', 14) : '') + '</span>');
  });
  var vis = Array.prototype.map.call(grid.querySelectorAll('[data-sgame]'), function (c) { return c.getAttribute('data-sgame'); });
  var ids = ST.sel.ids, all = vis.length && vis.every(function (id) { return ids.indexOf(id) >= 0; });
  var size = ids.reduce(function (t, id) { return t + stBkSrc(stG(id)).size; }, 0);
  bar.hidden = false;
  bar.innerHTML = '<span class="bk-sel-count"><b>' + (ids.length ? ids.length + ' selected' : 'Select games') + '</b>' + (ids.length ? '<span class="note">' + bkFmt(size) + ' to write</span>' : '<span class="note">Click covers to pick them</span>') + '</span>' +
    '<button class="small-btn" data-st-selall>' + (all ? 'All ' + vis.length + ' selected' : 'Select all ' + vis.length) + '</button>' + (ids.length ? '<button class="small-btn" data-st-selclear>Clear</button>' : '') +
    '<span class="bk-sel-gap"></span><button class="g-btn" data-st-bkopen' + (ids.length ? '' : ' disabled') + '>Backup to drive…</button>';
}
document.addEventListener('click', function (e) {
  var sv = stView();
  if (!e.target.closest('[data-bk-select]') || !sv || sv.s.id !== 'gog' || sv.part !== 'inst') return;
  e.stopImmediatePropagation(); ST.sel = ST.sel ? null : { ids: [] }; renderLibrary();
}, true);

/* ---- the backup dialog for GOG games (the Game backups dialog's layout) ---- */
function stOpenBackup(ids, title) {
  var ready = bkReady();
  ST.dlg = { kind: 'backup', ids: ids.slice(), title: title, drive: ready[0] ? ready[0].id : null };
  stRenderDlg();
}
function stBkPlan(dl) {
  var d = bkDrive(dl.drive), rows = [], skip = [], refuse = [];
  dl.ids.forEach(function (id) {
    var g = stG(id);
    if (stJob(g)) { refuse.push(esc(g.t) + ' is being installed or updated. Wait for it to finish.'); return; }
    if (ST.playing === g.id) { refuse.push(esc(g.t) + ' is running right now; close it to copy it.'); return; }
    if (d && stPackOf(d, g)) { skip.push(g.t); return; }
    rows.push({ g: g, src: stBkSrc(g) });
  });
  return { d: d, rows: rows, skip: skip, refuse: refuse, size: rows.reduce(function (t, r) { return t + r.src.size; }, 0) };
}
function stBackupHTML(dl) {
  var ready = bkReady(), pl = stBkPlan(dl), n = pl.rows.length, over = pl.d && pl.size > bkFree(pl.d);
  var drives = !ready.length ? '<div class="empty-state"><b>No backup drive connected</b><span class="note">Plug in a drive you set up for GRID. Set up a drive once in Settings › Backup drives.</span><div class="form-actions"><button class="g-btn secondary" data-bk-goset>Set up a drive…</button></div></div>'
    : '<div class="bk-opts" role="radiogroup" aria-label="Drive">' + ready.map(function (d) {
      var p = stBkPlan({ ids: dl.ids, drive: d.id }), o = p.size > bkFree(d);
      return '<label class="bk-opt' + (dl.drive === d.id ? ' on' : '') + (o ? ' over' : '') + '"><input type="radio" name="st-bk-drive" value="' + d.id + '"' + (dl.drive === d.id ? ' checked' : '') + '>' +
        '<span class="bk-opt-main"><b>' + esc(d.name) + '</b><span class="note">' + esc(d.label) + ' · ' + d.fs + '</span></span>' +
        '<span class="bk-opt-free' + (o ? ' warn' : '') + '">' + (o ? 'Needs ' + bkFmt(p.size) + ' · ' + bkFmt(bkFree(d)) + ' free' : bkFmt(bkFree(d)) + ' free') + '</span>' + bkUsage(d, dl.drive === d.id ? p.size : 0) + '</label>';
    }).join('') + '</div>';
  var wine = pl.rows.some(function (r) { return r.src.wine; });
  var list = pl.rows.length ? '<ul class="bk-src-list">' + pl.rows.map(function (r) {
    var tag = r.src.how === 'installer' ? '<span class="bk-src-tag local"><span data-icon="download" data-size="12"></span>Installer files</span>' : '<span class="bk-src-tag local"><span data-icon="drive" data-size="12"></span>Installed copy</span>';
    return '<li><span class="bk-src-name"><b>' + esc(r.g.t) + '</b><span class="note">GOG · ' + esc(r.src.note) + '</span></span>' + tag + '</li>';
  }).join('') + '</ul>' : '';
  var notes = [];
  if (pl.skip.length) notes.push(bkPlural(pl.skip.length, 'game is', 'games are') + ' already on ' + esc(pl.d.name) + ' and will be skipped: ' + esc(pl.skip.join(', ')) + '.');
  pl.refuse.forEach(function (x) { notes.push('<span class="bk-warn-line">' + x + '</span>'); });
  var goLabel = over ? 'Not enough space' : n ? 'Backup ' + bkPlural(n, 'game') : 'Nothing to backup';
  var gone = dl.ids.indexOf('g3') >= 0;
  return '<h3>' + esc(dl.title || 'Backup ' + bkPlural(dl.ids.length, 'game')) + '</h3>' +
    '<p class="note">GRID writes each GOG game to the drive’s gridpack folder: its installer files when GRID has them, else a copy of the installed game. Saves always go with it. Nothing downloads, and no GOG login is needed.</p>' +
    '<div class="sub"><h4>Drive</h4>' + drives + '</div>' +
    (ready.length && pl.rows.length ? '<div class="sub bk-src"><h4>What goes on the drive</h4>' + list + (wine ? '<p class="note bk-src-wine">Windows games are copied without their Wine prefix. The save folders GOG reports are copied from it.</p>' : '') +
      (gone ? '<button class="small-btn bk-proto" data-st-bkgone style="align-self:flex-start">' + (ST_BKSRC.g3.gone ? 'Prototype: Brightwater’s installer is back' : 'Prototype: Brightwater’s installer was moved') + '</button>' : '') + '</div>' : '') +
    (ready.length ? '<p class="note">Covers and the store’s details go too. GRID adds no emulator or Proton for GOG games; the computer that installs a Windows game uses its own compatibility tool.</p>' : '') +
    (notes.length ? '<div class="bk-notes">' + notes.map(function (t) { return '<p class="note">' + t + '</p>'; }).join('') + '</div>' : '') +
    '<div class="form-actions bk-foot">' + (ready.length ? '<span class="note">' + (n ? bkFmt(pl.size) + ' to write · nothing to download' : 'Nothing new to write') + '</span>' : '<span></span>') +
    '<button class="g-btn secondary" data-st-dlg="close">Cancel</button>' + (ready.length ? '<button class="g-btn" data-st-dlg="backup-go"' + (n && !over ? '' : ' disabled') + '>' + goLabel + '</button>' : '') + '</div>';
}
function stBackupGo() {
  var dl = ST.dlg, pl = stBkPlan(dl), d = pl.d, bid = 's' + Date.now();
  if (!pl.rows.length) return;
  BK.batches[bid] = { drive: d.id, games: pl.rows.length, emus: 0, fw: 0, left: pl.rows.length, written: 0 };
  BK.q = BK.q.concat(pl.rows.map(function (r) { return { kind: 'sgame', sid: r.g.id, t: r.g.t, sub: 'GOG · ' + (r.src.how === 'installer' ? 'Installer files and saves' : 'Installed copy and saves'), size: r.src.size, drive: d.id, batch: bid, pct: 0, phase: 'queued', local: true, how: r.src.how, ver: r.g.ver }; }));
  stClose(); ST.sel = null;
  toast(bkPlural(pl.rows.length, 'game') + ' added to the backup queue for ' + d.name);
  renderAll(); bkRun();
}
var stOrigBkFinish = bkFinish;
bkFinish = function (it) {
  if (it.kind === 'sgame') { var d = bkDrive(it.drive); d.sgames = d.sgames || []; if (!stPackOf(d, stG(it.sid))) d.sgames.push({ id: it.sid, how: it.how, ver: it.ver }); }
  stOrigBkFinish(it);
};
/* A GOG backup row reads the game's GOG details from this computer, not the server. */
var stOrigDl2 = renderDownloads;
renderDownloads = function () {
  stOrigDl2();
  if (!BK.job || BK.job.kind !== 'sgame') return;
  ['dl-active', 'strip-line'].forEach(function (id) { var el = $(id); el.innerHTML = el.innerHTML.replace('Getting details from server', 'Reading its GOG details').replace('getting details', 'reading its GOG details'); });
};

/* ---- Library › a backup drive: its GOG games, after the RomM ones ---- */
function stDriveGog() {
  var d = bkLibDrive(); if (!d || !d.connected || !d.init || !(d.sgames || []).length) return;
  var list = d.sgames.map(function (x) { return stG(x.id); }).filter(stMatch), el = $('lib-grid');
  var empty = el.querySelector('.empty'); if (empty && list.length) empty.remove();
  el.insertAdjacentHTML('beforeend', list.map(function (g) {
    var j = stJob(g), s = stS('gog'), prim = g.inst ? ['Play', 'play'] : j ? ['Installing…', 'details'] : ['Install', 'install'];
    return '<div class="g-card" data-sgame="' + g.id + '" data-sdrive="' + d.id + '"><div class="g-cover"><div class="g-art" style="background:' + g.c + '"><span>' + esc(g.t) + '</span></div>' +
      (j ? '<div class="g-tag-stack"><span class="g-tag st-q">' + (j.st === 'active' ? Math.round(j.pct) + '%' : 'QUEUED') + '</span></div>' : '') + (g.inst ? '<span class="g-dot" role="img" aria-label="Installed"></span>' : '') +
      '<span class="g-tag platform g-corner st-tag">' + stMark(s) + 'GOG</span>' + stProgress(g) +
      '<div class="g-shade"></div><div class="g-bar"><button class="g-card-primary" data-st-act="' + prim[1] + '">' + prim[0] + '</button><button class="g-round" data-st-act="details" aria-label="Details" data-icon="more" data-size="15"></button></div></div>' +
      '<span class="g-label" title="' + syEsc(g.t) + '">' + esc(g.t) + '</span></div>';
  }).join(''));
  gIcons(el);
  var n = d.games.length + d.sgames.length;
  $('lib-count').textContent = bkPlural(n, 'game') + ' · ' + bkFmt(bkGridUsed(d)) + ' · on ' + d.label;
  var chips = $('lib-chips'); if (chips) chips.insertAdjacentHTML('beforeend', '<span class="g-chip-outline">GOG: ' + bkPlural(d.sgames.length, 'game') + '</span>');
}
function stRestore(g, d) {
  if (!d || !d.connected) { toast('Plug in the drive to install from it', 'error'); return; }
  var x = stPackOf(d, g); if (!x || g.inst || stJob(g)) return;
  stQueue(g, 'restore', { how: x.how, from: d.label, os: g.builds.indexOf('linux') >= 0 ? 'linux' : 'windows' });
  stToast(g.t + ' added to download queue', '', ['Open Downloads', function () { S.view = 'downloads'; renderAll(); }]);
}
function stRestoreDone(j) {
  var g = stG(j.gid), d = BK.drives.filter(function (x) { return x.label === j.from; })[0], x = d && stPackOf(d, g);
  ST.jobs.splice(ST.jobs.indexOf(j), 1);
  g.inst = true; g.os = j.os; g.ver = x && x.ver || g.ver || '1.0'; if (j.how === 'installer') g.offline = true; g.cloud = g.cloud || 'synced'; g.synced = g.synced || 'just now'; g.played = g.played || 'never';
  var line = j.how === 'installer' ? 'Installed from ' + j.from + ' · ran its installer · saves restored' : 'Copied from ' + j.from + ' · saves restored';
  ST.done.unshift({ t: j.t, s: 'gog', kind: 'restore', line: line });
  stToast(g.t + ' installed from ' + (d ? d.name : j.from));
  renderAll();
}

/* ==== RomM games whose archive holds a GOG offline installer (user decision
   2026-11-13). The user zips the installer (setup_*.exe and its .bin parts, or a
   Linux .sh) and uploads it to RomM; GRID never uploads. After the download GRID
   finds the installer and runs it: a .sh is read as a zip (data/noarch/), an .exe
   runs silently under Wine/Proton (natively on Windows), with the installer's own
   window as a fallback. The game stays a RomM game, with every RomM feature. ==== */
var RI = {
  'Grey Meridian': { type: 'exe' },
  'Foxglove': { type: 'exe', wizard: 1 },
  'Paper Orbit': { type: 'sh' },
  'Ashgrove': { type: 'exe', dlc: 'Harbor Lights' },
  'Iron Lullaby': { type: 'exe', fail: 'unknown' },
  'Low Tide Radio': { type: 'exe', fail: 'dlc', base: 'Pale Summit' }
};
function riOf(g) { return g && RI[g.t]; }
var riOrigStart = startInstall;
startInstall = function (g, fromQueue) {
  if (!riOf(g) || g.installed) return riOrigStart(g, fromQueue);
  if (stJob(g)) return;
  stQueue(g, 'romm', { s: 'romm', p: g.p, ri: riOf(g) });
  stToast(g.t + ' added to download queue', '', ['Open Downloads', function () { S.view = 'downloads'; renderAll(); }]);
};
function riLine(j) {
  var p = Math.round(j.pct), q = Math.max(0, Math.round((p - 55) / 45 * 100));
  if (j.wizard) return 'Waiting for the installer’s setup window · finish it there';
  if (p < 55) return 'Downloading ' + Math.round(p / 55 * 100) + '% · 38.2 MB/s';
  return j.ri.type === 'sh' ? 'Unpacking the GOG installer ' + q + '%' : 'Running the GOG installer silently under ' + (S.compatDefault || 'GE-Proton10-17') + ' · ' + q + '%';
}
function riTick(j) {
  var r = j.ri;
  if (r.fail && j.pct >= 55) {
    ST.jobs.splice(ST.jobs.indexOf(j), 1);
    var why = r.fail === 'unknown' ? 'Its archive holds an installer GRID doesn’t recognise (setup.exe has no GOG game info). The download was removed.'
      : 'Its archive holds a DLC installer for ' + r.base + ', which isn’t installed. Install ' + r.base + ' first; the download was removed.';
    ST.done.unshift({ t: j.t, s: 'romm', p: j.p, gid: j.gid, kind: 'romm', line: PLAT[j.p] + ' · Failed · ' + why, err: true });
    var base = r.base && G.filter(function (x) { return x.t === r.base; })[0];
    stToast('Couldn’t install ' + j.t + ': ' + (r.fail === 'unknown' ? 'its archive holds an installer GRID doesn’t recognise' : 'it’s DLC for ' + r.base + ', which isn’t installed'), 'error',
      base ? ['Install ' + r.base, function () { startInstall(base); }] : ['Open Downloads', function () { S.view = 'downloads'; renderAll(); }]);
    renderAll(); return true;
  }
  if (r.wizard && j.pct >= 72 && !j.wizDone) {
    if (!j.wizard) { j.wizard = true; stToast(j.t + '’s installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.'); }
    j.wizN = (j.wizN || 0) + 1; if (j.wizN > 14) { j.wizard = false; j.wizDone = true; }
    return true;
  }
  return false;
}
function riFinish(j) {
  ST.jobs.splice(ST.jobs.indexOf(j), 1);
  var g = G[j.gid]; g.installed = true;
  ST.done.unshift({ t: j.t, s: 'romm', p: j.p, gid: j.gid, kind: 'romm', line: PLAT[j.p] + ' · ' + (j.ri.dlc ? 'Added to ' + j.ri.dlc + ' · ran its GOG installer' : 'Installed · ran its GOG installer') });
  stToast(j.ri.dlc ? j.t + ' added to ' + j.ri.dlc : j.t + ' installed', '', j.ri.dlc ? null : ['Play', function () { S.playing = g.id; toast('Launching ' + g.t + '…'); }]);
  renderAll();
}
var riOrigCard = card;
card = function (g, sp) {
  var h = riOrigCard(g, sp), j = stJob(g); if (!j || g.installed) return h;
  h = h.replace('data-act="primary">Install<', 'data-act="primary">' + (j.st === 'queued' ? 'Queued' : 'Installing…') + '<');
  return h.replace('<div class="g-shade">', '<div class="g-tag-stack"><span class="g-tag st-q">' + (j.st === 'active' ? Math.round(j.pct) + '%' : 'QUEUED') + '</span></div>' + stProgress(g) + '<div class="g-shade">');
};
/* Details of such a RomM game: one line says what its archive holds. */
function riDetails() {
  var g = S.detail, r = riOf(g); if (!r) return;
  var left = $('d-left'), j = stJob(g), first = left.querySelector('.g-meta-line');
  var inst = left.querySelector('[data-d=install]'); if (inst && j) { inst.textContent = j.st === 'queued' ? 'Queued' : 'Installing… ' + Math.round(j.pct) + '%'; inst.disabled = true; }
  var line = g.installed ? (r.dlc ? 'Added to ' + r.dlc + ' by its GOG installer' : 'Installed by running its GOG installer')
    : 'Its archive holds a GOG ' + (r.dlc ? 'DLC installer for ' + r.dlc : r.type === 'sh' ? 'Linux installer' : 'Windows installer') + '. GRID runs it after the download' + (r.type === 'exe' ? ', under ' + (S.compatDefault || 'GE-Proton10-17') : '') + '.';
  if (first) first.insertAdjacentHTML('beforebegin', '<p class="g-meta-line">' + line + '</p>');
}

/* ---- events for backups and drive games ---- */
document.addEventListener('click', function (e) {
  var t = e.target;
  if (t.closest('[data-st-selall]')) { var vis = stInst(stS('gog')).map(function (g) { return g.id; }); ST.sel.ids = ST.sel.ids.length === vis.length ? [] : vis; renderLibrary(); return; }
  if (t.closest('[data-st-selclear]')) { ST.sel.ids = []; renderLibrary(); return; }
  if (t.closest('[data-st-bkopen]')) { stOpenBackup(ST.sel.ids, 'Backup ' + bkPlural(ST.sel.ids.length, 'game')); return; }
  if (t.closest('[data-st-bkall]')) { var ids = stInst(stS('gog')).map(function (g) { return g.id; }); stOpenBackup(ids, 'Backup GOG · ' + bkPlural(ids.length, 'game')); return; }
  if (t.closest('[data-st-bkgone]')) { ST_BKSRC.g3.gone = !ST_BKSRC.g3.gone; stRenderDlg(); return; }
  if (t.closest('[data-bk-goset]') && ST.dlg) { stClose(); }
});
document.addEventListener('change', function (e) { if (e.target.name === 'st-bk-drive') { ST.dlg.drive = e.target.value; stRenderDlg(); } });

/* A drive's rail count includes its GOG games. */
var stOrigLib2 = renderLibrary;
renderLibrary = function () {
  stOrigLib2();
  BK.drives.forEach(function (d) { var n = (d.sgames || []).length, c = n && document.querySelector('#lib-rail [data-key="drive:' + d.id + '"] .g-rail-count'); if (c) c.textContent = d.games.length + n; });
};
