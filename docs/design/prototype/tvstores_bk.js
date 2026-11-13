/* ------------------------------------------------------------------ stores: backups and RomM installers
   User decisions 2026-11-13, for a controller; the words and rules are the Game
   stores card's. Loaded after tvstores.js. */

/* ---- GOG games on backup drives ---- */
var SNOBK = {
  epic: 'Epic Games games can\'t be backed up: legendary installs only from Epic\'s servers.',
  amazon: 'Amazon Games games can\'t be backed up: nile installs only from Amazon\'s servers.',
  steam: 'Steam games can\'t be backed up here. Steam has its own Backup and restore.'
};
var SBKSRC = { g3: { file: '~/Downloads/brightwater_1_3_58810.sh', isize: '2.9 GB' } };
function sBkSrc(g) {
  var x = SBKSRC[g.id];
  if (x && !x.gone) return { how: 'installer', tag: 'Installer files', note: x.file.split('/').pop() + ' · ' + x.isize + ', and its saves', gb: parseFloat(x.isize) };
  return { how: 'copy', tag: 'Installed copy', note: (x && x.gone ? 'Its installer isn\'t at ' + x.file + ' any more, so the installed files are copied' : 'Installed with gogdl, which keeps no installer, so the installed files are copied') + ' · ' + (parseFloat(g.size) * 1.3).toFixed(1) + ' GB, and its saves', gb: parseFloat(g.size) * 1.3, wine: g.os === 'windows' };
}
function sPackOf(d, g) { return (d.sgames || []).filter(function (x) { return x.id === g.id; })[0]; }
driveOf('couch').sgames = [{ id: 'g6', how: 'installer', ver: '1.2' }, { id: 'g7', how: 'copy', ver: '1.1.0' }];
driveOf('travel').sgames = [{ id: 'g1', how: 'copy', ver: '1.0.4' }];

/* The drive's grid: its RomM games, then its GOG games with a GOG tag. */
var tvbPaint = libPaint;
libPaint = function () {
  tvbPaint();
  var r = S.libRail, v = sView();
  if (r.indexOf('drv:') === 0) {
    var dv = driveOf(r.slice(4)); if (!dv || !dv.connected || !(dv.sgames || []).length) return;
    var L = dv.sgames.map(function (x) { return sG(x.id); }).filter(function (g) { return !S.libQ || g.t.toLowerCase().indexOf(S.libQ.toLowerCase()) >= 0; });
    var grid = $('lib-grid'), e = grid.querySelector('.empty'); if (e && L.length) e.remove();
    grid.insertAdjacentHTML('beforeend', L.map(function (g) { return scover(g, { mixed: 1, k: 'dsg', drive: dv.id }); }).join(''));
    var c = $('lib-h').querySelector('.cnt'); if (c) c.textContent = c.textContent.replace(/^\d+ games/, (dv.games.length + dv.sgames.length) + ' games');
    var sp = $('lib-h').querySelector('.sp'); if (sp) sp.insertAdjacentHTML('beforebegin', '<span class="chip">GOG: <b>' + dv.sgames.length + ' games</b></span>');
    return;
  }
  if (!v || v.s.id !== 'gog' || !v.inst) return;
  var n = sInst(v.s).length, row = $('lib-h').querySelector('.vh-row');
  if (n && row) row.querySelector('.sp').insertAdjacentHTML('afterend', '<button class="btn" data-f data-k="sh-bk" data-a="sbackupall" data-al="Backup">Backup all ' + n + '…</button>');
  if (row) row.insertAdjacentHTML('beforeend', '<span class="chip">' + gl('RT') + ' Select</span>');
  var bar = $('selbar'); if (!bar) return;
  bar.hidden = !S.selMode; if (!S.selMode) return;
  var ids = S.ssel || [], gb = ids.reduce(function (t, id) { return t + sBkSrc(sG(id)).gb; }, 0), all = ids.length === n;
  bar.innerHTML = '<b>' + ids.length + ' selected · ' + gb.toFixed(1) + ' GB to write</b><span class="sp"></span><span class="hint">' + gl('X') + (all ? 'All ' + n + ' selected · Clear' : 'Select all ' + n) + '</span><span class="hint">' + gl('Y') + 'Backup to drive…</span><span class="hint">' + gl('RT') + 'Done</span>';
};
function sGogSel() { var v = sView(); return v && v.s.id === 'gog' && v.inst && S.tab === 'library' && !S.pages.length; }
var tvbRt = TABS.library.rt, tvbHints = TABS.library.hints;
TABS.library.rt = function () { if (sGogSel()) { S.selMode = !S.selMode; S.ssel = []; repaint(); return; } tvbRt(); };
TABS.library.hints = function () { return sGogSel() ? [['Y', 'Search'], ['RT', 'Select']] : tvbHints(); };
var tvbPick = selPick, tvbAll = selAll, tvbAct = selAct, tvbSelHints = selHints, tvbToggle = selToggle;
selPick = function (el) { if (!el.dataset.sid) return tvbPick(el); var i = S.ssel.indexOf(el.dataset.sid); if (i >= 0) S.ssel.splice(i, 1); else S.ssel.push(el.dataset.sid); repaint(); };
selAll = function () { if (!sGogSel()) return tvbAll(); var L = sInst(sst('gog')).map(function (g) { return g.id; }); S.ssel = S.ssel.length === L.length ? [] : L; repaint(); };
selAct = function () { if (!sGogSel()) return tvbAct(); if (!S.ssel.length) { toast('Pick games first'); return; } sBackupPage(S.ssel.slice(), 'Backup ' + S.ssel.length + ' game' + (S.ssel.length === 1 ? '' : 's')); };
selHints = function () {
  if (!S.selMode || !sGogSel() || S.layers.length) return tvbSelHints();
  var n = sInst(sst('gog')).length, on = cur && cur.dataset.sid && S.ssel.indexOf(cur.dataset.sid) >= 0;
  return [['A', on ? 'Unpick' : 'Pick'], ['X', S.ssel.length === n ? 'Clear all' : 'Select all'], ['Y', 'Backup to drive…'], ['RT', 'Done']];
};
selToggle = function () { S.ssel = []; tvbToggle(); };
S.ssel = [];

/* The Backup page for GOG games: the Game backups page's layout. */
function sBackupPage(ids, title) {
  var F = { drive: (readyDrives()[0] || {}).id };
  push(rowsPage({
    title: title, key: 'sbk',
    h1: function () { return esc(title); },
    sub: function () { return 'GRID writes each GOG game to the drive\'s gridpack folder: its installer files when GRID has them, else a copy of the installed game. Saves always go with it. Nothing downloads, and no GOG login is needed.'; },
    rows: function () {
      var R = [{ heading: 'Drive' }], ready = readyDrives();
      if (!ready.length) R.push({ note: 'No backup drive connected. Plug in a drive you set up for GRID, or set one up in Settings › Backup drives.', warn: 1 });
      ready.forEach(function (d) { R.push({ label: esc(d.name), sub: esc(d.label) + ' · ' + d.fs + ' · ' + d.free + ' GB free', radio: F.drive === d.id, fn: function () { F.drive = d.id; repaint(); } }); });
      var P = plan(); R.push({ heading: 'What goes on the drive' });
      P.rows.forEach(function (x) { R.push({ info: 1, label: esc(x.g.t), sub: 'GOG · ' + esc(x.src.note), val: '<span class="st ok">' + x.src.tag + '</span>' }); });
      if (P.rows.some(function (x) { return x.src.wine; })) R.push({ note: 'Windows games are copied without their Wine prefix. The save folders GOG reports are copied from it.' });
      if (ids.indexOf('g3') >= 0) R.push({ label: SBKSRC.g3.gone ? 'Prototype: Brightwater\'s installer is back' : 'Prototype: Brightwater\'s installer was moved', fn: function () { SBKSRC.g3.gone = !SBKSRC.g3.gone; repaint(); } });
      P.skip.forEach(function (t) { R.push({ note: esc(t) + ' is already on ' + esc(driveOf(F.drive).name) + ' and will be skipped.' }); });
      P.refuse.forEach(function (t) { R.push({ note: t, warn: 1 }); });
      return R;
    },
    main: function () {
      var P = plan(), n = P.rows.length, d = driveOf(F.drive), over = d && P.gb > d.free;
      return { label: over ? 'Not enough space' : n ? 'Backup ' + n + ' game' + (n === 1 ? '' : 's') : 'Nothing to backup', disabled: !n || over || !d, why: over ? 'Needs ' + P.gb.toFixed(1) + ' GB · ' + d.free + ' GB free' : 'Nothing new to write', summary: n ? P.gb.toFixed(1) + ' GB to write · nothing to download' : '', fn: function () {
        var b = ++BATCH, active = DL.some(function (x) { return x.st === 'active'; });
        P.rows.forEach(function (x, i) { DL.push({ id: 'sb' + b + '-' + x.g.id, sbk: x.g.id, how: x.src.how, ver: x.g.ver, t: x.g.t, st: !active && i === 0 ? 'active' : 'queued', pct: 0, kind: 'Backup', drive: d.id, batch: b, local: true }); });
        BATCH_INFO[b] = { games: n, emus: 0 }; S.selMode = false; S.ssel = []; pop();
        toast(n + ' game' + (n === 1 ? '' : 's') + ' added to the backup queue for ' + d.name, { act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] });
      } };
    },
    mainHint: 'Backup'
  }));
  function plan() {
    var d = driveOf(F.drive), rows = [], skip = [], refuse = [];
    ids.forEach(function (id) {
      var g = sG(id);
      if (sJob(g)) { refuse.push(esc(g.t) + ' is being installed or updated. Wait for it to finish.'); return; }
      if (S.pages.some(function (p) { return p.title === 'Playing'; }) && g.played === 'today') { refuse.push(esc(g.t) + ' is running right now; close it to copy it.'); return; }
      if (d && sPackOf(d, g)) { skip.push(g.t); return; }
      rows.push({ g: g, src: sBkSrc(g) });
    });
    return { rows: rows, skip: skip, refuse: refuse, gb: rows.reduce(function (t, x) { return t + x.src.gb; }, 0) };
  }
}
function sRestore(g, dv) {
  var x = sPackOf(dv, g); if (!x || g.inst || sJob(g)) return;
  sQueue(g, 'restore', { how: x.how, from: dv.label, dname: dv.name, os: g.builds.indexOf('linux') >= 0 ? 'linux' : 'windows' });
  toast(g.t + ' added to download queue', { s: x.how === 'installer' ? 'Runs its installer from ' + dv.label : 'Copying from ' + dv.label, act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] });
}
ACT.sbackup = function (el) { var g = sG(el.dataset.sid); sBackupPage([g.id], 'Backup ' + g.t); };
ACT.sbackupall = function () { var L = sInst(sst('gog')).map(function (g) { return g.id; }); sBackupPage(L, 'Backup GOG · ' + L.length + ' games'); };
ACT.srestore = function (el) { sRestore(sG(el.dataset.sid), driveOf(el.dataset.drive)); };
ACT.sdrm = function (el) { var g = sG(el.dataset.sid), d = driveOf(el.dataset.drive); confirmBox({ title: 'Remove ' + g.t + ' from ' + d.name + '?', body: 'Its files leave the drive. ' + (g.inst ? 'The copy installed on this computer stays.' : 'You can back it up again later.'), ok: 'Remove from drive', danger: 1, fn: function () { d.sgames = d.sgames.filter(function (x) { return x.id !== g.id; }); toast(g.t + ' removed from ' + d.name); pop(); } }); };

/* ---- RomM games whose archive holds a GOG offline installer ---- */
G.push({ id: 21, t: 'Grey Meridian', c: 'hsl(220 15% 34%)', p: 'WIN', native: 1, rel: 220, added: 13 },
  { id: 22, t: 'Foxglove', c: 'hsl(330 45% 40%)', p: 'WIN', native: 1, rel: 300, added: 14 },
  { id: 23, t: 'Iron Lullaby', c: 'hsl(15 35% 34%)', p: 'WIN', native: 1, rel: 400, added: 15 },
  { id: 24, t: 'Low Tide Radio', c: 'hsl(185 40% 34%)', p: 'WIN', native: 1, rel: 500, added: 16 },
  { id: 25, t: 'Pale Summit', c: 'hsl(210 25% 46%)', p: 'WIN', native: 1, rel: 600, added: 17 });
var RIT = { 'Grey Meridian': { type: 'exe' }, 'Foxglove': { type: 'exe', wizard: 1 }, 'Wren Hollow': { type: 'sh' }, 'Iron Lullaby': { type: 'exe', fail: 'unknown' }, 'Low Tide Radio': { type: 'exe', fail: 'dlc', base: 'Pale Summit' } };
var tvbInstall = install;
install = function (g) {
  tvbInstall(g);
  var r = RIT[g.t]; if (!r) return;
  var d = DL.filter(function (x) { return x.gid === g.id && (x.st === 'active' || x.st === 'queued'); }).pop(); if (d) { d.store = 'romm'; d.ri = r; }
};
function riLine(d) {
  if (d.st === 'queued') return 'Queued';
  if (d.wiz && !d.wizDone) return 'Waiting for the installer\'s setup window · finish it there';
  if (d.pct < 55) return 'Downloading · ' + Math.round(d.pct / 55 * 100) + '% · 38.2 MB/s';
  var q = Math.round((d.pct - 55) / 45 * 100);
  return d.ri.type === 'sh' ? 'Unpacking the GOG installer ' + q + '%' : 'Running the GOG installer silently under ' + STV.compat + ' · ' + q + '%';
}
function riTvTick() {
  DL.forEach(function (d) {
    if (d.sbk && d.st === 'done' && !d.packed) { d.packed = 1; var dv = driveOf(d.drive); dv.sgames = dv.sgames || []; if (!sPackOf(dv, sG(d.sbk))) dv.sgames.push({ id: d.sbk, how: d.how, ver: d.ver }); }
    if (d.store !== 'romm') return;
    if (d.st === 'done' && !d.fixed) { d.fixed = 1; d.line = 'Installed · ran its GOG installer'; }
    if (d.st !== 'active') return;
    var r = d.ri;
    if (r.fail && d.pct >= 55) {
      d.st = 'failed';
      d.line = 'Failed · ' + (r.fail === 'unknown' ? 'Its archive holds an installer GRID doesn\'t recognise (setup.exe has no GOG game info). The download was removed.' : 'Its archive holds a DLC installer for ' + r.base + ', which isn\'t installed. Install ' + r.base + ' first; the download was removed.');
      var base = r.base && G.filter(function (x) { return x.t === r.base; })[0];
      toast('Couldn\'t install ' + d.t, { err: 1, s: r.fail === 'unknown' ? 'Its archive holds an installer GRID doesn\'t recognise.' : 'It\'s DLC for ' + r.base + ', which isn\'t installed.', act: base ? ['Install ' + r.base, function () { install(base); }] : ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] });
      advanceQueue(); if (!S.layers.length) repaint(); return;
    }
    if (r.wizard && d.pct >= 70 && !d.wizDone) {
      if (!d.wiz) toast('The ' + d.t + ' installer couldn\'t run silently', { s: 'Its setup window opened over TV mode. Finish it with a mouse or keyboard; GRID adds the game when it closes.' });
      d.wiz = (d.wiz || 0) + 1; d.pct = 70; if (d.wiz > 8) d.wizDone = 1;
    }
  });
}
/* Details of such a RomM game: one line says what its archive holds. */
var tvbDetails = details;
details = function (g, drv) {
  var o = tvbDetails(g, drv), r = RIT[g.t]; if (!r) return o;
  var rr = o.render;
  o.render = function () { return rr.call(this).replace('<div class="lp">', '<div class="lp">' + (g.inst ? 'Installed by running its <b>GOG installer</b>' : 'Its archive holds a <b>GOG ' + (r.type === 'sh' ? 'Linux' : 'Windows') + ' installer</b>. GRID runs it after the download.') + '<br>'); };
  return o;
};

/* A drive's rail count includes its GOG games. */
var tvbRail = libRailHTML;
libRailHTML = function () {
  var h = tvbRail();
  readyDrives().forEach(function (d) { var n = (d.sgames || []).length; if (n) h = h.replace(new RegExp('(data-key="drv:' + d.id + '"[^>]*>[^<]*<span class="n">)(\\d+)'), function (m, a, c) { return a + (+c + n); }); });
  return h;
};
