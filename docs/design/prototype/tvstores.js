/* ------------------------------------------------------------------ game stores
   The Game stores design in TV mode: GOG (and its offline installers), Epic Games,
   Amazon Games and Steam. The words, rules and states are the Game stores card's;
   only the controls change for a controller. Placeholder titles; the store marks are
   monogram tiles, never a logo. */
var STV = {
  order: ['amazon', 'epic', 'gog', 'steam'],
  stores: {
    amazon: { id: 'amazon', name: 'Amazon Games', mono: 'A', hue: 'hsl(32 80% 42%)', conn: 'expired', account: 'sam.plays', tool: 'ok', toolName: 'nile', toolPkg: 'nile', toolVer: '1.1.2', toolSize: '9 MB', license: 'GPL-3.0', src: 'github.com/imLinguin/nile', host: 'www.amazon.com/ap/signin', folder: '/home/sam/Games/GRID/stores/amazon', refreshed: '3 days ago' },
    epic: { id: 'epic', name: 'Epic Games', mono: 'E', hue: 'hsl(230 10% 26%)', conn: 'ok', account: 'SamPlays', tool: 'ok', toolName: 'legendary', toolPkg: 'legendary', toolVer: '0.20.37', toolSize: '14 MB', license: 'GPL-3.0', src: 'github.com/derrod/legendary', host: 'www.epicgames.com/id/login', cloud: true, folder: '/home/sam/Games/GRID/stores/epic', refreshed: '5 min ago' },
    gog: { id: 'gog', name: 'GOG', mono: 'G', hue: 'hsl(278 45% 42%)', conn: 'ok', account: 'sam_plays', tool: 'ok', toolName: 'gogdl', toolPkg: 'heroic-gogdl', toolVer: '1.1.2', toolSize: '12 MB', license: 'GPL-3.0', src: 'github.com/Heroic-Games-Launcher/heroic-gogdl', host: 'login.gog.com/auth', cloud: true, folder: '/home/sam/Games/GRID/stores/gog', refreshed: '5 min ago' },
    steam: { id: 'steam', name: 'Steam', mono: 'S', hue: 'hsl(210 45% 30%)', conn: 'ok', account: 'SamPlays', tool: 'ok', steamId: '76561198000000001', key: 'ok', client: true, refreshed: '5 min ago',
      accounts: [['SamPlays', '76561198000000001', 'Signed in to Steam on this computer'], ['couchguest', '76561198000000002', 'Has signed in on this computer before']] }
  },
  games: [
    { id: 'g1', s: 'gog', t: 'Ashen Tide', c: 'hsl(12 55% 38%)', inst: 1, os: 'linux', ver: '1.0.4', size: '14.2 GB', played: '2 days ago', cloud: 'synced', synced: '2 hours ago', builds: ['linux', 'windows'], dlc: [['Original Soundtrack', '410 MB', 1], ['Digital Artbook', '120 MB', 0]] },
    { id: 'g2', s: 'gog', t: 'Lanternfall', c: 'hsl(42 60% 36%)', inst: 1, os: 'windows', ver: '2.1', upd: '2.2', size: '32.8 GB', played: 'yesterday', cloud: 'conflict', synced: '3 days ago', builds: ['windows'] },
    { id: 'g3', s: 'gog', t: 'Brightwater', c: 'hsl(190 50% 32%)', inst: 1, os: 'linux', ver: '1.3', size: '3.1 GB', played: '3 weeks ago', cloud: 'off', builds: ['linux', 'windows'], offline: 1 },
    { id: 'g4', s: 'gog', t: 'Copper Saints', c: 'hsl(25 45% 30%)', size: '18.4 GB', builds: ['linux', 'windows'] },
    { id: 'g5', s: 'gog', t: 'Hush Valley', c: 'hsl(140 30% 30%)', size: '6.2 GB', builds: ['windows'] },
    { id: 'g6', s: 'gog', t: 'Marrowind Keep', c: 'hsl(300 25% 30%)', size: '22.0 GB', builds: ['linux', 'windows'], langs: ['English', 'Deutsch', 'Français', 'Polski'], dlc: [['The Sunken Crypt', '2.4 GB', 0], ['Soundtrack', '300 MB', 0]] },
    { id: 'g7', s: 'gog', t: 'Paper Lanterns', c: 'hsl(350 45% 40%)', size: '1.1 GB', builds: ['linux'] },
    { id: 'g8', s: 'gog', t: 'Quarry Nine', c: 'hsl(210 15% 34%)', size: '9.7 GB', builds: ['windows'] },
    { id: 'e1', s: 'epic', t: 'Starlight Courier', c: 'hsl(250 45% 36%)', inst: 1, os: 'windows', ver: '1.12', size: '24.0 GB', played: '4 days ago', cloud: 'synced', synced: '4 days ago', builds: ['windows'] },
    { id: 'e2', s: 'epic', t: 'Vector Bloom', c: 'hsl(160 45% 30%)', inst: 1, os: 'windows', ver: '3.0.1', size: '8.8 GB', played: 'never', cloud: 'none', builds: ['windows'] },
    { id: 'e3', s: 'epic', t: 'Glimmer Deep', c: 'hsl(195 55% 30%)', size: '45.0 GB', builds: ['windows'] },
    { id: 'e4', s: 'epic', t: 'Iron Orchard', c: 'hsl(20 30% 28%)', size: '52.0 GB', builds: ['windows'], ns: 'the EA app' },
    { id: 'e5', s: 'epic', t: 'Northern Static', c: 'hsl(205 25% 36%)', size: '38.0 GB', builds: ['windows'], ns: 'Ubisoft Connect' },
    { id: 'a1', s: 'amazon', t: 'Fogline', c: 'hsl(220 20% 34%)', inst: 1, os: 'windows', ver: '1.0', size: '5.5 GB', played: 'last month', builds: ['windows'] },
    { id: 'a2', s: 'amazon', t: 'Kettle Drum', c: 'hsl(5 45% 36%)', size: '2.2 GB', builds: ['windows'] },
    { id: 'a3', s: 'amazon', t: 'Lowsun', c: 'hsl(45 55% 38%)', size: '15.0 GB', builds: ['windows'] },
    { id: 's1', s: 'steam', t: 'Moonlit Ferry', c: 'hsl(235 35% 30%)', inst: 1, played: 'today', size: '7.0 GB' },
    { id: 's2', s: 'steam', t: 'Deep Field Radio', c: 'hsl(170 30% 28%)', inst: 1, played: '1 week ago', outside: 1, size: '3.3 GB' },
    { id: 's3', s: 'steam', t: 'Old Brick Road', c: 'hsl(15 35% 32%)', inst: 1, played: 'never', outside: 1, size: '11.0 GB' },
    { id: 's4', s: 'steam', t: 'Quiet Comet', c: 'hsl(265 30% 36%)', size: '6.0 GB' },
    { id: 's5', s: 'steam', t: 'Harbor of Bells', c: 'hsl(200 40% 38%)', size: '19.0 GB' },
    { id: 's6', s: 'steam', t: 'Rime Assembly', c: 'hsl(185 25% 40%)', size: '4.4 GB' }
  ],
  compat: 'GE-Proton9-20', testIdx: 0
};
var SKIND = { restore: '', install: '', update: 'Update', repair: 'Repair', dlc: 'Content', tool: 'Store tool', installer: 'Installer', steam: '' };
var sst = function (id) { return STV.stores[id]; };
var sG = function (id) { return STV.games.filter(function (g) { return g.id === id; })[0]; };
var sOf = function (sid) { return STV.games.filter(function (g) { return g.s === sid; }); };
var sInst = function (s) { return sOf(s.id).filter(function (g) { return g.inst; }); };
var sAvail = function (s) { return sOf(s.id).filter(function (g) { return !g.inst; }); };
var sShown = function (s) { return s.conn !== 'off' || sInst(s).length > 0; };
function smark(s, cls) { return '<span class="smono' + (cls ? ' ' + cls : '') + '" style="--h:' + s.hue + '">' + s.mono + '</span>'; }
function sJob(g) { return DL.filter(function (d) { return d.sgid === g.id && (d.st === 'sactive' || d.st === 'squeued'); })[0]; }
function sBlock(s) {
  if (s.conn === 'off') return 'off'; if (s.conn === 'expired') return 'expired';
  if (s.id === 'steam') return s.key === 'ok' ? '' : s.key;
  if (s.tool !== 'ok') return 'tool'; if (s.loading) return 'loading'; return '';
}
function sQuick(g) {
  var s = sst(g.s), j = sJob(g);
  if (j || g.ns) return null;
  if (g.inst) return 'Play';
  if (s.id === 'steam') return 'Install in Steam';
  return s.conn === 'ok' ? 'Install…' : null;
}
function scover(g, o) {
  o = o || {};
  var s = sst(g.s), j = sJob(g), b = '', q = sQuick(g);
  if (o.drive) q = j ? null : g.inst ? 'Play' : 'Install from drive';
  var pk = S.selMode && (S.ssel || []).indexOf(g.id) >= 0;
  if (pk) b += '<span class="pick" aria-label="Selected">' + gIcon('check', 16) + '</span>';
  if (g.ns) b += '<span class="tag sns">NOT SUPPORTED</span>';
  else if (g.inst && g.cloud === 'conflict') b += '<span class="tag scf">SAVE CONFLICT</span>';
  else if (g.inst && g.upd && !j) b += '<span class="tag upd">UPDATE</span>';
  if (g.inst) b += '<i class="idot" aria-label="Installed"></i>';
  if (o.mixed) b += '<span class="tag sst">' + smark(s) + esc(s.name) + '</span>';
  if (j) b += '<span class="tag q">' + (j.st === 'sactive' ? j.pct + '%' : 'Queued') + '</span><span class="sprog' + (j.kind === 'steam' ? ' steam' : '') + '"><b style="width:' + (j.st === 'sactive' ? j.pct : 0) + '%"></b></span>';
  return '<button class="cv' + (g.ns ? ' sns' : '') + (pk ? ' pk' : '') + '" data-f data-k="' + (o.k || 's') + g.id + '" data-a="sdetails" data-sid="' + g.id + '"' + (o.drive ? ' data-drive="' + o.drive + '"' : '') + ' data-al="Details" data-onfocus="shero"' + (q ? ' data-xl="' + q + '" data-xa="squick"' : '') + ' style="--c:' + g.c + '">' +
    '<span class="cv-art">' + b + '<span class="ttl">' + esc(g.t) + '</span></span><span class="cv-l">' + esc(g.t) + '</span></button>';
}
ONF.shero = function (el) { var g = sG(el.dataset.sid); if (g) hero({ c: g.c }); };

/* ---- Library: one folder per store, Installed and Available under it ---- */
STV.order.forEach(function (id) { S.folds['st-' + id] = true; });
var tvsOrigRail = libRailHTML;
libRailHTML = function () {
  return tvsOrigRail() + STV.order.map(function (id) {
    var s = sst(id); if (!sShown(s)) return '';
    var blk = sBlock(s), flag = s.conn === 'expired' || (blk && blk !== 'loading' && blk !== 'off') || (id === 'steam' && !s.client) ? '<span class="sflag">!</span>' : s.conn === 'off' ? '<span class="sflag off">off</span>' : '';
    var row = function (part, label, n) { return railItem('st:' + id + ':' + part, label, n, S.libRail === 'st:' + id + ':' + part, 'lib').replace('class="ri', 'class="ri sub'); };
    var an = blk === 'loading' ? '<span class="sspin"></span>' : blk ? '!' : sAvail(s).length;
    var html = row('inst', 'Installed', id === 'steam' && !s.client ? '!' : sInst(s).length) + row('avail', 'Available', an);
    if (blk && blk !== 'loading') html = html.replace(/(data-key="st:[a-z]+:avail"[^>]*>Available<span class="n)/, '$1 warn');
    return folder('st-' + id, smark(s) + esc(s.name), 0, '', html).replace('data-a="fold"', 'data-a="fold"').replace('</button>', flag + '</button>');
  }).join('');
};
function sView() { var r = S.libRail; if (r.indexOf('st:') !== 0) return null; var p = r.split(':'); return { s: sst(p[1]), inst: p[2] === 'inst' }; }
var tvsOrigList = libList;
libList = function () { return sView() ? [] : tvsOrigList(); };
var tvsOrigPaint = libPaint;
libPaint = function () {
  var v = sView(), h = $('lib-h');
  if (!v) { if (h) h.className = 'vh'; return tvsOrigPaint(); }
  if (!sShown(v.s)) { S.libRail = 'all'; h.className = 'vh'; return tvsOrigPaint(); }
  var s = v.s, blk = sBlock(s), L = (v.inst ? sInst(s) : sAvail(s)).filter(function (g) { return !S.libQ || g.t.toLowerCase().indexOf(S.libQ.toLowerCase()) >= 0; });
  var cnt = v.inst ? L.length + ' game' + (L.length === 1 ? '' : 's') + (s.id === 'steam' ? ' · in your Steam libraries' : '') : blk ? 'Games you own and haven\'t installed' : L.length + ' games you own and haven\'t installed · updated ' + s.refreshed;
  var chips = s.conn === 'ok' ? '<span class="chip">' + (s.id === 'steam' ? 'Steam account: <b>' : 'Signed in as <b>') + esc(s.account) + '</b></span>' : s.conn === 'expired' ? '<button class="chip warn" data-f data-k="sh-login" data-a="slogin" data-id="' + s.id + '" data-al="Sign in again">Login expired · Sign in again</button>' : '<span class="chip">Not connected</span>';
  if (s.cloud && s.conn !== 'off') chips += '<span class="chip">Cloud saves: <b>on</b></span>';
  var acts = (s.id === 'gog' ? '<button class="btn" data-f data-k="sh-inst" data-a="sinstaller" data-al="Choose a file">Install from installer…</button>' : '') + (!v.inst && !blk ? '<button class="btn" data-f data-k="sh-ref" data-a="srefresh" data-id="' + s.id + '" data-al="Refresh">Refresh</button>' : '');
  h.className = 'vh two';
  h.innerHTML = '<div class="vh-top"><h1>' + esc(s.name) + ' <span class="muted">· ' + (v.inst ? 'Installed' : 'Available') + '</span></h1><span class="cnt">' + cnt + '</span></div><div class="vh-row" data-nav="h">' + chips + (S.libQ ? '<span class="chip">Search: <b>' + esc(S.libQ) + '</b></span>' : '') + '<span class="sp"></span>' + acts + '<span class="chip">' + gl('Y') + ' Search</span></div>';
  var grid = $('lib-grid'), st = function (o) { return '<div class="sstate' + (o.tone ? ' ' + o.tone : '') + '">' + smark(s, 'lg') + '<h2>' + o.h + '</h2><p>' + o.p + '</p>' + (o.b ? '<div class="btns" data-nav="h">' + o.b + '</div>' : '') + '</div>'; };
  var B = function (lab, a, pri, extra) { return '<button class="btn' + (pri ? ' pri' : '') + '" data-f data-k="sb-' + a + '" data-a="' + a + '" data-id="' + s.id + '"' + (extra || '') + '>' + lab + '</button>'; };
  if (v.inst) {
    if (s.id === 'steam' && !s.client) { grid.innerHTML = st({ h: 'Steam isn\'t installed', p: 'GRID installs, starts and uninstalls Steam games through the Steam client. Install Steam on this PC and sign in once, then press Check again.', b: B('Check again', 'srecheck', 1) }); return; }
    if (!sInst(s).length) { grid.innerHTML = st({ h: 'No ' + esc(s.name) + ' games installed yet', p: s.id === 'gog' ? 'Install one from Available, or from a GOG offline installer you downloaded.' : s.id === 'steam' ? 'Install one from Available. Games you install in Steam itself show here too.' : 'Install one from Available.', b: B('Open Available', 'sgoavail', 1) + (s.id === 'gog' ? B('Install from installer…', 'sinstaller') : '') }); return; }
  } else {
    if (blk === 'off') { grid.innerHTML = st({ h: esc(s.name) + ' isn\'t connected', p: 'Your installed ' + esc(s.name) + ' games still play. Connect again to see the games you own and install them.', b: B('Connect ' + esc(s.name) + '…', 'slogin', 1) }); return; }
    if (blk === 'expired') { grid.innerHTML = st({ tone: 'warn', h: 'Your ' + esc(s.name) + ' login expired', p: 'Sign in again to see the games you own and install them. Installed games still play.', b: B('Sign in again…', 'slogin', 1) }); return; }
    if (blk === 'tool') { grid.innerHTML = st({ tone: s.tool === 'failed' ? 'err' : '', h: s.tool === 'failed' ? 'Couldn\'t install ' + s.toolName : esc(s.name) + ' needs ' + s.toolName, p: s.tool === 'failed' ? 'The download didn\'t match its checksum, so GRID deleted it. Nothing else changed.' : 'GRID installs and updates ' + esc(s.name) + ' games with ' + s.toolName + ' (' + s.toolPkg + ', ' + s.license + '). It\'s ' + s.toolSize + '.', b: s.tool === 'installing' ? B('Installing ' + s.toolName + '…', 'stool', 1, ' disabled') : B(s.tool === 'failed' ? 'Try again' : 'Install ' + s.toolName + ' (' + s.toolSize + ')', 'stool', 1) }); return; }
    if (blk === 'none') { grid.innerHTML = st({ h: 'Steam needs your Web API key to list the games you haven\'t installed', p: 'Installed Steam games show without it. GRID asks Steam for your owned games with your own key, kept in the OS keyring.', b: B('Open Settings › Stores', 'sgoset', 1) }); return; }
    if (blk === 'bad') { grid.innerHTML = st({ tone: 'err', h: 'Steam rejected your Web API key', p: 'Steam answered 403 Forbidden. The key may have been revoked. Make a new key, then enter it in Settings › Stores › Steam.', b: B('Open Settings › Stores', 'sgoset', 1) }); return; }
    if (blk === 'private') { grid.innerHTML = st({ tone: 'warn', h: 'Your Steam game details are private', p: 'Steam lists owned games only when the profile\'s game details are public. In Steam: your profile › Edit Profile › Privacy Settings › Game details: Public. Then refresh.', b: B('Refresh', 'srefresh', 1) + B('Open Settings › Stores', 'sgoset') }); return; }
    if (blk === 'loading') { grid.innerHTML = '<p class="note" style="grid-column:1/-1;margin:0"><span class="sspin"></span> Loading your ' + esc(s.name) + ' games…</p><div class="sskel"><i></i><i></i><i></i><i></i><i></i></div>'; return; }
    if (!sAvail(s).length) { grid.innerHTML = st({ h: 'Every ' + esc(s.name) + ' game you own is installed', p: 'New games you buy show here after a refresh.', b: B('Refresh', 'srefresh', 1) }); return; }
  }
  grid.innerHTML = L.length ? L.map(function (g) { return scover(g); }).join('') : '<div class="empty"><h2>No games match “' + esc(S.libQ) + '”</h2>Press ' + gl('Y') + ' to change the search.</div>';
};
/* All games, Recent and Updates hold the installed store games too, with the store's tag. */
var tvsOrigPaint2 = libPaint;
libPaint = function () {
  tvsOrigPaint2();
  var r = S.libRail; if (['all', 'recent', 'updates'].indexOf(r) < 0) return;
  var L = STV.games.filter(function (g) { return g.inst && sShown(sst(g.s)) && (!S.libQ || g.t.toLowerCase().indexOf(S.libQ.toLowerCase()) >= 0) && (r === 'all' || (r === 'recent' ? /today|yesterday|days ago|week ago/.test(g.played || '') : g.upd)); });
  if (!L.length) return;
  var grid = $('lib-grid'), e = grid.querySelector('.empty'); if (e) e.remove();
  grid.insertAdjacentHTML('beforeend', L.map(function (g) { return scover(g, { mixed: 1, k: 'ls' }); }).join(''));
  var c = $('lib-h').querySelector('.cnt'), n = grid.querySelectorAll('.cv').length; if (c) c.textContent = n + ' game' + (n === 1 ? '' : 's');
};
var tvsLibHints = TABS.library.hints, tvsLibRt = TABS.library.rt;
TABS.library.hints = function () { return sView() ? [['Y', 'Search']] : tvsLibHints(); };
TABS.library.rt = function () { if (!sView()) tvsLibRt(); };

/* ---- the store game page (Details) ---- */
S.sdTab = 'Overview';
function sTabs(g) { var s = sst(g.s); return ['Overview', 'Media'].concat(s.cloud && g.inst ? ['Cloud saves'] : []); }
function sdetails(g, drv) {
  return {
    title: g.t, sid: g.id,
    render: function () {
      var s = sst(g.s), j = sJob(g), btn = [], steam = s.id === 'steam', pack = drv && sPackOf(drv, g);
      var B = function (lab, a, pri, extra) { btn.push('<button class="btn' + (pri ? ' pri' : '') + (/Uninstall/.test(lab) ? ' dng' : '') + '" data-f data-k="sd-' + a + '" data-a="' + a + '" data-sid="' + g.id + '"' + (extra || '') + '>' + lab + '</button>'); };
      if (j && j.kind === 'steam') B('Installing in Steam · ' + j.pct + '%', 'snop', 1, ' disabled data-why="Managed by Steam. Pause or cancel it in Steam."');
      else if (j) B('Cancel ' + (j.kind === 'update' ? 'update' : j.kind === 'repair' ? 'verify' : 'install') + (j.st === 'sactive' ? ' · ' + j.pct + '%' : ' · queued'), 'scancel', 1);
      else if (g.inst) B(gIcon('play', 18) + 'Play', 'splay', 1);
      else if (pack) B(gIcon('download', 18) + 'Install from drive', 'srestore', 1, ' data-drive="' + drv.id + '"');
      else if (g.ns) B('Not supported', 'snop', 1, ' disabled data-why="Needs ' + g.ns + '. GRID can\'t install games that start through another company\'s launcher."');
      else if (steam) B(gIcon('download', 18) + 'Install in Steam', 'sinstall', 1);
      else B(gIcon('download', 18) + 'Install…', 'sinstall', 1, s.conn !== 'ok' ? ' disabled data-why="' + (s.conn === 'off' ? 'Connect ' + s.name + ' in Settings › Stores to install.' : 'Your ' + s.name + ' login expired. Sign in again in Settings › Stores.') + '"' : '');
      if (g.inst && g.upd && !j) B('Update to ' + g.upd, 'supdate');
      if (g.inst && !steam) B('Game settings', 'ssettings');
      if (g.inst && !j) B(steam ? 'Verify in Steam' : 'Verify and repair', 'sverify');
      if (g.inst && !j) B(steam ? 'Uninstall in Steam…' : 'Uninstall', 'suninstall');
      if (pack) B('Remove from drive…', 'sdrm', 0, ' data-drive="' + drv.id + '"');
      else if (s.id === 'gog' && g.inst) B('Backup to drive…', 'sbackup');
      var tabs = sTabs(g); if (tabs.indexOf(S.sdTab) < 0) S.sdTab = 'Overview';
      var lp = [];
      if (j && j.kind === 'steam') lp.push('<div class="smanaged"><b>Managed by Steam.</b> Pause or cancel it in Steam.<div class="pbar"><b style="width:' + j.pct + '%"></b></div></div>');
      if (g.ns) lp.push('<b>Needs ' + esc(g.ns) + '.</b> GRID can\'t install games that start through another company\'s launcher.');
      if (g.inst) lp.push('Last played <b>' + esc(g.played || 'never') + '</b>');
      lp.push(steam ? 'Launches through <b>Steam</b>' : !g.inst ? (g.builds.indexOf('linux') >= 0 ? 'Has a <b>Linux version</b>' : 'Windows only · runs through <b>' + STV.compat + '</b>') : g.os === 'linux' ? 'Runs natively · <b>Linux version</b>' : 'Runs through <b>' + STV.compat + '</b> · Windows version');
      if (s.cloud && g.inst) lp.push(g.cloud === 'conflict' ? '<span class="warn"><b>Cloud save conflict.</b></span> See Cloud saves.' : g.cloud === 'off' ? 'Cloud saves off for this game' : g.cloud === 'none' ? 'No cloud saves for this game' : 'Cloud saves synced <b>' + esc(g.synced) + '</b>');
      if (g.offline) lp.push('Installed from an offline installer');
      if (g.outside) lp.push('Installed in Steam, outside GRID');
      if (pack) lp.push('From <b>' + esc(drv.name) + '</b> · ' + (pack.how === 'installer' ? 'its installer files and saves. Installing runs the installer again.' : 'a copy of an installed game, with its saves. Installing copies it back.'));
      else if (s.id === 'gog') { var on = readyDrives().filter(function (d) { return sPackOf(d, g); })[0]; if (on) lp.push('On <b>' + esc(on.name) + '</b>'); }
      else lp.push(SNOBK[s.id]);
      var cv = scover(g, { k: 'dc' }).replace('data-f ', '').replace('data-a="sdetails"', '').replace(/ data-xl="[^"]*" data-xa="squick"/, '');
      return '<div class="det"><div class="det-l">' + cv + '<div class="lp">' + lp.join('<br>') + '</div></div>' +
        '<div class="det-r"><h1>' + esc(g.t) + '</h1><div class="hl">' + (pack ? 'From ' + esc(drv.name) + ' · ' : '') + esc(s.name) + ' · 2022 · Lantern Works · Adventure</div>' +
        '<div class="chips"><span class="chip">' + smark(s) + '<b>' + esc(s.name) + '</b></span>' + (g.inst ? '<span class="chip"><b>Installed</b></span>' : '') + (g.inst && g.ver ? '<span class="chip">v' + g.ver + '</span>' : '') + (g.upd && g.inst ? '<span class="chip warn">Update available</span>' : '') + (g.offline ? '<span class="chip">Offline installer</span>' : '') + (g.ns ? '<span class="chip warn">Not supported</span>' : '') + '</div>' +
        '<div class="btns" data-nav="h">' + btn.join('') + '</div>' +
        '<div class="ttabs">' + gl('LB') + tabs.map(function (t) { return '<button class="ttab' + (t === S.sdTab ? ' on' : '') + '">' + t + '</button>'; }).join('') + gl('RB') + '</div>' +
        '<div class="tbody" id="tbody">' + sBody(g) + '</div></div></div>';
    },
    after: function () { hero({ c: g.c }); },
    hints: function (el) { return el && el.classList.contains('prose') ? [['DPAD', 'Read']] : [['RS', 'Scroll']]; },
    lb: function () { sTabStep(g, -1); }, rb: function () { sTabStep(g, 1); }
  };
}
function sTabStep(g, d) { var t = sTabs(g); S.sdTab = t[(t.indexOf(S.sdTab) + d + t.length) % t.length]; renderStage(); var tb = $('tbody'); if (!tb) return; tb.scrollTop = 0; var f = focusables(tb)[0] || focusables(document.querySelector('.det-r .btns'))[0]; if (f) { INSTANT = true; setFocus(f); INSTANT = false; } }
function sBody(g) {
  var s = sst(g.s);
  if (S.sdTab === 'Overview') {
    var dlc = g.dlc ? '<div class="sub-h">DLC you own<span>' + g.dlc.length + '</span></div><div data-nav="v">' + g.dlc.map(function (x, i) {
      return '<div class="lrow" data-f data-k="dlc' + i + '" data-a="sdlc" data-sid="' + g.id + '" data-i="' + i + '" data-al="' + (g.inst ? (x[2] ? 'Remove' : 'Install') : 'Select') + '"><div class="grow"><div class="t">' + esc(x[0]) + '</div><div class="s">' + x[1] + (x[2] ? ' · Installed' : '') + '</div></div></div>';
    }).join('') + '</div>' : '';
    return '<div class="ov"><div><div class="prose" data-f data-k="prose" data-al="Read" tabindex="-1"><p>' + esc(g.t) + ' is a placeholder title. The real page shows the store\'s own description here.</p><p>Related games aren\'t shown for store games.</p></div>' + dlc + '</div>' +
      '<dl class="facts"><div><dt>Store</dt><dd>' + esc(s.name) + '</dd></div><div><dt>Developer</dt><dd>Lantern Works</dd></div><div><dt>Release</dt><dd>2022</dd></div><div><dt>Size</dt><dd>' + g.size + '</dd></div>' + (g.inst && g.ver ? '<div><dt>Version</dt><dd>v' + g.ver + (g.upd ? ' · ' + esc(s.name) + ' v' + g.upd : '') + '</dd></div>' : '') + '</dl></div>';
  }
  if (S.sdTab === 'Media') return '<div class="sub-h">Screenshots<span>4</span></div><div class="mtrack" data-nav="h">' + [0, 1, 2, 3].map(function (i) { return '<button class="mtile" data-f data-k="sm' + i + '" data-a="viewer" data-i="' + i + '" data-al="View" style="--c:' + g.c + ';width:14.2rem;filter:hue-rotate(' + i * 25 + 'deg)"></button>'; }).join('') + '</div><p class="note">From ' + esc(s.name) + '\'s store page.</p>';
  var path = g.os === 'linux' ? '~/.local/share/' + g.t.replace(/[^A-Za-z]+/g, '') + '/saves' : s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '') + '/pfx/drive_c/users/steamuser/Saved Games/' + g.t;
  var rows;
  if (g.cloud === 'conflict') rows = '<p class="note warn">Both this computer\'s save and the one in your ' + esc(s.name) + ' account changed since the last sync. Pick the one to keep; the other is replaced.</p>' +
    '<div class="lrow warnrow" data-f data-k="ck-l" data-a="skeep" data-v="local" data-sid="' + g.id + '" data-al="Keep it"><div class="grow"><div class="t">Keep this computer\'s save</div><div class="s">Changed today, 21:40 · 2.1 MB · uploads it to ' + esc(s.name) + '</div></div></div>' +
    '<div class="lrow" data-f data-k="ck-c" data-a="skeep" data-v="cloud" data-sid="' + g.id + '" data-al="Use it"><div class="grow"><div class="t">Use the cloud save</div><div class="s">Changed today, 19:12 on another computer · 2.0 MB · replaces the save here</div></div></div>';
  else if (g.cloud === 'off') rows = '<div class="lrow" data-f data-k="ck-o" data-a="ssettings" data-sid="' + g.id + '" data-al="Open"><div class="grow"><div class="t">Cloud saves are off for this game</div><div class="s">Turn them on in Game settings</div></div></div>';
  else if (g.cloud === 'none') rows = '<p class="note">' + esc(s.name) + ' doesn\'t keep saves for this game.</p>';
  else rows = '<div class="lrow" data-f data-k="ck-s" data-a="ssync" data-sid="' + g.id + '" data-al="Sync now"><div class="grow"><div class="t">' + (STV.syncing === g.id ? 'Syncing…' : 'Synced ' + esc(g.synced)) + '</div><div class="s">GRID syncs with ' + esc(s.name) + ' before the game starts and after it closes</div></div><span class="muted">Sync now</span></div>';
  return '<div data-nav="v">' + rows + '</div><div class="sub-h">Save location</div><p class="note" style="font-family:ui-monospace,monospace;font-size:.85rem">' + esc(path) + '</p><p class="note">RomM cloud saves are off for store games.</p>';
}
function splaying(g) {
  var s = sst(g.s), t0 = Date.now();
  return {
    title: 'Playing', render: function () {
      return '<div class="playing"><div class="box"><div class="lbl">NOW PLAYING</div><div class="logo">' + esc(g.t) + '</div><div class="muted">' + esc(s.name) + ' · ' + (s.id === 'steam' ? 'running in Steam' : g.os === 'linux' ? 'Native' : STV.compat) + ' · <span id="ses">0:00</span></div>' +
        (s.cloud && g.cloud === 'synced' ? '<p class="note" style="margin:1.2rem auto 0">Your saves sync with ' + esc(s.name) + ' when the game closes.</p>' : '') + (s.id === 'steam' ? '<p class="note" style="margin:1.2rem auto 0">Steam runs this game. Quit it from the game; GRID notices when it closes.</p>' : '') +
        '<div class="btns"><button class="btn" data-f data-a="resume" data-autofocus>Back to the game</button>' + (s.id === 'steam' ? '' : '<button class="btn dng" data-f data-a="sstop" data-sid="' + g.id + '">Stop game</button>') + '</div></div></div>';
    },
    after: function () { hero({ c: g.c }); clearInterval(S.sesT); S.sesT = setInterval(function () { var e = $('ses'); if (!e) return clearInterval(S.sesT); var x = Math.floor((Date.now() - t0) / 1000); e.textContent = Math.floor(x / 60) + ':' + ('0' + x % 60).slice(-2); }, 1000); },
    onClose: function () { clearInterval(S.sesT); }
  };
}
function sPlay(g) {
  var s = sst(g.s);
  if (s.id === 'steam') { toast('Starting ' + g.t + ' through Steam…'); g.played = 'today'; push(splaying(g)); return; }
  if (g.cloud === 'conflict') {
    openSheet({ title: 'Which save should ' + g.t + ' use?', sub: 'Both changed since the last sync. The game starts after you pick.', items: [
      { label: 'Keep this computer\'s save', sub: 'Changed today, 21:40 · uploads it to ' + s.name, fn: function () { g.cloud = 'synced'; g.synced = 'just now'; toast('Kept this computer\'s save for ' + g.t, { s: 'Uploaded to ' + s.name }); sPlay(g); } },
      { label: 'Use the cloud save', sub: 'Changed today, 19:12 on another computer', fn: function () { g.cloud = 'synced'; g.synced = 'just now'; toast('Restored the cloud save for ' + g.t); sPlay(g); } },
      { label: 'Don\'t launch', fn: function () {} }] });
    return;
  }
  toast('Launching ' + g.t + '…', s.cloud && g.cloud === 'synced' ? { s: 'Synced cloud saves with ' + s.name } : {});
  g.played = 'today'; push(splaying(g));
}

/* ---- jobs: store installs run in Downloads beside RomM's, as their own rows ---- */
function sQueue(g, kind, o) {
  o = o || {};
  var busy = DL.some(function (d) { return d.st === 'sactive' && d.skind !== 'steam'; });
  var d = { id: 's' + Date.now() + Math.random(), sgid: g ? g.id : null, t: o.t || g.t, store: o.store || g.s, skind: kind, kind: SKIND[kind], st: kind === 'steam' || kind === 'tool' || !busy ? 'sactive' : 'squeued', pct: 0, os: o.os, needsWiz: o.needsWiz, how: o.how, from: o.from, dname: o.dname };
  DL.push(d); repaint(); return d;
}
function sLine(d) {
  if (d.store === 'romm') return riLine(d);
  if (d.st === 'squeued') return 'Waiting for a slot';
  if (d.skind === 'restore') return d.how === 'installer' ? (d.pct < 45 ? 'Copying the installer from ' + d.from + ' ' + d.pct + '%' : 'Running the installer · ' + d.pct + '%') : 'Copying from ' + d.from + ' ' + d.pct + '%';
  if (d.skind === 'steam') return 'Installing in Steam ' + d.pct + '% · Managed by Steam';
  if (d.skind === 'tool') return 'Downloading ' + d.pct + '% · checking its SHA-256';
  if (d.skind === 'repair') return 'Verifying files ' + d.pct + '%';
  if (d.skind === 'installer') return d.os === 'windows' ? 'Running the installer · ' + d.pct + '%' : 'Unpacking the game files ' + d.pct + '%';
  return d.pct < 85 ? 'Downloading · ' + d.pct + '% · 41.2 MB/s' : 'Installing ' + d.pct + '%';
}
function sFinish(d) {
  var g = d.sgid && sG(d.sgid), s = sst(d.store);
  d.st = 'done';
  if (d.skind === 'restore') { g.inst = 1; g.os = d.os; var pk = DRIVES.map(function (x) { return sPackOf(x, g); }).filter(Boolean)[0]; g.ver = pk && pk.ver || g.ver || '1.0'; if (d.how === 'installer') g.offline = 1; if (!g.cloud) { g.cloud = 'synced'; g.synced = 'just now'; } d.line = d.how === 'installer' ? 'Installed from ' + d.from + ' · ran its installer · saves restored' : 'Copied from ' + d.from + ' · saves restored'; toast(g.t + ' installed from ' + d.dname, { act: ['Play', function () { sPlay(g); }] }); return; }
  if (d.skind === 'tool') { s.tool = 'ok'; d.line = 'Installed · ' + s.toolVer; toast(s.toolName + ' installed'); sLoad(s); }
  else if (d.skind === 'update') { g.ver = g.upd; delete g.upd; d.line = 'Updated to ' + g.ver; toast(g.t + ' updated to ' + g.ver); }
  else if (d.skind === 'repair') { d.line = 'Verified · 2 files repaired'; toast(g.t + ': 2 damaged files were downloaded again'); }
  else if (d.skind === 'dlc') { d.line = 'Installed'; (g.dlc || []).forEach(function (x) { if (x[0] === d.t.replace(g.t + ' — ', '')) x[2] = 1; }); toast(d.t + ' installed'); }
  else { g.inst = 1; g.os = d.os || (g.builds || [])[0]; g.ver = g.ver || '1.0'; if (d.skind === 'installer') g.offline = 1; if (s.cloud && !g.cloud) { g.cloud = 'synced'; g.synced = 'just now'; } d.line = d.skind === 'steam' ? 'Installed by Steam' : d.skind === 'installer' ? 'Installed from its installer' : 'Installed · ' + g.size; toast(g.t + ' installed', { act: ['Play', function () { sPlay(g); }] }); }
}
setInterval(function () {
  if (typeof riTvTick === 'function') riTvTick();
  var act = DL.filter(function (d) { return d.st === 'sactive'; }); if (!act.length && !DL.some(function (d) { return d.st === 'squeued'; })) return;
  var done = false;
  act.forEach(function (d) { if (d.needsWiz && d.pct >= 50) { sSilentFailed(d); done = true; return; } d.pct = Math.min(100, d.pct + (d.skind === 'tool' ? 25 : d.skind === 'steam' ? 1 : 2) + Math.floor(Math.random() * 3)); if (d.pct >= 100) { sFinish(d); done = true; } });
  if (!DL.some(function (d) { return d.st === 'sactive' && d.skind !== 'steam'; })) { var q = DL.filter(function (d) { return d.st === 'squeued'; })[0]; if (q) { q.st = 'sactive'; done = true; } }
  if (S.layers.length) { if (done) S.dirty = true; return; }
  if (done || (S.tab === 'downloads' && !S.pages.length) || (S.pages.length && S.pages[S.pages.length - 1].sid)) { repaint(); return; }
  act.forEach(function (d) { document.querySelectorAll('.cv[data-sid="' + d.sgid + '"]').forEach(function (c) { var t = c.querySelector('.tag.q'), b = c.querySelector('.sprog b'); if (t) t.textContent = d.pct + '%'; if (b) b.style.width = d.pct + '%'; }); });
}, 900);
/* Downloads: the RomM rows as before, plus store rows with the store's mark. */
var tvsDlRender = TABS.downloads.render;
TABS.downloads.render = function () {
  var keep = DL; var rom = DL.filter(function (d) { return !d.store; });
  DL = rom; var base = tvsDlRender(); DL = keep;
  var srow = function (d) {
    var romm = d.store === 'romm', s = romm ? null : sst(d.store), live = d.st === 'sactive' || d.st === 'active', steam = d.skind === 'steam';
    var line = d.st === 'done' || d.st === 'failed' || d.st === 'cancelled' ? d.line : sLine(d);
    if (romm) line = PLAT[d.p] + ' · ' + line;
    return '<div class="lrow dl" data-f data-k="dl-' + d.id + '" data-a="dlrow" data-id="' + d.id + '" data-al="' + (steam && live ? 'Open' : live || d.st === 'squeued' ? 'Cancel' : d.st === 'failed' ? 'Retry or dismiss' : 'Dismiss') + '"><div><div class="t">' + esc(d.t) + (s ? '<span class="kind" style="display:inline-flex;align-items:center;gap:.3rem">' + smark(s) + esc(s.name) + '</span>' : '') + (d.kind ? '<span class="kind">' + d.kind + '</span>' : '') + '</div><div class="s' + (d.st === 'failed' ? ' err' : '') + '">' + esc(line) + '</div>' + (live ? '<div class="pbar"><b style="width:' + d.pct + '%"></b></div>' : '') + '</div>' +
      (live ? '<div class="spark"><svg viewBox="0 0 120 38" preserveAspectRatio="none"><polyline fill="none" stroke="var(--primary)" stroke-width="2" points="0,30 20,22 40,25 60,14 80,18 100,10 120,16"/></svg></div>' : '<div></div>') + '<div class="s">' + (live && !steam ? '41.2 MB/s' : '') + '</div></div>';
  };
  var segs = { Active: ['sactive', 'active'], Queued: ['squeued', 'queued'], Completed: ['done', 'failed', 'cancelled'] };
  var el = document.createElement('div'); el.innerHTML = base;
  el.querySelectorAll('section.panel').forEach(function (sec) {
    var name = sec.querySelector('h2').firstChild.textContent, L = DL.filter(function (d) { return d.store && segs[name].indexOf(d.st) >= 0; });
    if (!L.length) return;
    var box = sec.querySelector('[data-nav]'), note = box.querySelector('.note'); if (note) note.remove();
    box.insertAdjacentHTML(name === 'Completed' ? 'afterbegin' : 'beforeend', L.map(srow).join(''));
    var n = sec.querySelector('h2 span'); n.textContent = +n.textContent + L.length;
  });
  return el.innerHTML;
};
var tvsDlrow = ACT.dlrow;
ACT.dlrow = function (el) {
  var d = DL.filter(function (x) { return x.id === el.dataset.id; })[0];
  if (d && d.silentFail && d.store === 'romm') {
    openSheet({ title: d.t, sub: PLAT[d.p], items: [
      { label: 'Retry', fn: function () { sRetrySilent(d); } },
      { label: 'Delete installer', danger: 1, fn: function () { sDeleteInstaller(d); } },
      { label: 'Dismiss', fn: function () { DL.splice(DL.indexOf(d), 1); renderStage(); restoreFocus(); } },
      { label: 'Open ' + d.t, fn: function () { push(details(byId(d.gid))); } }] });
    return;
  }
  if (!d || !d.store || d.store === 'romm') return tvsDlrow(el);
  var s = sst(d.store), items = [], g = d.sgid && sG(d.sgid);
  if ((d.st === 'sactive' || d.st === 'squeued') && d.skind !== 'steam') items.push({ label: 'Cancel', danger: 1, fn: function () { d.st = 'cancelled'; d.line = 'Cancelled'; if (d.skind === 'tool') s.tool = 'missing'; toast('Cancelled ' + d.t); renderStage(); } });
  if (d.st === 'failed' || d.st === 'cancelled') items.push({ label: 'Retry', fn: function () { d.st = 'squeued'; d.pct = 0; d.silentFail = 0; toast(d.t + ' added to download queue'); renderStage(); } });
  if (d.silentFail) items.push({ label: 'Delete installer', danger: 1, fn: function () { sDeleteInstaller(d); } });
  if (d.st === 'done' || d.st === 'failed' || d.st === 'cancelled') items.push({ label: 'Dismiss', fn: function () { DL.splice(DL.indexOf(d), 1); renderStage(); restoreFocus(); } });
  if (g) items.push({ label: 'Open ' + g.t, fn: function () { push(sdetails(g)); } });
  openSheet({ title: d.t, sub: s.name + (d.kind ? ' · ' + d.kind : '') + (d.skind === 'steam' ? ' · Managed by Steam: pause or cancel it in Steam' : ''), items: items });
};
var tvsDlHints = TABS.downloads.hints;
TABS.downloads.hints = function () { return DL.some(function (d) { return d.st === 'done' || d.st === 'failed' || d.st === 'cancelled'; }) ? [['Y', 'Clear completed']] : []; };
TABS.downloads.y = function () { DL = DL.filter(function (d) { return d.st === 'active' || d.st === 'queued' || d.st === 'sactive' || d.st === 'squeued'; }); render(); };

/* ---- the install page ---- */
function sInstallPage(g) {
  var s = sst(g.s), F = { os: g.builds.indexOf('linux') >= 0 ? 'linux' : 'windows', compat: STV.compat, lang: (g.langs || ['English'])[0], dlc: (g.dlc || []).map(function () { return true; }), folder: s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '') };
  push(rowsPage({
    title: 'Install ' + g.t, key: 'si',
    h1: function () { return 'Install ' + esc(g.t); }, sub: function () { return esc(s.name) + ' · ' + g.size + ' download'; },
    rows: function () {
      var R = [];
      if (g.builds.length > 1) { R.push({ heading: 'Version' }); R.push({ label: 'Linux', sub: 'Runs natively on this computer · recommended', radio: F.os === 'linux', fn: function () { F.os = 'linux'; repaint(); } }); R.push({ label: 'Windows', sub: 'Runs through ' + F.compat, radio: F.os === 'windows', fn: function () { F.os = 'windows'; repaint(); } }); }
      R.push({ heading: 'Options' });
      if (F.os === 'windows') R.push({ label: 'Compatibility tool', sub: 'The default from Settings › Systems › Windows', val: esc(F.compat), fn: function () { openSheet({ title: 'Compatibility tool', items: ['GE-Proton9-20', 'Proton 9.0 (Steam)', 'System Wine'].map(function (t) { return { label: t, checked: F.compat === t, fn: function () { F.compat = t; repaint(); } }; }) }); } });
      if (g.langs) R.push({ label: 'Language', val: esc(F.lang), fn: function () { openSheet({ title: 'Language', items: g.langs.map(function (l) { return { label: l, checked: F.lang === l, fn: function () { F.lang = l; repaint(); } }; }) }); } });
      R.push({ label: 'Install folder', sub: 'The default for ' + esc(s.name) + ' games is in Settings › Stores', val: esc(F.folder), fn: function () { openBrowser({ title: 'Install ' + g.t + ' in', start: '/home/sam/Games', onPick: function (p) { F.folder = p + '/' + g.t.replace(/[^A-Za-z0-9]+/g, ''); } }); } });
      if (g.dlc) { R.push({ heading: 'DLC you own' }); g.dlc.forEach(function (x, i) { R.push({ label: esc(x[0]), sub: x[1], toggle: F.dlc[i], fn: function () { F.dlc[i] = !F.dlc[i]; repaint(); } }); }); }
      return R;
    },
    main: function () { return { label: 'Install', summary: g.size + ' to download · 412 GB free on /home', fn: function () { pop(); sQueue(g, 'install', { os: F.os }); (g.dlc || []).forEach(function (x, i) { if (F.dlc[i]) x[2] = 1; }); toast(g.t + ' added to download queue', { act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] }); } }; },
    mainHint: 'Install'
  }));
}
function sGameSettings(g) {
  var s = sst(g.s), win = g.os === 'windows', F = { exe: win ? g.t.replace(/[^A-Za-z]+/g, '') + '.exe' : 'start.sh', args: '', compat: STV.compat, mine: false };
  push(rowsPage({
    title: 'Game settings', key: 'sg', h1: function () { return esc(g.t); }, sub: function () { return 'Game settings'; },
    rows: function () {
      var R = [{ info: 1, label: 'Install folder', val: esc(s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '')) },
        { label: 'Launch file', sub: F.mine ? 'Your choice' : 'Set by ' + esc(s.name), val: esc(F.exe), fn: function () { openSheet({ title: 'Launch file', items: (win ? [F.exe, 'Launcher.exe'] : ['start.sh', 'game/run']).map(function (x) { return { label: x, checked: x === F.exe, fn: function () { F.exe = x; F.mine = true; repaint(); } }; }) }); } },
        { label: 'Launch parameters', sub: 'Optional', val: esc(F.args || 'None'), fn: function () { openOSK({ title: 'Launch parameters', value: F.args, onDone: function (v) { F.args = v; repaint(); } }); } }];
      if (win) { R.push({ label: 'Compatibility tool', val: esc(F.compat), fn: function () { openSheet({ title: 'Compatibility tool', items: ['GE-Proton9-20', 'Proton 9.0 (Steam)', 'System Wine', 'None'].map(function (t) { return { label: t, checked: F.compat === t, fn: function () { F.compat = t; repaint(); } }; }) }); } }); R.push({ info: 1, label: 'Wine prefix', sub: 'Read-only', val: esc(s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '') + '/pfx') }); }
      else R.push({ note: 'Linux games run directly.' });
      if (s.cloud && g.cloud !== 'none') R.push({ label: 'Cloud saves', sub: 'Sync this game\'s saves with ' + esc(s.name), toggle: g.cloud !== 'off', fn: function () { g.cloud = g.cloud === 'off' ? 'synced' : 'off'; g.synced = g.synced || 'just now'; repaint(); } });
      return R;
    },
    main: function () { return { label: 'Save', fn: function () { toast('Game settings saved'); pop(); } }; }, mainHint: 'Save'
  }));
}

/* ---- Settings › Stores ---- */
PANES.splice(PANES.map(function (p) { return p[0]; }).indexOf('systems'), 0, ['stores', 'Stores']);
var tvsSetPaint = setPaint;
setPaint = function () {
  var pane = $('pane'); if (!pane || S.setPane !== 'stores') return tvsSetPaint();
  pane.innerHTML = '<div class="pane-h"><h1>Stores</h1></div><p class="note" style="margin-left:1.2rem">Each connected store gets a group in the Library with its Installed and Available games.</p>' + STV.order.map(function (id) {
    var s = sst(id), st = s.conn === 'ok' ? (id === 'steam' && s.key !== 'ok' ? '<span class="st warn">Installed games only</span>' : '<span class="st ok">Connected</span>') : s.conn === 'expired' ? '<span class="st warn">Login expired</span>' : '<span class="st">Not connected</span>';
    var sub = s.conn === 'off' ? (id === 'steam' ? 'Not set up' : 'Not signed in') : id === 'steam' ? 'SteamID64 ' + s.steamId : 'Signed in as ' + s.account + (s.tool !== 'ok' ? ' · ' + s.toolName + ' not installed' : '');
    return '<div class="srow" data-f data-k="sto-' + id + '" data-a="sstorepage" data-id="' + id + '" data-al="Open"><span style="margin-right:.2rem">' + smark(s, 'lg') + '</span><div class="lab"><div class="t">' + esc(s.name) + '</div><div class="s">' + esc(sub) + '</div></div><div class="val">' + st + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>';
  }).join('');
};
function sStorePage(s) {
  if (s.id === 'steam') return sSteamPage(s);
  push(rowsPage({
    title: s.name, key: 'sp', h1: function () { return smark(s, 'lg') + ' ' + esc(s.name); },
    sub: function () { return s.conn === 'off' ? 'Not signed in' : 'Signed in as <b>' + esc(s.account) + '</b>'; },
    rows: function () {
      var R = [];
      if (s.conn === 'off') {
        R.push({ note: 'Sign in to ' + esc(s.name) + ' to see the games you own in the Library and install them. GRID installs with ' + s.toolName + ' (' + s.toolPkg + ', ' + s.license + ', ' + s.toolSize + '), which it downloads when you connect.' + (s.id === 'gog' ? ' You can install GOG offline installers without signing in.' : '') });
        if (s.id === 'gog') R.push({ label: 'Install from installer…', sub: 'A setup_….exe or ….sh you downloaded', fn: sOpenInstaller });
        return R;
      }
      if (s.conn === 'expired') R.push({ note: '<b>Your ' + esc(s.name) + ' login expired.</b> Sign in again to see new games, install and update. Installed games still play.', warn: 1 });
      R.push({ info: 1, label: 'Games you own', val: sOf(s.id).length + ' · ' + sInst(s).length + ' installed · updated ' + s.refreshed });
      if (s.tool === 'ok') R.push({ info: 1, label: 'Store tool', val: s.toolName + ' ' + s.toolVer + ' · installed' });
      else R.push({ label: s.tool === 'failed' ? 'Couldn\'t install ' + s.toolName : 'Store tool', sub: s.tool === 'failed' ? 'The download didn\'t match its checksum, so GRID deleted it' : s.tool === 'installing' ? 'Installing…' : 'Not installed · ' + s.toolSize, val: s.tool === 'failed' ? 'Try again' : 'Install', warn: 1, fn: function () { sTool(s); } });
      R.push({ info: 1, label: 'License', sub: s.src + ' (opens in desktop mode)', val: s.toolPkg + ' · ' + s.license });
      R.push({ label: 'Install folder', sub: 'New ' + esc(s.name) + ' installs go here', val: esc(s.folder), fn: function () { openBrowser({ title: 'Folder for new ' + s.name + ' games', start: '/home/sam/Games', onPick: function (p) { s.folder = p; toast('New ' + s.name + ' games install in ' + p); } }); } });
      if (s.cloud) R.push({ label: 'Cloud saves', sub: 'Sync before a game starts and after it closes', toggle: s.cloudOff ? false : true, fn: function () { s.cloudOff = !s.cloudOff; toast(s.cloudOff ? 'Cloud saves off for ' + s.name + ' games' : 'Cloud saves on for ' + s.name + ' games'); repaint(); } });
      R.push({ label: 'Show in Library', fn: function () { S.pages = []; S.tab = 'library'; S.libRail = 'st:' + s.id + ':inst'; S.folds['st-' + s.id] = true; TABS.library._k = 'r-st:' + s.id + ':inst'; render(); } });
      if (s.id === 'gog') R.push({ label: 'Install from installer…', sub: 'A setup_….exe or ….sh you downloaded', fn: sOpenInstaller });
      R.push({ label: 'Refresh games', fn: function () { sLoad(s); } });
      R.push({ label: 'Disconnect…', danger: 1, fn: function () { confirmBox({ title: 'Disconnect ' + s.name + '?', body: 'GRID deletes your ' + s.name + ' login and ' + s.toolName + ' from this computer. ' + (sInst(s).length ? 'The ' + sInst(s).length + ' installed games stay and still play; they can\'t install, update' + (s.cloud ? ' or sync cloud saves' : '') + ' until you connect again.' : ''), ok: 'Disconnect', danger: 1, fn: function () { s.conn = 'off'; s.tool = 'missing'; toast(s.name + ' disconnected', { s: 'Its login and ' + s.toolName + ' were deleted; installed games stay.' }); pop(); } }); } });
      return R;
    },
    main: function () { return s.conn === 'ok' ? null : { label: s.conn === 'off' ? 'Connect ' + s.name + '…' : 'Sign in again…', fn: function () { sLogin(s); } }; },
    mainHint: 'Sign in'
  }));
}
function sSteamPage(s) {
  var F = { pick: s.steamId, manual: '', key: '' };
  var res = ['ok', 'bad', 'private', 'sid', 'net'];
  push(rowsPage({
    title: 'Steam', key: 'ss', h1: function () { return smark(s, 'lg') + ' Steam'; },
    sub: function () { return 'Installed Steam games show as soon as Steam is set up here, including games you install in Steam itself. To list the games you own but haven\'t installed, GRID needs your SteamID64 and your own Web API key.'; },
    rows: function () {
      var acct = s.accounts.filter(function (a) { return a[1] === F.pick; })[0];
      var R = [{ info: 1, label: 'Steam client', val: s.client ? 'Found · /usr/bin/steam' : '<span class="st warn">Not installed</span>', sub: s.client ? '' : 'Install Steam on this PC and sign in once, then come back here' },
        { label: 'Account', sub: 'Found in Steam\'s files on this computer', val: acct ? esc(acct[0]) + ' · ' + acct[1] : esc(F.manual || 'Enter a SteamID64'), fn: function () {
          openSheet({ title: 'Steam account', items: s.accounts.map(function (a) { return { label: a[0] + ' · ' + a[1], sub: a[2], checked: F.pick === a[1], fn: function () { F.pick = a[1]; repaint(); } }; }).concat([{ label: 'Enter a SteamID64…', sub: '17 digits, starting 7656', checked: F.pick === 'manual', fn: function () { openOSK({ title: 'SteamID64', value: F.manual, onDone: function (v) { F.pick = 'manual'; F.manual = v; repaint(); } }); } }]) }); } },
        { label: 'Web API key', sub: 'From steamcommunity.com/dev/apikey (any domain name works). Kept in the OS keyring, sent only to Steam.', val: s.key === 'ok' && !F.key ? '••••••••3F2A' : F.key ? '••••••••' + F.key.slice(-4) : '<i>Not set</i>', fn: function () { openOSK({ title: 'Steam Web API key', mask: 1, onDone: function (v) { F.key = v; repaint(); } }); } },
        { label: 'Test', sub: 'Prototype: the next test says “' + ['the key works', 'rejected', 'profile private', 'not a SteamID64', 'no network'][STV.testIdx] + '”', fn: function () { sSteamTest(false); } }];
      if (s.conn !== 'off') R.push({ label: 'Show in Library', fn: function () { S.pages = []; S.tab = 'library'; S.libRail = 'st:steam:inst'; TABS.library._k = 'r-st:steam:inst'; render(); } });
      if (s.conn !== 'off') R.push({ label: 'Disconnect…', danger: 1, fn: function () { confirmBox({ title: 'Disconnect Steam?', body: 'GRID forgets your SteamID64 and deletes your Web API key from the OS keyring. Steam games leave the Library; they stay installed in Steam.', ok: 'Disconnect', danger: 1, fn: function () { s.conn = 'off'; s.key = 'none'; sOf('steam').forEach(function (g) { g.inst = 0; }); toast('Steam disconnected', { s: 'Your key was deleted from the keyring' }); pop(); } }); } });
      return R;
    },
    main: function () { return { label: s.conn === 'off' ? 'Connect Steam' : 'Save', fn: function () { sSteamTest(true); } }; }, mainHint: 'Save'
  }));
  function sSteamTest(save) {
    var r = res[STV.testIdx]; STV.testIdx = (STV.testIdx + 1) % res.length;
    toast('Asking Steam…');
    setTimeout(function () {
      if (r === 'ok') toast('The key works', { s: sOf('steam').length + ' games owned' });
      if (r === 'bad') toast('Steam rejected the key (403 Forbidden)', { err: 1, s: 'Check that you entered all 32 characters, or make a new key.' });
      if (r === 'private') toast('This profile\'s game details are private', { err: 1, s: 'In Steam: your profile › Edit Profile › Privacy Settings › Game details: Public. Then test again.' });
      if (r === 'sid') toast('That isn\'t a SteamID64', { err: 1, s: 'It has 17 digits and starts with 7656.' });
      if (r === 'net') toast('Couldn\'t reach api.steampowered.com', { err: 1, s: 'Check your connection and try again.' });
      if (save && (r === 'ok' || r === 'private')) { s.conn = 'ok'; s.key = r === 'ok' ? 'ok' : 'private'; toast(r === 'ok' ? 'Steam connected' : 'Steam connected, but Available is empty until game details are public', r === 'ok' ? {} : { err: 1 }); }
      repaint();
    }, 700);
  }
}
function sTool(s) { s.tool = 'installing'; sQueue(null, 'tool', { t: s.toolName, store: s.id }); toast('Installing ' + s.toolName + ' (' + s.toolSize + ') for ' + s.name + '…'); repaint(); }
function sLoad(s) { s.loading = true; repaint(); setTimeout(function () { s.loading = false; s.refreshed = 'just now'; toast(s.name + ': ' + sOf(s.id).length + ' games found', { s: sAvail(s).length + ' to install' }); repaint(); }, 1800); }

/* ---- the store login window, over TV mode ---- */
function sLogin(s) {
  var L, F = { email: '', pw: '', phase: 'loading', err: '' };
  function html() {
    var fld = function (k, lab, v, mask) { return '<button class="field" data-f data-k="lf-' + k + '" data-a="slfield" data-key="' + k + '" data-al="Type"><span class="v' + (v ? '' : ' ph') + '">' + (v ? (mask ? v.replace(/./g, '•') : esc(v)) : lab) + '</span></button>'; };
    var page = F.phase === 'loading' ? '<div class="swin-ph"><span><span class="sspin"></span> Loading ' + esc(s.host.split('/')[0]) + '…</span></div>'
      : F.phase === 'done' ? '<div class="swin-ph"><b>Signed in.</b> Closing…</div>'
      : '<div class="swin-ph" data-nav="v">' + (F.err ? '<p class="swin-err">' + esc(F.err) + '</p>' : '') + '<b>' + esc(s.name) + ' sign-in page</b><span>The store\'s page loads here, as in a browser. Move between its fields with the d-pad; ' + gl('Y') + ' opens the keyboard. GRID never sees what you type.</span>' +
        fld('email', '(the store\'s email field)', F.email) + fld('pw', '(the store\'s password field)', F.pw, 1) + '<button class="btn pri" data-f data-k="lf-go" data-a="slok" data-al="Select">(the store\'s sign-in button)</button></div>';
    return '<div class="swin" role="dialog" aria-label="Sign in to ' + esc(s.name) + '"><div class="swin-bar">' + smark(s) + 'Sign in to ' + esc(s.name) + ' — GRID</div><div class="swin-url"><span class="lock">🔒</span> https://' + esc(s.host) + '</div><div class="swin-page">' + page + '</div>' +
      '<div class="swin-foot" data-nav="h"><span class="note">GRID never sees your password. This window closes on its own when you\'ve signed in.</span><button class="btn proto" data-f data-k="lf-fail" data-a="slfail" data-al="Select">Prototype: fail</button><button class="btn" data-f data-k="lf-x" data-a="slx" data-al="Cancel">Cancel</button></div></div>';
  }
  L = openLayer({ html: html(), cls: 'slogin', slogin: true, F: F, s: s, hints: function () { return [['A', 'Select'], ['Y', 'Keyboard'], ['B', 'Cancel sign-in']]; } });
  L.draw = function () { var k = cur && cur.dataset.k; L.el.innerHTML = html(); var el = (k && L.el.querySelector('[data-k="' + k + '"]')) || L.el.querySelector('.swin-page [data-f]') || L.el.querySelector('[data-f]'); setFocus(el, true); paintHints(); };
  L.type = function (k) { openOSK({ title: k === 'pw' ? s.name + ' password (typed into the store\'s page)' : s.name + ' email (typed into the store\'s page)', value: k === 'pw' ? '' : F[k], mask: k === 'pw', onDone: function (v) { F[k] = v; L.draw(); } }); };
  setTimeout(function () { if (F.phase === 'loading') { F.phase = 'page'; L.draw(); setFocus(L.el.querySelector('[data-k="lf-email"]'), true); paintHints(); } }, 700);
}
function sLoginTop() { var t = S.layers[S.layers.length - 1]; return t && t.slogin ? t : null; }
ACT.slfield = function (el) { var L = sLoginTop(); if (L) L.type(el.dataset.key); };
ACT.slok = function () {
  var L = sLoginTop(); if (!L) return; var s = L.s; L.F.phase = 'done'; L.draw();
  setTimeout(function () { closeLayer(); var was = s.conn; s.conn = 'ok'; toast((was === 'expired' ? 'Signed in to ' : 'Connected to ') + s.name + ' as ' + s.account); if (s.tool !== 'ok') sTool(s); else sLoad(s); repaint(); }, 600);
};
ACT.slfail = function () {
  var L = sLoginTop(); if (!L) return; STV.fail = !STV.fail;
  if (STV.fail) { L.F.err = 'Couldn\'t reach ' + L.s.host.split('/')[0] + '. Check your connection; the page reloads when you try again.'; L.draw(); }
  else { var s = L.s; closeLayer(); toast('Couldn\'t sign in to ' + s.name, { err: 1, s: s.name + ' returned an error. Try again.', act: ['Try again', function () { sLogin(s); }] }); }
};
ACT.slx = function () { if (sLoginTop()) closeLayer(); };
var tvsPress = press;
press = function (b) {
  var L = sLoginTop();
  if (L && b === 'Y') { var k = cur && cur.dataset.key; L.type(k === 'pw' ? 'pw' : 'email'); return; }
  return tvsPress(b);
};

/* ---- GOG offline installers, through the folder browser ---- */
FS['/home/sam/Downloads'] = ['paper_lanterns_1_1_0_60321.sh', 'setup_quarry_nine_1.4_(64bit)_(69001).exe', 'ashen_tide_digital_artbook_71299.sh', 'setup_marrowind_keep_1.2_(64bit)_(70555).exe', 'setup_tidewater_tales_dlc_(55120).exe', 'ashen_tide_1.0.4_71234.pkg', 'Moonrise_Setup.exe', 'manual_ashen_tide.pdf'];
function sOpenInstaller() { openBrowser({ title: 'Choose a GOG installer', start: '/home/sam/Downloads', files: true, onPick: sPicked }); }
function sRefuse(title, body) { openLayer({ html: '<div class="dialog" role="alertdialog"><h2>Can\'t install this file</h2><p><b>' + title + '</b><br>' + body + '</p><div class="btns" data-nav="h"><button class="btn" data-f data-ok="1">Choose another file…</button><button class="btn pri" data-f data-autofocus data-cl="1">OK</button></div></div>', ok: sOpenInstaller }); }
function sPicked(p) {
  var f = p.split('/').pop();
  toast('Reading the installer…', { s: f });
  setTimeout(function () {
    if (/\.pkg$/.test(f)) return sRefuse('This is a macOS installer.', 'GRID installs GOG\'s Linux (.sh) and Windows (.exe) installers. Download one of those from your GOG library.');
    if (/Moonrise|\.pdf$/.test(f)) return sRefuse('This isn\'t a GOG installer.', f + ' has no GOG game info in it. GRID reads GOG\'s own setup_….exe and ….sh installers.');
    if (/tidewater/.test(f)) return sRefuse('This is DLC for Tidewater Tales, which isn\'t installed.', 'Install Tidewater Tales first, from GOG or from its own installer. Then install this DLC.');
    if (/marrowind/.test(f)) return sRefuse('Part of this installer is missing.', 'It comes in 6 parts, and setup_marrowind_keep_1.2_(64bit)_(70555)-1.bin isn\'t in the same folder. Put every part beside the .exe.');
    var dlc = /artbook/.test(f), win = /\.exe$/.test(f), g = sG(dlc ? 'g1' : /quarry/.test(f) ? 'g8' : 'g7'), F = { compat: STV.compat, folder: sst('gog').folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '') };
    push(rowsPage({
      title: dlc ? 'Install DLC' : 'Install ' + g.t, key: 'oi',
      h1: function () { return dlc ? 'Install DLC for ' + esc(g.t) : 'Install ' + esc(g.t); }, sub: function () { return 'From a GOG offline installer · ' + esc(f); },
      rows: function () {
        var R = [{ info: 1, label: dlc ? 'Digital Artbook' : g.t, val: dlc ? 'DLC · 118 MB' : (win ? 'Windows version' : 'Linux version') + ' · ' + (win ? 'v1.4 · 9.4 GB' : 'v1.1.0 · 1.1 GB') }];
        R.push({ note: dlc ? 'Adds to the installed ' + esc(g.t) + ' (the Linux version). GRID unpacks its files into the game\'s folder; the installer script isn\'t run.' : win ? 'GRID runs the installer silently under ' + esc(F.compat) + '. If that fails, GRID tries once more with gentler settings. If it still can\u2019t install, GRID keeps the installer and tells you why, so you can retry or delete it.' : 'GRID unpacks the game files from it. The installer script isn\'t run.' });
        if (win) R.push({ label: 'Compatibility tool', val: esc(F.compat), fn: function () { openSheet({ title: 'Compatibility tool', items: ['GE-Proton9-20', 'Proton 9.0 (Steam)', 'System Wine'].map(function (t) { return { label: t, checked: F.compat === t, fn: function () { F.compat = t; repaint(); } }; }) }); } });
        if (!dlc) { R.push({ label: 'Install folder', val: esc(F.folder), fn: function () { openBrowser({ title: 'Install ' + g.t + ' in', start: '/home/sam/Games', onPick: function (q) { F.folder = q; } }); } }); R.push({ note: 'It joins your GOG games. Updates and cloud saves come from GOG while you\'re signed in.' }); }
        return R;
      },
      main: function () { return { label: dlc ? 'Install DLC' : 'Install', summary: 'Needs ' + (dlc ? '160 MB' : win ? '13.2 GB' : '1.6 GB') + ' · 412 GB free', fn: function () {
        pop();
        if (dlc) { sQueue(g, 'dlc', { t: g.t + ' — Digital Artbook' }); toast('Digital Artbook added to download queue'); return; }
        var d = sQueue(g, 'installer', { os: win ? 'windows' : 'linux', needsWiz: win });
        toast(g.t + ' added to download queue', { act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] });
      } }; },
      mainHint: 'Install'
    }));
  }, 700);
}

/* ---- actions ---- */
ACT.sdetails = function (el) { push(sdetails(sG(el.dataset.sid), el.dataset.drive ? driveOf(el.dataset.drive) : null)); };
ACT.squick = function (el) { var g = sG(el.dataset.sid), q = sQuick(g); if (el.dataset.drive && !g.inst) { sRestore(g, driveOf(el.dataset.drive)); return; } if (q === 'Play') sPlay(g); else if (q) ACT.sinstall(el); };
ACT.splay = function (el) { sPlay(sG(el.dataset.sid)); };
ACT.sinstall = function (el) {
  var g = sG(el.dataset.sid), s = sst(g.s);
  if (s.id === 'steam') { if (!s.client) { toast('Steam isn\'t installed', { err: 1 }); return; } sQueue(g, 'steam'); toast('Steam opened to install ' + g.t, { s: 'Choose the library folder there; GRID shows the progress. Steam\'s window opens over TV mode.' }); return; }
  sInstallPage(g);
};
ACT.scancel = function (el) { var g = sG(el.dataset.sid), d = sJob(g); confirmBox({ title: 'Cancel ' + g.t + '?', body: d.skind === 'install' ? 'The download stops and its files are removed. You can install it again later.' : 'It stops; the game stays as it was.', ok: 'Cancel ' + (d.skind === 'install' ? 'install' : d.skind), danger: 1, fn: function () { d.st = 'cancelled'; d.line = 'Cancelled'; toast('Cancelled ' + g.t); repaint(); } }); };
ACT.supdate = function (el) { var g = sG(el.dataset.sid); confirmBox({ title: 'Update ' + g.t + ' to ' + g.upd + '?', body: 'Saves and settings stay.', ok: 'Update', fn: function () { sQueue(g, 'update'); toast('Update for ' + g.t + ' added to download queue'); } }); };
ACT.ssettings = function (el) { sGameSettings(sG(el.dataset.sid)); };
ACT.sverify = function (el) { var g = sG(el.dataset.sid); if (g.s === 'steam') { toast('Steam is verifying ' + g.t, { s: 'Watch it in Steam' }); return; } sQueue(g, 'repair'); toast('Verifying ' + g.t + '\'s files…'); };
ACT.suninstall = function (el) { var g = sG(el.dataset.sid), s = sst(g.s); if (s.id === 'steam') { toast('Steam opened its uninstall window for ' + g.t, { s: 'It leaves the Library once Steam removes it.' }); return; } confirmBox({ title: 'Uninstall ' + g.t + '?', body: 'Its files are deleted from this computer.' + (s.cloud ? ' Its cloud saves stay in your ' + s.name + ' account.' : ''), ok: 'Uninstall', danger: 1, fn: function () { g.inst = 0; delete g.upd; toast(g.t + ' uninstalled'); repaint(); } }); };
ACT.sstop = function (el) { var g = sG(el.dataset.sid), s = sst(g.s); confirmBox({ title: 'Stop ' + g.t + '?', body: 'Unsaved progress in the game is lost.', ok: 'Stop game', danger: 1, fn: function () { pop(); if (s.cloud && g.cloud === 'synced') { toast('Syncing cloud saves for ' + g.t + '…'); setTimeout(function () { g.synced = 'just now'; toast('Cloud saves uploaded to ' + s.name + ' for ' + g.t); }, 1400); } } }); };
ACT.ssync = function (el) { var g = sG(el.dataset.sid); STV.syncing = g.id; repaint(); setTimeout(function () { STV.syncing = null; g.synced = 'just now'; toast('Cloud saves synced for ' + g.t); repaint(); }, 1200); };
ACT.skeep = function (el) { var g = sG(el.dataset.sid), s = sst(g.s), v = el.dataset.v; confirmBox({ title: v === 'local' ? 'Keep this computer\'s save?' : 'Use the cloud save?', body: v === 'local' ? 'It is uploaded to ' + s.name + ' and replaces the cloud save.' : 'It replaces the save on this computer.', ok: v === 'local' ? 'Keep and upload' : 'Use the cloud save', fn: function () { g.cloud = 'synced'; g.synced = 'just now'; toast(v === 'local' ? 'Kept this computer\'s save for ' + g.t : 'Restored the cloud save for ' + g.t); repaint(); } }); };
ACT.sdlc = function (el) { var g = sG(el.dataset.sid), x = g.dlc[+el.dataset.i]; if (!g.inst) return; if (x[2]) { confirmBox({ title: 'Remove ' + x[0] + '?', body: 'Its files leave ' + g.t + '\'s folder. You can install it again later.', ok: 'Remove', danger: 1, fn: function () { x[2] = 0; toast(x[0] + ' removed'); repaint(); } }); return; } sQueue(g, 'dlc', { t: g.t + ' — ' + x[0] }); toast(x[0] + ' added to download queue'); };
ACT.slogin = function (el) { sLogin(sst(el.dataset.id)); };
ACT.sinstaller = function () { sOpenInstaller(); };
ACT.srefresh = function (el) { var s = sst(el.dataset.id); if (s.id === 'steam' && s.key === 'private') { toast('Steam still lists no games', { err: 1, s: 'The profile\'s game details are private.' }); return; } sLoad(s); };
ACT.stool = function (el) { sTool(sst(el.dataset.id)); };
ACT.sgoavail = function (el) { S.libRail = 'st:' + el.dataset.id + ':avail'; TABS.library._k = 'r-' + S.libRail; render(); };
ACT.sgoset = function () { S.pages = []; S.tab = 'settings'; S.setPane = 'stores'; TABS.settings._k = 'r-stores'; render(); };
ACT.srecheck = function () { var s = sst('steam'); s.client = true; sOf('steam').forEach(function (g) { if (/s1|s2|s3/.test(g.id)) g.inst = 1; }); toast('Steam client found'); repaint(); };
ACT.sstorepage = function (el) { sStorePage(sst(el.dataset.id)); };
ACT.snop = function () {};

/* ---- prototype: store states, from the Menu's Prototype only section ---- */
var STV_BASE = JSON.parse(JSON.stringify(STV.stores));
function sStateSheet() {
  var set = function (id, fn) { return function () { var b = JSON.parse(JSON.stringify(STV_BASE[id])), s = sst(id); Object.keys(b).forEach(function (k) { s[k] = b[k]; }); delete s.loading; fn(s); repaint(); }; };
  openSheet({ title: 'Show a store state', sub: 'Prototype only', items: [
    { label: 'Every store as it started', fn: function () { STV.order.forEach(function (id) { set(id, function () {})(); }); } },
    { label: 'GOG: login expired', fn: set('gog', function (s) { s.conn = 'expired'; }) },
    { label: 'GOG: not connected (offline installs only)', fn: set('gog', function (s) { s.conn = 'off'; s.tool = 'missing'; }) },
    { label: 'Epic Games: loading owned games', fn: set('epic', function (s) { s.loading = true; }) },
    { label: 'Epic Games: legendary not installed', fn: set('epic', function (s) { s.tool = 'missing'; }) },
    { label: 'Epic Games: legendary install failed', fn: set('epic', function (s) { s.tool = 'failed'; }) },
    { label: 'Amazon Games: connected', fn: set('amazon', function (s) { s.conn = 'ok'; }) },
    { label: 'Steam: no Web API key', fn: set('steam', function (s) { s.key = 'none'; }) },
    { label: 'Steam: game details private', fn: set('steam', function (s) { s.key = 'private'; }) },
    { label: 'Steam: key rejected', fn: set('steam', function (s) { s.key = 'bad'; }) },
    { label: 'Steam: client not installed', fn: set('steam', function (s) { s.client = false; sOf('steam').forEach(function (g) { g.inst = 0; }); }) }] });
}
var tvsGuide = guideMenu;
guideMenu = function () {
  tvsGuide();
  var L = S.layers[S.layers.length - 1], box = L && L.el.querySelector('.guide'); if (!box || !L.gp) return;
  [['Show: a store state…', sStateSheet], ['Show: Settings › Stores', function () { ACT.sgoset(); }], ['Show: GOG offline installer', sOpenInstaller], ['Show: sign in to Amazon Games', function () { sLogin(sst('amazon')); }]].forEach(function (it) {
    var i = L.gp.push(it) - 1; box.insertAdjacentHTML('beforeend', '<button class="opt" data-f data-k="gp' + i + '" data-gp="' + i + '">' + esc(it[0]) + '</button>');
  });
};

/* The card opens on Library › GOG › Installed, with one GOG install running, one
   queued and a Steam install that Steam runs. */
function sBoot() {
  DL.push({ id: 'sb1', sgid: 'g4', t: 'Copper Saints', store: 'gog', skind: 'install', kind: '', st: 'sactive', pct: 38, os: 'linux' });
  DL.push({ id: 'sb2', sgid: 'g5', t: 'Hush Valley', store: 'gog', skind: 'install', kind: '', st: 'squeued', pct: 0, os: 'windows' });
  DL.push({ id: 'sb3', sgid: 's4', t: 'Quiet Comet', store: 'steam', skind: 'steam', kind: '', st: 'sactive', pct: 52 });
  DL.push({ id: 'sb4', t: 'Glimmer Deep', store: 'epic', skind: 'install', kind: '', st: 'failed', line: 'Failed · legendary stopped: Epic\'s download server didn\'t answer' });
  S.tab = 'library'; S.libRail = 'st:gog:inst'; TABS.library._k = 'r-st:gog:inst';
}
