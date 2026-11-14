/* ==== External game stores: GOG (plus GOG offline installers), Epic Games,
   Amazon Games and Steam, in the Library beside the RomM games.

   - Each store GRID knows gets a group in the Library rail, like Backup drives:
     the store's name as the heading, then two rows, Installed and Available.
   - GOG, Epic and Amazon install through Heroic's command-line tools (gogdl,
     legendary, nile), which GRID downloads the first time the store is
     connected. Steam installs, uninstalls and launches through the Steam client.
   - Store games have no RomM features: no server saves, no play activity, no
     relink, no drive backups. GOG and Epic have the store's own cloud saves.
   Placeholder titles only; the store marks are monogram tiles, never a logo. ==== */
var ST = {
  order: ['amazon', 'epic', 'gog', 'steam'],
  stores: {
    amazon: { id: 'amazon', name: 'Amazon Games', mono: 'A', hue: 'hsl(32 80% 42%)', conn: 'expired', account: 'sam.plays', tool: 'ok', loading: false,
      toolName: 'nile', toolPkg: 'nile', toolVer: '1.1.2', toolSize: '9 MB', license: 'GPL-3.0', src: 'github.com/imLinguin/nile', host: 'amazon.com', cloud: false,
      folder: '~/Games/GRID/stores/amazon', refreshed: '3 days ago' },
    epic: { id: 'epic', name: 'Epic Games', mono: 'E', hue: 'hsl(230 10% 26%)', conn: 'ok', account: 'SamPlays', tool: 'ok', loading: false,
      toolName: 'legendary', toolPkg: 'legendary', toolVer: '0.20.37', toolSize: '14 MB', license: 'GPL-3.0', src: 'github.com/derrod/legendary', host: 'epicgames.com', cloud: true,
      folder: '~/Games/GRID/stores/epic', refreshed: '5 min ago' },
    gog: { id: 'gog', name: 'GOG', mono: 'G', hue: 'hsl(278 45% 42%)', conn: 'ok', account: 'sam_plays', tool: 'ok', loading: false,
      toolName: 'gogdl', toolPkg: 'heroic-gogdl', toolVer: '1.1.2', toolSize: '12 MB', license: 'GPL-3.0', src: 'github.com/Heroic-Games-Launcher/heroic-gogdl', host: 'login.gog.com', cloud: true,
      folder: '~/Games/GRID/stores/gog', refreshed: '5 min ago' },
    steam: { id: 'steam', name: 'Steam', mono: 'S', hue: 'hsl(210 45% 30%)', conn: 'ok', account: 'SamPlays', tool: 'ok', loading: false,
      steamId: '76561198000000001', key: 'ok', keyTail: '3F2A', client: true, clientPath: '/usr/bin/steam', refreshed: '5 min ago',
      accounts: [['SamPlays', '76561198000000001', 'signed in to Steam on this computer'], ['couchguest', '76561198000000002', 'has signed in on this computer before']] }
  },
  /* Store games. `inst` is installed here; `os` the build (linux / windows); `cloud`
     the store cloud-save state for GOG and Epic; `ns` a game GRID can't install
     (it starts through another company's launcher). */
  games: [
    { id: 'g1', s: 'gog', t: 'Ashen Tide', c: 'hsl(12 55% 38%)', inst: true, os: 'linux', ver: '1.0.4', size: 14.2, disk: 21.6, played: '2 days ago', cloud: 'synced', synced: '2 hours ago', builds: ['linux', 'windows'],
      dlc: [{ t: 'Ashen Tide — Original Soundtrack', size: .41, inst: true }, { t: 'Ashen Tide — Digital Artbook', size: .12, inst: false }], dev: 'Lantern Works', year: 2021, genre: 'RPG' },
    { id: 'g2', s: 'gog', t: 'Lanternfall', c: 'hsl(42 60% 36%)', inst: true, os: 'windows', ver: '2.1', upd: '2.2', size: 32.8, disk: 41.0, played: 'yesterday', cloud: 'conflict', synced: '3 days ago', builds: ['windows'], dev: 'Brine & Ember', year: 2023, genre: 'Strategy' },
    { id: 'g3', s: 'gog', t: 'Brightwater', c: 'hsl(190 50% 32%)', inst: true, os: 'linux', ver: '1.3', size: 3.1, disk: 4.4, played: '3 weeks ago', cloud: 'off', builds: ['linux', 'windows'], offline: true, dev: 'Small Harbor', year: 2019, genre: 'Puzzle' },
    { id: 'g4', s: 'gog', t: 'Copper Saints', c: 'hsl(25 45% 30%)', inst: false, size: 18.4, disk: 26.0, builds: ['linux', 'windows'], dev: 'Iron Bell', year: 2022, genre: 'Action' },
    { id: 'g5', s: 'gog', t: 'Hush Valley', c: 'hsl(140 30% 30%)', inst: false, size: 6.2, disk: 8.0, builds: ['windows'], dev: 'Fernlight', year: 2020, genre: 'Adventure' },
    { id: 'g6', s: 'gog', t: 'Marrowind Keep', c: 'hsl(300 25% 30%)', inst: false, size: 22.0, disk: 30.5, builds: ['linux', 'windows'], langs: ['English', 'Deutsch', 'Français', 'Polski'], dev: 'Quill Hall', year: 2018, genre: 'RPG',
      dlc: [{ t: 'Marrowind Keep — The Sunken Crypt', size: 2.4 }, { t: 'Marrowind Keep — Soundtrack', size: .3 }] },
    { id: 'g7', s: 'gog', t: 'Paper Lanterns', c: 'hsl(350 45% 40%)', inst: false, size: 1.1, disk: 1.6, builds: ['linux'], dev: 'Origami House', year: 2017, genre: 'Puzzle' },
    { id: 'g8', s: 'gog', t: 'Quarry Nine', c: 'hsl(210 15% 34%)', inst: false, size: 9.7, disk: 13.2, builds: ['windows'], dev: 'Deepcut', year: 2024, genre: 'Simulation' },
    { id: 'e1', s: 'epic', t: 'Starlight Courier', c: 'hsl(250 45% 36%)', inst: true, os: 'windows', ver: '1.12', size: 24.0, disk: 31.2, played: '4 days ago', cloud: 'synced', synced: '4 days ago', builds: ['windows'], dev: 'Northstar', year: 2022, genre: 'Racing' },
    { id: 'e2', s: 'epic', t: 'Vector Bloom', c: 'hsl(160 45% 30%)', inst: true, os: 'windows', ver: '3.0.1', size: 8.8, disk: 11.0, played: 'never', cloud: 'none', builds: ['windows'], dev: 'Petal Logic', year: 2021, genre: 'Puzzle' },
    { id: 'e3', s: 'epic', t: 'Glimmer Deep', c: 'hsl(195 55% 30%)', inst: false, size: 45.0, disk: 60.0, builds: ['windows'], dev: 'Abyssal', year: 2023, genre: 'Action' },
    { id: 'e4', s: 'epic', t: 'Iron Orchard', c: 'hsl(20 30% 28%)', inst: false, size: 52.0, disk: 70.0, builds: ['windows'], ns: 'the EA app', dev: 'Forge North', year: 2022, genre: 'Shooter' },
    { id: 'e5', s: 'epic', t: 'Northern Static', c: 'hsl(205 25% 36%)', inst: false, size: 38.0, disk: 50.0, builds: ['windows'], ns: 'Ubisoft Connect', dev: 'Polar Line', year: 2021, genre: 'Open world' },
    { id: 'e6', s: 'epic', t: 'Rustbelt Rally', c: 'hsl(35 50% 34%)', inst: false, size: 12.5, disk: 16.0, builds: ['windows'], dev: 'Gravel Works', year: 2020, genre: 'Racing' },
    { id: 'a1', s: 'amazon', t: 'Fogline', c: 'hsl(220 20% 34%)', inst: true, os: 'windows', ver: '1.0', size: 5.5, disk: 7.0, played: 'last month', builds: ['windows'], dev: 'Mistral', year: 2020, genre: 'Adventure' },
    { id: 'a2', s: 'amazon', t: 'Kettle Drum', c: 'hsl(5 45% 36%)', inst: false, size: 2.2, disk: 3.0, builds: ['windows'], dev: 'Brass Band', year: 2019, genre: 'Rhythm' },
    { id: 'a3', s: 'amazon', t: 'Lowsun', c: 'hsl(45 55% 38%)', inst: false, size: 15.0, disk: 19.0, builds: ['windows'], dev: 'Dusk Yard', year: 2021, genre: 'Survival' },
    { id: 's1', s: 'steam', t: 'Moonlit Ferry', c: 'hsl(235 35% 30%)', inst: true, os: 'steam', played: 'today', size: 7.0, disk: 9.1, dev: 'Tideglass', year: 2024, genre: 'Adventure' },
    { id: 's2', s: 'steam', t: 'Deep Field Radio', c: 'hsl(170 30% 28%)', inst: true, os: 'steam', played: '1 week ago', outside: true, size: 3.3, disk: 4.0, dev: 'Static Choir', year: 2022, genre: 'Horror' },
    { id: 's3', s: 'steam', t: 'Old Brick Road', c: 'hsl(15 35% 32%)', inst: true, os: 'steam', played: 'never', outside: true, size: 11.0, disk: 14.0, dev: 'Kiln', year: 2018, genre: 'Strategy' },
    { id: 's4', s: 'steam', t: 'Quiet Comet', c: 'hsl(265 30% 36%)', inst: false, size: 6.0, disk: 8.0, dev: 'Slow Orbit', year: 2025, genre: 'Puzzle' },
    { id: 's5', s: 'steam', t: 'Harbor of Bells', c: 'hsl(200 40% 38%)', inst: false, size: 19.0, disk: 24.0, dev: 'Chime', year: 2023, genre: 'RPG' },
    { id: 's6', s: 'steam', t: 'Rime Assembly', c: 'hsl(185 25% 40%)', inst: false, size: 4.4, disk: 6.0, dev: 'Frostbyte Co.', year: 2021, genre: 'Building' },
    { id: 's7', s: 'steam', t: 'Signal Lost', c: 'hsl(0 0% 26%)', inst: false, size: 27.0, disk: 35.0, dev: 'Dead Air', year: 2020, genre: 'Shooter' }
  ],
  jobs: [], done: [], dlg: null, login: null, conf: null, proto: { amazon: 'expired' }, compat: 'GE-Proton10-17', playing: null
};
var ST_KIND = { install: '', restore: '', romm: '', update: 'Update', repair: 'Repair', dlc: 'Content', tool: 'Store tool', installer: 'Installer', steam: '' };
function stS(id) { return ST.stores[id]; }
function stG(id) { return ST.games.filter(function (g) { return g.id === id; })[0]; }
function stOf(sid) { return ST.games.filter(function (g) { return g.s === sid; }); }
function stMark(s, cls) { return '<span class="st-mono' + (cls ? ' ' + cls : '') + '" style="--st-hue:' + s.hue + '" aria-hidden="true">' + s.mono + '</span>'; }
function stJob(g) { return ST.jobs.filter(function (j) { return j.gid === g.id && (j.st === 'active' || j.st === 'queued'); })[0]; }
function stFmt(gb) { return bkFmt(gb); }
function stPlural(n, one, many) { return bkPlural(n, one, many); }
/* A store shows in the Library when it is connected (or its login expired), or
   when it still has games installed here after a Disconnect. GOG also shows when
   an offline installer put a game here, with no login. */
function stShown(s) { return s.conn !== 'off' || stOf(s.id).some(function (g) { return g.inst; }); }
function stInst(s) { return stOf(s.id).filter(function (g) { return g.inst; }); }
function stAvail(s) { return stOf(s.id).filter(function (g) { return !g.inst; }); }
/* Why Available can't list games right now, or '' when it can. */
function stAvailBlock(s) {
  if (s.conn === 'off') return 'off';
  if (s.conn === 'expired') return 'expired';
  if (s.id === 'steam') return s.key === 'ok' ? '' : s.key;
  if (s.tool !== 'ok') return 'tool';
  if (s.loading) return 'loading';
  return '';
}
function stView() {
  var f = S.libFilter; if (!f || f.indexOf('st:') !== 0) return null;
  var p = f.split(':'); return { s: stS(p[1]), part: p[2] };
}
/* A store notification card, optionally with one action button (the desktop's
   notification cards take one action beside Dismiss). */
function stToast(msg, kind, act) {
  toast(msg, kind);
  if (!act) return;
  var cards = document.querySelectorAll('#notes .note-card:not(#setup-card)'), c = cards[cards.length - 1]; if (!c) return;
  var b = document.createElement('button'); b.className = 'small-btn primary'; b.textContent = act[0];
  b.onclick = function () { noteClose(c); act[1](); };
  c.querySelector('.form-actions').insertBefore(b, c.querySelector('.note-dismiss'));
}

/* ---- Library: the rail groups ---- */
function stRailHTML(sv) {
  var h = '';
  ST.order.forEach(function (id) {
    var s = stS(id); if (!stShown(s)) return;
    var block = stAvailBlock(s), inst = stInst(s), av = stAvail(s);
    var flag = s.conn === 'expired' ? '<span class="st-flag" title="Login expired">!</span>' : s.conn === 'off' ? '<span class="st-flag off" title="Not connected">off</span>'
      : (s.tool !== 'ok' && id !== 'steam') || (id === 'steam' && (!s.client || s.key !== 'ok')) ? '<span class="st-flag" title="Needs attention">!</span>' : '';
    h += '<span class="g-rail-heading st-head">' + stMark(s) + esc(s.name.toUpperCase()) + flag + '</span>';
    [['inst', 'Installed', inst.length], ['avail', 'Available', av.length]].forEach(function (r) {
      var key = 'st:' + id + ':' + r[0], sel = sv && sv.s.id === id && sv.part === r[0];
      var cnt = '<span class="g-rail-count">' + r[2] + '</span>';
      if (r[0] === 'avail' && block === 'loading') cnt = '<span class="g-rail-count st-wait" aria-label="Loading"><span class="st-spin"></span></span>';
      else if (r[0] === 'avail' && block) cnt = '<span class="g-rail-count st-warn" aria-label="Needs attention">!</span>';
      else if (r[0] === 'inst' && id === 'steam' && !s.client) cnt = '<span class="g-rail-count st-warn">!</span>';
      h += '<button class="g-rail-item st-row' + (sel ? ' active' : '') + '" data-key="' + key + '"' + (sel ? ' aria-current="page"' : '') + '><span class="g-rail-label">' + r[1] + '</span>' + cnt + '</button>';
    });
  });
  return h;
}

/* ---- Library: a store view's header and grid ---- */
function stProgress(g) {
  var j = stJob(g); if (!j) return '';
  return '<span class="st-cprog' + (j.kind === 'steam' ? ' steam' : '') + '"><b style="width:' + Math.round(j.st === 'active' ? j.pct : 0) + '%"></b></span>';
}
function stPrimary(g) {
  var s = stS(g.s), j = stJob(g);
  if (j) return j.kind === 'steam' ? ['Installing in Steam', 'details', true] : j.st === 'queued' ? ['Queued', 'details'] : [j.kind === 'update' ? 'Updating…' : j.kind === 'repair' ? 'Verifying…' : 'Installing…', 'details'];
  if (g.inst) return ['Play', 'play'];
  if (g.ns) return ['Details', 'details'];
  if (s.id === 'steam') return ['Install in Steam', 'install'];
  if (s.conn !== 'ok') return ['Details', 'details'];
  return ['Install', 'install'];
}
function stCard(g, mixed) {
  var s = stS(g.s), j = stJob(g), p = stPrimary(g);
  var tags = [];
  if (j) tags.push('<span class="g-tag st-q">' + (j.st === 'active' ? Math.round(j.pct) + '%' : 'QUEUED') + '</span>');
  if (g.upd && g.inst && !j) tags.push('<span class="g-tag update">UPDATE</span>');
  if (g.cloud === 'conflict' && g.inst) tags.push('<span class="g-tag st-conflict">SAVE CONFLICT</span>');
  if (g.ns) tags.push('<span class="g-tag st-ns">NOT SUPPORTED</span>');
  var cloudBtn = (s.id === 'gog' || s.id === 'epic') && g.inst ? '<button class="g-round" data-st-act="cloud" aria-label="Cloud saves" data-icon="cloud" data-size="15"></button>' : '';
  return '<div class="g-card' + (g.ns ? ' st-ns-card' : '') + '" data-sgame="' + g.id + '"><div class="g-cover"><div class="g-art" style="background:' + g.c + '"><span>' + esc(g.t) + '</span></div>' +
    (tags.length ? '<div class="g-tag-stack">' + tags.join('') + '</div>' : '') +
    (g.inst ? '<span class="g-dot" role="img" aria-label="Installed"></span>' : '') +
    (mixed ? '<span class="g-tag platform g-corner st-tag">' + stMark(s) + esc(s.name) + '</span>' : '') +
    stProgress(g) +
    '<div class="g-shade"></div><div class="g-bar"><button class="g-card-primary" data-st-act="' + p[1] + '"' + (p[2] ? ' disabled title="Managed by Steam"' : '') + '>' + p[0] + '</button>' + cloudBtn +
    '<button class="g-round" data-st-act="details" aria-label="Details" data-icon="more" data-size="15"></button></div></div>' +
    '<span class="g-label" title="' + syEsc(g.t) + '">' + esc(g.t) + '</span></div>';
}
function stMatch(g) { var q = S.q.library.trim().toLowerCase(); return !q || g.t.toLowerCase().indexOf(q) >= 0 || stS(g.s).name.toLowerCase().indexOf(q) >= 0; }
function stGrid(list, mixed) {
  var el = $('lib-grid');
  el.className = 'grid' + (S.labels === 'hover' ? ' hover-labels' : '');
  el.style.setProperty('--card-w', S.cardW + 'px');
  el.innerHTML = list.map(function (g) { return stCard(g, mixed); }).join('');
  gIcons(el);
}
function stState(o) {
  return '<div class="g-panel st-state' + (o.tone ? ' ' + o.tone : '') + '">' + (o.icon ? '<span class="st-ico">' + o.icon + '</span>' : '') + '<h3>' + o.h + '</h3>' + (o.p ? '<p>' + o.p + '</p>' : '') +
    (o.acts ? '<div class="form-actions">' + o.acts + '</div>' : '') + '</div>';
}
function stProtoSelect(s) {
  var opts = [['', 'Prototype: normal']];
  if (s.id === 'steam') opts = opts.concat([['loading', 'Loading owned games'], ['emptyInst', 'Nothing installed'], ['emptyAvail', 'Everything installed'], ['nokey', 'No Web API key'], ['private', 'Profile game details private'], ['badkey', 'Key rejected'], ['noclient', 'Steam not installed'], ['off', 'Not connected']]);
  else opts = opts.concat([['loading', 'Loading owned games'], ['emptyInst', 'Nothing installed'], ['emptyAvail', 'Everything installed'], ['tool', s.toolName + ' not installed'], ['toolfail', s.toolName + ' install failed'], ['expired', 'Login expired'], ['off', 'Not connected']]);
  var cur = ST.proto[s.id] || '';
  return '<select class="small-btn bk-proto" data-st-proto="' + s.id + '" aria-label="Prototype state">' + opts.map(function (o) { return '<option value="' + o[0] + '"' + (o[0] === cur ? ' selected' : '') + '>' + esc(o[1]) + '</option>'; }).join('') + '</select>';
}
/* The prototype's states, set from the dashed select in a store view's header
   or in Settings › Stores. The base data comes back first, then the state applies. */
var ST_BASE = JSON.parse(JSON.stringify({ stores: ST.stores, games: ST.games }));
function stApplyProto(sid, v) {
  ST.proto[sid] = v;
  var base = JSON.parse(JSON.stringify(ST_BASE.stores[sid])), s = stS(sid);
  Object.keys(base).forEach(function (k) { s[k] = base[k]; });
  if (sid === 'amazon') s.conn = 'expired';
  if (v === 'loading') { s.conn = 'ok'; s.loading = true; }
  if (v === 'tool') { s.conn = 'ok'; s.tool = 'missing'; }
  if (v === 'toolfail') { s.conn = 'ok'; s.tool = 'failed'; }
  if (v === 'expired') s.conn = 'expired';
  if (v === 'off') { s.conn = 'off'; }
  if (v === 'nokey') s.key = 'none';
  if (v === 'private') s.key = 'private';
  if (v === 'badkey') s.key = 'bad';
  if (v === 'noclient') s.client = false;
  if (sid === 'amazon' && v === '') s.conn = 'ok';
  /* States that change what is installed keep a snapshot, so going back restores it. */
  ST.snap = ST.snap || {};
  var wipes = ['emptyInst', 'emptyAvail', 'noclient'].indexOf(v) >= 0 || (v === 'off' && sid === 'steam');
  if (ST.snap[sid]) { ST.games.forEach(function (g) { if (g.s === sid && g.id in ST.snap[sid]) g.inst = ST.snap[sid][g.id]; }); delete ST.snap[sid]; }
  if (!wipes) return;
  ST.snap[sid] = {};
  ST.games.forEach(function (g) {
    if (g.s !== sid || stJob(g)) return;
    ST.snap[sid][g.id] = g.inst;
    g.inst = v === 'emptyAvail';
  });
}
function stRenderView(sv) {
  var s = sv.s, inst = sv.part === 'inst', block = stAvailBlock(s);
  var list = (inst ? stInst(s) : stAvail(s)).filter(stMatch);
  $('lib-title').innerHTML = esc(s.name) + ' <span class="st-sub">· ' + (inst ? 'Installed' : 'Available') + '</span>';
  var total = (inst ? stInst(s) : stAvail(s)).reduce(function (t, g) { return t + (inst ? g.disk : g.size); }, 0);
  var count = inst
    ? stPlural(stInst(s).length, 'game') + ' · ' + stFmt(total) + (s.id === 'steam' ? ' · in your Steam libraries' : '')
    : (block ? 'Games you own and haven’t installed' : stPlural(stAvail(s).length, 'game') + ' you own and haven’t installed · updated ' + s.refreshed);
  $('lib-count').textContent = count;
  var chips = $('lib-chips'); chips.hidden = false;
  var c = [];
  if (s.conn === 'ok') c.push('<button class="g-chip-outline" data-st-goset="' + s.id + '">' + (s.id === 'steam' ? 'Steam account: ' + esc(s.account) : 'Signed in as ' + esc(s.account)) + '</button>');
  if (s.conn === 'expired') c.push('<button class="g-chip-outline st-chip-warn" data-st-login="' + s.id + '">Login expired · Sign in again</button>');
  if (s.conn === 'off') c.push('<button class="g-chip-outline" data-st-goset="' + s.id + '">Not connected · Connect…</button>');
  if (s.cloud && s.conn !== 'off') c.push('<span class="g-chip-outline">Cloud saves: on</span>');
  if (s.id === 'gog' && inst && stInst(s).length) c.push('<button class="small-btn bk-plat" data-st-bkall>Backup all ' + stInst(s).length + '…</button>');
  if (s.id !== 'steam' && s.tool !== 'ok' && s.conn === 'ok') c.push('<button class="g-chip-outline st-chip-warn" data-st-goset="' + s.id + '">' + s.toolName + ' not installed</button>');
  /* Header actions sit after the chips, on the right like Backup all N… */
  chips.innerHTML = c.join('') + (s.id === 'gog' ? '<button class="small-btn" data-st-installer>Install from installer…</button>' : '') +
    (!inst && !block ? '<button class="small-btn" data-st-refresh="' + s.id + '">Refresh</button>' : '') + stProtoSelect(s);
  var sel = document.querySelector('[data-view=library] [data-bk-select]'); if (sel) sel.hidden = true;
  var g = $('lib-grid');
  /* Installed */
  if (inst) {
    if (s.id === 'steam' && !s.client) {
      g.className = 'grid'; g.innerHTML = stState({ icon: stMark(s, 'lg'), h: 'Steam isn’t installed', p: 'GRID installs, starts and uninstalls Steam games through the Steam client. Install Steam from your distribution’s software centre or from Steam’s own site, sign in once, then come back.', acts: '<button class="g-btn secondary" data-st-recheck>Check again</button>' });
      return;
    }
    if (!stInst(s).length) {
      g.className = 'grid';
      g.innerHTML = stState({ icon: stMark(s, 'lg'), h: 'No ' + esc(s.name) + ' games installed yet',
        p: s.id === 'gog' ? 'Install one from Available, or from a GOG offline installer you downloaded.' : s.id === 'steam' ? 'Install one from Available. Games you install in Steam itself show here too.' : 'Install one from Available.',
        acts: '<button class="g-btn" data-st-go="st:' + s.id + ':avail">Open Available</button>' + (s.id === 'gog' ? '<button class="g-btn secondary" data-st-installer>Install from installer…</button>' : '') });
      return;
    }
    if (!list.length) { g.className = 'grid'; g.innerHTML = '<p class="g-panel empty">No installed ' + esc(s.name) + ' games match “' + esc(S.q.library.trim()) + '”</p>'; return; }
    stGrid(list, false);
    return;
  }
  /* Available */
  if (block === 'off') { g.className = 'grid'; g.innerHTML = stState({ icon: stMark(s, 'lg'), h: esc(s.name) + ' isn’t connected', p: 'Your installed ' + esc(s.name) + ' games still play. Connect again to see the games you own and install them.', acts: '<button class="g-btn" data-st-connect="' + s.id + '">Connect ' + esc(s.name) + '…</button>' }); return; }
  if (block === 'expired') { g.className = 'grid'; g.innerHTML = stState({ tone: 'warn', icon: stMark(s, 'lg'), h: 'Your ' + esc(s.name) + ' login expired', p: 'Sign in again to see the games you own and install them. Installed games still play.', acts: '<button class="g-btn" data-st-login="' + s.id + '">Sign in again…</button>' }); return; }
  if (block === 'tool') {
    var busy = s.tool === 'installing', fail = s.tool === 'failed';
    g.className = 'grid';
    g.innerHTML = stState({ tone: fail ? 'err' : '', icon: stMark(s, 'lg'), h: fail ? 'Couldn’t install ' + s.toolName : esc(s.name) + ' needs ' + s.toolName,
      p: fail ? 'The download didn’t match its checksum, so GRID deleted it. Nothing else changed.' : 'GRID installs and updates ' + esc(s.name) + ' games with ' + s.toolName + ' (' + s.toolPkg + ', ' + s.license + '). It’s ' + s.toolSize + '.',
      acts: '<button class="g-btn" data-st-tool="' + s.id + '"' + (busy ? ' disabled' : '') + '>' + (busy ? 'Installing ' + s.toolName + '…' : fail ? 'Try again' : 'Install ' + s.toolName + ' (' + s.toolSize + ')') + '</button>' });
    return;
  }
  if (block === 'none') { g.className = 'grid'; g.innerHTML = stState({ icon: stMark(s, 'lg'), h: 'Steam needs your Web API key to list the games you haven’t installed', p: 'Installed Steam games show without it. GRID asks Steam for your owned games with your own key, kept in the OS keyring.', acts: '<button class="g-btn" data-st-goset="steam">Add your key in Settings…</button>' }); return; }
  if (block === 'bad') { g.className = 'grid'; g.innerHTML = stState({ tone: 'err', icon: stMark(s, 'lg'), h: 'Steam rejected your Web API key', p: 'Steam answered 403 Forbidden. The key may have been revoked. Make a new key, then paste it in Settings › Stores.', acts: '<button class="g-btn" data-st-goset="steam">Open Settings › Stores</button>' }); return; }
  if (block === 'private') { g.className = 'grid'; g.innerHTML = stState({ tone: 'warn', icon: stMark(s, 'lg'), h: 'Your Steam game details are private', p: 'Steam lists owned games only when the profile’s game details are public. In Steam: your profile › Edit Profile › Privacy Settings › Game details: Public. Then refresh.', acts: '<button class="g-btn" data-st-refresh="steam">Refresh</button><button class="g-btn secondary" data-st-goset="steam">Open Settings › Stores</button>' }); return; }
  if (block === 'loading') {
    g.className = 'grid'; g.style.setProperty('--card-w', S.cardW + 'px');
    var sk = ''; for (var i = 0; i < 8; i++) sk += '<div class="g-card st-skel" aria-hidden="true"><div class="g-cover"></div><span class="g-label">&nbsp;</span></div>';
    g.innerHTML = '<p class="st-loading-line" style="grid-column:1/-1"><span class="st-spin"></span>Loading your ' + esc(s.name) + ' games…</p>' + sk;
    $('lib-count').textContent = 'Games you own and haven’t installed · loading';
    return;
  }
  if (!stAvail(s).length) { g.className = 'grid'; g.innerHTML = stState({ icon: stMark(s, 'lg'), h: 'Every ' + esc(s.name) + ' game you own is installed', p: 'New games you buy show here after a refresh.', acts: '<button class="g-btn secondary" data-st-refresh="' + s.id + '">Refresh</button>' }); return; }
  if (!list.length) { g.className = 'grid'; g.innerHTML = '<p class="g-panel empty">No ' + esc(s.name) + ' games match “' + esc(S.q.library.trim()) + '”</p>'; return; }
  stGrid(list, false);
}
var stOrigLib = renderLibrary;
renderLibrary = function () {
  var sv = stView(); if (sv && !stShown(sv.s)) { S.libFilter = 'all'; sv = null; }
  var keep = S.libFilter;
  if (sv) { S.libFilter = 'all'; BK.sel = { view: null, ids: [] }; }
  if (!sv || sv.s.id !== 'gog' || sv.part !== 'inst') ST.sel = null;
  stOrigLib();
  S.libFilter = keep;
  var rail = $('lib-rail');
  if (sv) rail.querySelectorAll('.g-rail-item').forEach(function (b) { b.classList.remove('active'); b.removeAttribute('aria-current'); });
  rail.insertAdjacentHTML('beforeend', stRailHTML(sv));
  if (!sv) {
    var sel = document.querySelector('[data-view=library] [data-bk-select]'); if (sel && !bkLibDrive()) sel.hidden = false;
    stMixIn(); stDriveGog(); return;
  }
  stRenderView(sv);
  $('lib-grid').querySelectorAll('.bk-check').forEach(function (x) { x.remove(); });
  stSelPaint(sv);
};
/* All games, Recent and Updates hold the installed store games too, each with
   its store's tag. Platform rows don't: store games live in their store group. */
function stMixIn() {
  var f = S.libFilter; if (['all', 'recent', 'updates'].indexOf(f) < 0) return;
  var list = ST.games.filter(function (g) { return g.inst && stShown(stS(g.s)) && stMatch(g) && (f === 'all' || (f === 'recent' ? /today|yesterday|days? ago|week ago/.test(g.played || '') : g.upd)); });
  if (!list.length) return;
  var el = $('lib-grid'), empty = el.querySelector('.empty'); if (empty) empty.remove();
  el.insertAdjacentHTML('beforeend', list.map(function (g) { return stCard(g, true); }).join(''));
  gIcons(el);
  var n = el.querySelectorAll('[data-game],[data-sgame]').length;
  $('lib-count').textContent = n + (n === 1 ? ' game' : ' games');
  var rail = $('lib-rail');
  var fix = function (key, add) { var b = rail.querySelector('[data-key="' + key + '"] .g-rail-count'); if (b) b.textContent = +b.textContent + add; };
  fix('all', ST.games.filter(function (g) { return g.inst && stShown(stS(g.s)); }).length);
  fix('updates', ST.games.filter(function (g) { return g.inst && g.upd; }).length);
}

/* ---- the install dialog ---- */
function stOpenInstall(g) {
  var s = stS(g.s);
  if (s.id === 'steam') { stSteamInstall(g); return; }
  if (g.ns) { stOpenDetails(g); return; }
  if (s.conn !== 'ok') { stToast('Sign in to ' + s.name + ' again to install ' + g.t, 'error', ['Sign in again', function () { stLoginOpen(s.id); }]); return; }
  if (s.tool !== 'ok') { stToast(s.name + ' needs ' + s.toolName + ' to install games', 'error', ['Install ' + s.toolName, function () { stInstallTool(s); }]); return; }
  ST.dlg = { kind: 'install', gid: g.id, os: g.builds[0] === 'linux' || g.builds.indexOf('linux') >= 0 ? 'linux' : 'windows', compat: ST.compat, lang: (g.langs || ['English'])[0],
    dlc: (g.dlc || []).map(function () { return true; }), folder: s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, ''), small: false };
  stRenderDlg();
}
function stDlgSize(d, g) { var add = (g.dlc || []).reduce(function (t, x, i) { return t + (d.dlc[i] ? x.size : 0); }, 0); return { dl: g.size + add, disk: g.disk + add * 1.3 }; }
function stRenderDlg() {
  var d = ST.dlg, el = $('stdlg'); if (!d) { $('stscrim').hidden = true; return; }
  var h = '';
  if (d.kind === 'install') h = stInstallHTML(d);
  else if (d.kind === 'backup') h = stBackupHTML(d);
  else if (d.kind === 'settings') h = stSettingsHTML(d);
  else if (d.kind === 'picker') h = stPickerHTML(d);
  else if (d.kind === 'reading') h = stReadingHTML(d);
  else if (d.kind === 'found') h = stFoundHTML(d);
  else if (d.kind === 'conflict') h = stConflictHTML(d);
  el.innerHTML = '<button class="g-icon-btn g-dialog-close" aria-label="Close" data-st-dlg="close"><span data-icon="close" data-size="20"></span></button>' + h;
  $('stscrim').hidden = false; gIcons(el);
}
function stClose() { ST.dlg = null; $('stscrim').hidden = true; }
function stHead(g, title, sub) {
  return '<div class="st-dlg-head"><span class="st-cover-sm" style="background:' + g.c + '">' + esc(g.t) + '</span><span><h3>' + title + '</h3><span class="note">' + sub + '</span></span></div>';
}
function stInstallHTML(d) {
  var g = stG(d.gid), s = stS(g.s), sz = stDlgSize(d, g), free = d.small ? 18.4 : 412, over = sz.disk > free;
  var plat = '';
  if (s.id === 'gog' && g.builds.length > 1) {
    plat = '<div class="sub"><h4>Version</h4><div class="bk-opts" role="radiogroup" aria-label="Version">' +
      [['linux', 'Linux', 'Runs natively on this computer'], ['windows', 'Windows', 'Runs through ' + esc(d.compat)]].map(function (o) {
        return '<label class="bk-opt' + (d.os === o[0] ? ' on' : '') + '"><input type="radio" name="st-os" value="' + o[0] + '"' + (d.os === o[0] ? ' checked' : '') + '><span class="bk-opt-main"><b>' + o[1] + '</b><span class="note">' + o[2] + '</span></span>' + (o[0] === 'linux' ? '<span class="bk-opt-free">Recommended</span>' : '') + '</label>';
      }).join('') + '</div></div>';
  }
  var compat = d.os === 'windows' ? '<label class="st-field"><span>Compatibility tool</span><span class="g-control"><select data-st-f="compat">' + ['GE-Proton10-17', 'Proton Experimental (umu)', 'Wine 10.0 (system)'].map(function (t) { return '<option' + (d.compat === t ? ' selected' : '') + '>' + t + '</option>'; }).join('') + '</select></span></label>' +
    '<p class="help">The default from Settings › Systems › Windows. GRID makes a Wine prefix for this game.</p>' : '';
  var langs = g.langs ? '<label class="st-field"><span>Language</span><span class="g-control"><select data-st-f="lang">' + g.langs.map(function (l) { return '<option' + (d.lang === l ? ' selected' : '') + '>' + l + '</option>'; }).join('') + '</select></span></label>' : '';
  var dlc = g.dlc && g.dlc.length ? '<div class="sub"><h4>DLC you own</h4>' + g.dlc.map(function (x, i) { return '<label class="check bk-also"><input type="checkbox" data-st-dlc="' + i + '"' + (d.dlc[i] ? ' checked' : '') + '><span><b>' + esc(x.t.replace(g.t + ' — ', '')) + '</b><span class="note">' + stFmt(x.size) + '</span></span></label>'; }).join('') + '</div>' : '';
  return stHead(g, 'Install ' + esc(g.t), stMark(s) + ' ' + esc(s.name) + ' · ' + stFmt(g.size) + ' download') + plat +
    '<div class="sub"><h4>Options</h4>' + compat + langs +
    '<div class="st-field"><span>Install folder</span><span class="inline"><code>' + esc(d.folder) + '</code><button class="small-btn" data-st-folder>Change…</button></span></div>' +
    '<p class="help">The default folder for ' + esc(s.name) + ' games is set in Settings › Stores.</p></div>' + dlc +
    '<div class="st-space"><p class="note' + (over ? ' warn' : '') + '">' + (over ? 'Needs ' + stFmt(sz.disk) + ' · ' + stFmt(free) + ' free on /home' : stFmt(sz.disk) + ' on disk · ' + stFmt(free) + ' free on /home') + '</p>' +
    '<button class="small-btn bk-proto" data-st-small>' + (d.small ? 'Prototype: plenty of space' : 'Prototype: not enough space') + '</button></div>' +
    '<div class="form-actions bk-foot"><span class="note">' + stFmt(sz.dl) + ' to download</span><button class="g-btn secondary" data-st-dlg="close">Cancel</button><button class="g-btn" data-st-dlg="install"' + (over ? ' disabled' : '') + '>' + (over ? 'Not enough space' : 'Install') + '</button></div>';
}
function stQueue(g, kind, opts) {
  opts = opts || {};
  var active = ST.jobs.some(function (j) { return j.st === 'active' && j.kind !== 'steam'; }) || !!S.dl;
  var j = { id: 'j' + Date.now() + Math.random(), gid: g ? g.id : null, t: opts.t || g.t, s: opts.s || g.s, kind: kind, st: kind === 'steam' || kind === 'tool' || !active ? 'active' : 'queued', pct: 0, os: opts.os, line: '', how: opts.how, from: opts.from, ri: opts.ri, p: opts.p };
  ST.jobs.push(j); stRun(); renderAll(); return j;
}
function stTick() {
  var changed = false;
  ST.jobs.forEach(function (j) {
    if (j.st !== 'active') return;
    if (j.s === 'romm' && riTick(j)) { changed = true; return; }
    j.pct = Math.min(100, j.pct + (j.kind === 'tool' ? 22 : j.kind === 'repair' ? 6 : j.kind === 'steam' ? 1 : j.s === 'romm' ? 3 : 1.6) + Math.random() * 1.5); changed = true;
    if (j.pct >= 100) stFinish(j);
  });
  var started = false;
  if (!ST.jobs.some(function (j) { return j.st === 'active' && j.kind !== 'steam'; })) { var q = ST.jobs.filter(function (j) { return j.st === 'queued'; })[0]; if (q) { q.st = 'active'; changed = started = true; } }
  if (!changed) return;
  if (started) { renderAll(); return; }
  if (S.view === 'downloads') renderDownloads();
  else { stLiveCards(); renderDownloads(); }
  if (S.stDetail) stRenderDetails();
}
/* While a job runs only the card's percentage and bar change. */
function stLiveCards() {
  ST.jobs.forEach(function (j) {
    if (!j.gid) return;
    document.querySelectorAll('[data-sgame="' + j.gid + '"], [data-game="' + j.gid + '"]').forEach(function (c) {
      var b = c.querySelector('.st-cprog b'), t = c.querySelector('.g-tag.st-q');
      if (b && j.st === 'active') b.style.width = Math.round(j.pct) + '%';
      if (t) t.textContent = j.st === 'active' ? Math.round(j.pct) + '%' : 'QUEUED';
    });
  });
}
function stRun() { if (!ST.timer) ST.timer = setInterval(stTick, 450); }
function stFinish(j) {
  if (j.s === 'romm') return riFinish(j);
  if (j.kind === 'restore') return stRestoreDone(j);
  var g = j.gid && stG(j.gid), s = stS(j.s);
  j.st = 'done';
  ST.jobs.splice(ST.jobs.indexOf(j), 1);
  var line;
  if (j.kind === 'tool') { s.tool = 'ok'; line = 'Installed · ' + s.toolVer; stToast(s.toolName + ' installed', '', null); if (s.conn === 'ok') stLoadOwned(s); }
  else if (j.kind === 'update') { g.ver = g.upd; delete g.upd; line = 'Updated to ' + g.ver; stToast(g.t + ' updated to ' + g.ver); }
  else if (j.kind === 'repair') { line = 'Verified · 2 files repaired'; stToast(g.t + ': 2 damaged files were downloaded again'); }
  else if (j.kind === 'dlc') { line = 'Installed'; (g.dlc || []).forEach(function (x) { if (x.t === j.t) x.inst = true; }); stToast(j.t + ' installed'); }
  else if (j.kind === 'installer') { g.inst = true; g.offline = true; g.os = j.os; g.ver = g.ver || '1.0.4'; line = 'Installed from its installer'; stToast(g.t + ' installed from its installer'); }
  else if (j.kind === 'steam') { g.inst = true; g.os = 'steam'; line = 'Installed by Steam'; stToast(g.t + ' installed', '', ['Play', function () { stPlay(g); }]); }
  else { g.inst = true; g.os = j.os || g.builds[0]; g.ver = g.ver || '1.0'; g.cloud = s.cloud ? 'synced' : undefined; g.synced = 'just now'; line = 'Installed · ' + stFmt(g.disk); stToast(g.t + ' installed', '', ['Play', function () { stPlay(g); }]); }
  ST.done.unshift({ t: j.t, s: j.s, kind: j.kind, line: line });
  renderAll();
}
function stJobLine(j) {
  var p = Math.round(j.pct);
  if (j.st === 'queued') return 'Waiting for a slot';
  if (j.s === 'romm') return riLine(j);
  if (j.kind === 'restore') return j.how === 'installer' ? (p < 45 ? 'Copying the installer from ' + j.from + ' ' + Math.round(p / 45 * 100) + '%' : 'Running the installer · ' + Math.round((p - 45) / 55 * 100) + '%') : 'Copying from ' + j.from + ' ' + p + '%';
  if (j.kind === 'steam') return 'Installing in Steam ' + p + '%';
  if (j.kind === 'tool') return 'Downloading ' + p + '% · checking its SHA-256';
  if (j.kind === 'repair') return 'Verifying files ' + p + '%';
  if (j.kind === 'installer') return j.os === 'windows' ? (p < 50 ? 'Running the installer silently · ' + p + '%' : 'Running the installer · ' + p + '%') : 'Unpacking the game files ' + p + '%';
  return (p < 85 ? 'Downloading ' + p + '% · 41.2 MB/s' : 'Installing ' + p + '%');
}

/* ---- Steam: everything goes through the Steam client ---- */
function stSteamInstall(g) {
  var s = stS('steam');
  if (!s.client) { stToast('Steam isn’t installed. Install Steam, then try again.', 'error'); return; }
  stToast('Steam opened to install ' + g.t + '. Choose the library folder there; GRID shows the progress.');
  stQueue(g, 'steam');
}
function stPlay(g) {
  var s = stS(g.s);
  if (s.id === 'steam') { ST.playing = g.id; stToast('Starting ' + g.t + ' through Steam…'); renderAll(); if (S.stDetail) stRenderDetails(); return; }
  if (g.cloud === 'conflict' && s.cloud) { ST.dlg = { kind: 'conflict', gid: g.id, launch: true }; stRenderDlg(); return; }
  var go = function () {
    ST.playing = g.id; g.played = 'today';
    stToast('Launching ' + g.t + '…'); renderAll(); if (S.stDetail) stRenderDetails();
    /* The prototype "closes" the game after a few seconds to show the sync after exit. */
    setTimeout(function () {
      if (ST.playing !== g.id) return; ST.playing = null;
      if (s.cloud && g.cloud !== 'off' && g.cloud !== 'none') { g.synced = 'just now'; stToast('Cloud saves uploaded to ' + s.name + ' for ' + g.t); }
      renderAll(); if (S.stDetail) stRenderDetails();
    }, 6000);
  };
  if (s.cloud && g.cloud === 'synced') { stToast('Syncing cloud saves for ' + g.t + '…'); setTimeout(go, 900); } else go();
}

/* ---- Details for a store game ---- */
function stOpenDetails(g, tab) {
  var keepFrom = ST.detFrom; closeDetails(); ST.detFrom = keepFrom;
  S.stDetail = g; S.tab = tab || 'overview'; S.stConfirm = null; paintArt(g);
  $('scrim').hidden = false; stRenderDetails(); $('dialog').focus({ preventScroll: true });
}
function stTabs(g) {
  var s = stS(g.s), t = [['overview', 'Overview'], ['media', 'Media']];
  if (s.cloud && g.inst) t.push(['cloud', 'Cloud saves']);
  return t;
}
function stRenderDetails() {
  var g = S.stDetail; if (!g) return;
  var s = stS(g.s), j = stJob(g), b = [], lines = [];
  var steam = s.id === 'steam';
  if (j && j.kind === 'steam') b.push('<div class="st-managed"><span><b>Installing in Steam · ' + Math.round(j.pct) + '%</b></span><div class="bar"><b style="width:' + Math.round(j.pct) + '%"></b></div><span>Managed by Steam. Pause or cancel it in Steam.</span></div><button class="g-btn secondary" disabled title="Managed by Steam">Cancel</button>');
  else if (j) b.push('<button class="g-btn" data-st-d="cancel">Cancel ' + (j.kind === 'update' ? 'update' : j.kind === 'repair' ? 'verify' : 'install') + (j.st === 'active' ? ' · ' + Math.round(j.pct) + '%' : ' · queued') + '</button>');
  else if (g.inst) b.push(ST.playing === g.id ? '<button class="g-btn" data-st-d="stop">' + (steam ? 'Running in Steam' : 'Stop') + '</button>' : '<button class="g-btn" data-st-d="play">Play</button>');
  else if (g.ns) b.push('<button class="g-btn" disabled>Not supported</button>');
  else if (steam) b.push('<button class="g-btn" data-st-d="install">Install in Steam</button>');
  else b.push('<button class="g-btn" data-st-d="install"' + (s.conn !== 'ok' ? ' disabled' : '') + '>Install…</button>');
  if (g.inst && g.upd && !j) b.push('<button class="g-btn secondary" data-st-d="update">Update to ' + esc(g.upd) + '</button>');
  if (g.inst && !steam) b.push('<button class="g-btn secondary" data-st-d="settings">Game Settings</button>');
  var fromD = ST.detFrom && bkDrive(ST.detFrom), packed = fromD && stPackOf(fromD, g);
  if (packed) {
    if (!g.inst && !j) b[0] = '<button class="g-btn" data-st-d="restore">Install from drive</button>';
    b.push(S.stConfirm === 'drm' ? '<button class="g-btn destructive" data-st-d="drm-go">Remove from ' + esc(fromD.name) + '</button>' : '<button class="g-btn secondary destructive" data-st-d="drm">Remove from drive…</button>');
  } else if (s.id === 'gog' && g.inst) b.push('<button class="g-btn secondary" data-st-d="backup">Backup to drive…</button>');
  if (g.inst && !j) b.push(steam ? '<button class="g-btn secondary" data-st-d="verify">Verify in Steam</button>' : '<button class="g-btn secondary" data-st-d="verify">Verify and repair</button>');
  if (g.inst && !j) b.push(steam ? '<button class="g-btn secondary" data-st-d="uninstall">Uninstall in Steam…</button>'
    : S.stConfirm === 'uninstall' ? '<button class="g-btn destructive confirm" data-st-d="uninstall-go">Confirm uninstall</button>' : '<button class="g-btn secondary destructive" data-st-d="uninstall">Uninstall</button>');
  if (!g.inst && s.conn !== 'ok' && !steam) lines.push('<p class="st-why">' + (s.conn === 'off' ? 'Connect ' + esc(s.name) + ' in Settings › Stores to install.' : 'Your ' + esc(s.name) + ' login expired. <button class="linkish" data-st-login="' + s.id + '">Sign in again</button>') + '</p>');
  if (g.ns) lines.push('<p class="st-why">Needs ' + esc(g.ns) + '. GRID can’t install games that start through another company’s launcher; install it from the Epic Games Launcher instead.</p>');
  if (g.inst) lines.push('<p class="g-meta-line">' + (g.played === 'never' ? 'Never played' : 'Last played ' + esc(g.played || 'never')) + '</p>');
  lines.push('<p class="g-meta-line">' + (steam ? 'Launches through Steam' : !g.inst ? (g.builds.indexOf('linux') >= 0 ? 'Has a Linux version' : 'Windows only · runs through ' + esc(ST.compat)) : g.os === 'linux' ? 'Runs natively · Linux version' : 'Runs through ' + esc(ST.compat) + ' · Windows version') + '</p>');
  if (s.cloud && g.inst) {
    if (g.cloud === 'conflict') lines.push('<p class="g-meta-line st-line err"><span data-icon="cloud" data-size="12"></span>Cloud save conflict · <button class="linkish" data-tab="cloud">Resolve</button></p>');
    else if (g.cloud === 'off') lines.push('<p class="g-meta-line st-line"><span data-icon="cloud" data-size="12"></span>Cloud saves off for this game</p>');
    else if (g.cloud === 'none') lines.push('<p class="g-meta-line st-line"><span data-icon="cloud" data-size="12"></span>This game has no cloud saves</p>');
    else lines.push('<p class="g-meta-line st-line"><span data-icon="cloud" data-size="12"></span>' + (ST.syncing === g.id ? 'Syncing cloud saves…' : 'Cloud saves synced ' + esc(g.synced)) + '</p>');
  }
  if (g.offline) lines.push('<p class="g-meta-line">Installed from an offline installer</p>');
  if (g.outside) lines.push('<p class="g-meta-line">Installed in Steam, outside GRID</p>');
  if (packed) lines.push('<p class="g-meta-line">' + (packed.how === 'installer' ? 'On the drive as its installer files' + (packed.ver ? ' · v' + packed.ver : '') + ' and saves. Installing runs the installer again.' : 'On the drive as a copy of an installed game' + (packed.ver ? ' · v' + packed.ver : '') + ', with its saves. Installing copies it back.') + '</p>');
  else if (s.id === 'gog') { var onD = bkReady().filter(function (d) { return stPackOf(d, g); }); if (onD.length) lines.push('<p class="g-meta-line bk-on"><span data-icon="drive" data-size="12"></span>On ' + esc(onD.map(function (d) { return d.name; }).join(', ')) + '</p>'); }
  else lines.push('<p class="g-meta-line">' + ST_NOBK[s.id] + '</p>');
  $('d-left').innerHTML = '<div class="g-cover" style="box-shadow:var(--shadow-lift);max-height:320px"><div class="g-art" style="background:' + g.c + ';font-size:20px"><span>' + esc(g.t) + '</span></div></div>' + b.join('') + lines.join('');
  var chips = '<span class="g-chip st-schip">' + stMark(s) + esc(s.name) + '</span>' + (ST.playing === g.id ? '<span class="g-chip playing">Playing</span>' : '') +
    (g.inst && g.ver ? '<span class="g-chip">v' + esc(g.ver) + '</span>' : '') + (g.inst && g.os && g.os !== 'steam' ? '<span class="g-chip">' + (g.os === 'linux' ? 'Linux version' : 'Windows version') + '</span>' : '') +
    (g.offline ? '<span class="g-chip">Offline installer</span>' : '') + (g.ns ? '<span class="g-chip">Not supported</span>' : '');
  $('d-head').innerHTML = '<h2>' + esc(g.t) + '</h2><p class="g-header-line">' + (packed ? '<span class="bk-src">From ' + esc(fromD.name) + '</span> · ' : '') + esc(s.name) + ' · ' + g.year + ' · ' + esc(g.dev) + ' · ' + esc(g.genre) + '</p><div class="row" style="display:flex;gap:6px;flex-wrap:wrap">' + chips + '</div>';
  var tabs = stTabs(g); if (!tabs.some(function (t) { return t[0] === S.tab; })) S.tab = 'overview';
  $('d-tabs').innerHTML = tabs.map(function (t) { return '<button role="tab" data-tab="' + t[0] + '" class="' + (S.tab === t[0] ? 'active' : '') + '" aria-selected="' + (S.tab === t[0]) + '">' + t[1] + '</button>'; }).join('');
  var p = $('d-panel');
  if (S.tab === 'overview') {
    var dlc = g.dlc && g.dlc.length ? '<p class="sec-title" style="margin-top:18px">DLC you own</p><ul class="st-dlc">' + g.dlc.map(function (x, i) {
      var busy = ST.jobs.some(function (k) { return k.kind === 'dlc' && k.t === x.t; });
      return '<li><span class="t">' + esc(x.t.replace(g.t + ' — ', '')) + '</span><span class="s">' + stFmt(x.size) + (x.inst ? ' · Installed' : '') + '</span>' +
        (g.inst ? (x.inst ? '<button class="small-btn" data-st-dlcrm="' + i + '">Remove</button>' : '<button class="small-btn primary" data-st-dlcadd="' + i + '"' + (busy ? ' disabled' : '') + '>' + (busy ? 'Installing…' : 'Install') + '</button>') : '') + '</li>';
    }).join('') + '</ul>' : '';
    p.innerHTML = '<div class="overview"><div class="g-description" style="flex:1 1 340px"><p>' + esc(g.t) + ' is a placeholder title. The real Overview shows the store’s own description here, split into paragraphs.</p><p>Related games aren’t shown for store games: the store doesn’t say which games it relates.</p>' + dlc + '</div>' +
      '<dl class="g-facts" style="flex:0 1 220px"><div><dt>Store</dt><dd>' + esc(s.name) + '</dd></div><div><dt>Developer</dt><dd>' + esc(g.dev) + '</dd></div><div><dt>Released</dt><dd>' + g.year + '</dd></div><div><dt>Genres</dt><dd>' + esc(g.genre) + '</dd></div>' +
      '<div><dt>Size</dt><dd>' + (g.inst ? stFmt(g.disk) + ' on disk' : stFmt(g.size) + ' download') + '</dd></div>' + (g.inst && g.ver ? '<div><dt>Version</dt><dd>' + (g.upd ? 'v' + esc(g.ver) + ' · ' + esc(s.name) + ' v' + esc(g.upd) : 'v' + esc(g.ver)) + '</dd></div>' : '') +
      (g.inst && !steam ? '<div><dt>Folder</dt><dd><code style="font-size:11px;overflow-wrap:anywhere">' + esc(s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '')) + '</code></dd></div>' : '') + '</dl></div>';
  } else if (S.tab === 'media') {
    p.innerHTML = '<p class="sec-title">Screenshots</p><div class="media-row">' + [0, 1, 2, 3].map(function (i) { return '<div class="shot" style="background:' + g.c + ';filter:hue-rotate(' + (i * 25) + 'deg)"></div>'; }).join('') + '</div><p class="note" style="margin-top:10px">From ' + esc(s.name) + '’s store page.</p>';
  } else if (S.tab === 'cloud') {
    p.innerHTML = stCloudHTML(g);
  }
  gIcons($('dialog'));
}
function stCloudHTML(g) {
  var s = stS(g.s), path = g.os === 'linux' ? '~/.local/share/' + g.t.replace(/[^A-Za-z]+/g, '') + '/saves' : s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '') + '/pfx/drive_c/users/steamuser/Saved Games/' + g.t;
  var h = '<div class="st-cloud">';
  if (g.cloud === 'conflict') {
    h += '<div class="st-cloud-state warn"><span class="st-cloud-ico" data-icon="cloud" data-size="18"></span><span><b>These saves changed in two places</b><span class="note">Both this computer’s save and the one in your ' + esc(s.name) + ' account changed since the last sync. Pick the one to keep; the other is replaced.</span></span></div>' +
      '<div class="st-sides"><div class="st-side newer"><b>This computer</b><span class="note">Changed today, 21:40 · 3 files · 2.1 MB</span><button class="g-btn" data-st-keep="local">Keep this computer’s save</button><span class="note">Uploads it to ' + esc(s.name) + '.</span></div>' +
      '<div class="st-side"><b>' + esc(s.name) + ' cloud</b><span class="note">Changed today, 19:12 on another computer · 3 files · 2.0 MB</span><button class="g-btn secondary" data-st-keep="cloud">Use the cloud save</button><span class="note">Replaces the save on this computer.</span></div></div>';
  } else if (g.cloud === 'off') {
    h += '<div class="st-cloud-state"><span class="st-cloud-ico" data-icon="cloud" data-size="18"></span><span><b>Cloud saves are off for this game</b><span class="note">Turn them on in Game Settings, or for every ' + esc(s.name) + ' game in Settings › Stores.</span></span><button class="small-btn" data-st-d="settings">Game Settings</button></div>';
  } else if (g.cloud === 'none') {
    h += '<div class="st-cloud-state"><span class="st-cloud-ico" data-icon="cloud" data-size="18"></span><span><b>This game has no cloud saves</b><span class="note">' + esc(s.name) + ' doesn’t keep saves for it.</span></span></div>';
  } else {
    h += '<div class="st-cloud-state"><span class="st-cloud-ico" data-icon="cloud" data-size="18"></span><span><b>' + (ST.syncing === g.id ? 'Syncing…' : 'Synced ' + esc(g.synced)) + '</b><span class="note">GRID syncs with ' + esc(s.name) + ' before the game starts and after it closes.</span></span><button class="small-btn" data-st-sync' + (ST.syncing === g.id ? ' disabled' : '') + '>' + (ST.syncing === g.id ? 'Syncing…' : 'Sync now') + '</button></div>';
  }
  h += '<div><p class="sec-title">Save locations</p><ul class="st-paths"><li><code>' + esc(path) + '</code></li></ul><p class="note" style="margin-top:6px">' + esc(s.name) + ' says where the game keeps its saves. RomM cloud saves are off for store games.</p></div>';
  return h + '</div>';
}
function stConflictHTML(d) {
  var g = stG(d.gid), s = stS(g.s);
  return stHead(g, 'Which save should ' + esc(g.t) + ' use?', esc(s.name) + ' cloud saves') +
    '<p class="note">Both this computer’s save and the one in your ' + esc(s.name) + ' account changed since the last sync. Pick the one to keep; the other is replaced.</p>' +
    '<div class="st-sides"><div class="st-side newer"><b>This computer</b><span class="note">Changed today, 21:40 · 2.1 MB</span><button class="g-btn" data-st-keep="local">Keep this computer’s save</button></div>' +
    '<div class="st-side"><b>' + esc(s.name) + ' cloud</b><span class="note">Changed today, 19:12 on another computer · 2.0 MB</span><button class="g-btn secondary" data-st-keep="cloud">Use the cloud save</button></div></div>' +
    '<div class="form-actions bk-foot"><span class="note">' + (d.launch ? 'The game starts after you pick.' : '') + '</span><button class="g-btn secondary" data-st-dlg="close">' + (d.launch ? 'Don’t launch' : 'Decide later') + '</button></div>';
}
function stSettingsHTML(d) {
  var g = stG(d.gid), s = stS(g.s), win = g.os === 'windows', dir = s.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '');
  var exes = win ? [g.t.replace(/[^A-Za-z]+/g, '') + '.exe', 'Launcher.exe'] : ['start.sh', 'game/' + g.t.replace(/[^A-Za-z]+/g, '').toLowerCase()];
  return '<h3>Game Settings</h3><div class="st-field"><span>Install folder</span><code>' + esc(dir) + '</code></div>' +
    '<label class="st-field"><span>Executable</span><span class="g-control"><select>' + exes.map(function (x) { return '<option>' + esc(x) + '</option>'; }).join('') + '</select></span></label>' +
    '<p class="help">Set by ' + esc(s.name) + '. Picking another one makes it your choice; ' + esc(s.name) + ' updates won’t change it.</p>' +
    '<label class="st-field"><span>Launch parameters</span><input class="field" placeholder="Optional"></label>' +
    (win ? '<label class="st-field"><span>Compatibility tool</span><span class="g-control"><select>' + ['GE-Proton10-17', 'Proton Experimental (umu)', 'Wine 10.0 (system)', 'None'].map(function (t) { return '<option>' + t + '</option>'; }).join('') + '</select></span></label>' +
      '<div class="st-field"><span>Wine prefix (read-only)</span><code>' + esc(dir) + '/pfx</code></div>' : '<p class="help">Linux games run directly.</p>') +
    (s.cloud && g.cloud !== 'none' ? '<label class="check"><input type="checkbox" data-st-gcloud' + (g.cloud !== 'off' ? ' checked' : '') + '> Sync this game’s cloud saves with ' + esc(s.name) + '</label>' : '') +
    '<div class="form-actions bk-foot"><span></span><button class="g-btn secondary" data-st-dlg="close">Cancel</button><button class="g-btn" data-st-dlg="save-settings">Save</button></div>';
}

/* ---- the offline GOG installer ---- */
var ST_FILES = [
  { n: 'paper_lanterns_1_1_0_60321.sh', ext: 'sh', size: '1.1 GB', kind: 'game', gid: 'g7', os: 'linux', ver: '1.1.0' },
  { n: 'setup_quarry_nine_1.4_(64bit)_(69001).exe', ext: 'exe', size: '9.4 GB', kind: 'game', gid: 'g8', os: 'windows', ver: '1.4', wizard: true },
  { n: 'ashen_tide_1_0_5_71240.sh', ext: 'sh', size: '6.3 GB', kind: 'update', gid: 'g1', os: 'linux', ver: '1.0.5' },
  { n: 'setup_ashen_tide_1.0.4_(64bit)_(71234).exe', ext: 'exe', size: '6.2 GB', kind: 'otheros', gid: 'g1', os: 'windows' },
  { n: 'setup_marrowind_keep_1.2_(64bit)_(70555).exe', ext: 'exe', size: '4.0 GB', kind: 'missingpart', gid: 'g6', os: 'windows', ver: '1.2' },
  { n: 'ashen_tide_digital_artbook_71299.sh', ext: 'sh', size: '118 MB', kind: 'dlc', gid: 'g1', os: 'linux', dlc: 1 },
  { n: 'setup_tidewater_tales_dlc_(55120).exe', ext: 'exe', size: '890 MB', kind: 'nobase', base: 'Tidewater Tales', os: 'windows' },
  { n: 'ashen_tide_1.0.4_71234.pkg', ext: 'pkg', size: '6.0 GB', kind: 'mac' },
  { n: 'Moonrise_Setup.exe', ext: 'exe', size: '2.4 GB', kind: 'unknown' },
  { n: 'manual_ashen_tide.pdf', ext: 'pdf', size: '12 MB', kind: 'unknown' }
];
function stOpenInstaller() { ST.dlg = { kind: 'picker', pick: null }; stRenderDlg(); }
function stPickerHTML(d) {
  return '<h3>Choose a GOG installer</h3><p class="note">Pick a GOG offline installer you downloaded: <code>setup_….exe</code> (Windows) or <code>….sh</code> (Linux). The system’s file chooser opens here in the app; this is a stand-in for it.</p>' +
    '<div class="st-fp-path"><span data-icon="drive" data-size="13"></span>/home/player/Downloads</div><ul class="st-files">' + ST_FILES.map(function (f, i) {
      return '<li><button data-st-file="' + i + '" class="' + (d.pick === i ? 'on' : '') + '"><span class="ext">' + f.ext + '</span>' + esc(f.n) + '<span class="note">' + f.size + '</span></button></li>';
    }).join('') + '</ul><div class="form-actions bk-foot"><span class="note">Multi-part installers: pick the .exe; its .bin parts must sit beside it.</span><button class="g-btn secondary" data-st-dlg="close">Cancel</button><button class="g-btn" data-st-dlg="read"' + (d.pick == null ? ' disabled' : '') + '>Open</button></div>';
}
function stReadingHTML(d) { return '<h3>Reading the installer…</h3><div class="st-reading"><span class="st-spin"></span>Looking for the game’s goggame info in ' + esc(ST_FILES[d.pick].n) + '</div>'; }
function stFoundHTML(d) {
  var f = ST_FILES[d.pick], g = f.gid && stG(f.gid), gog = stS('gog');
  var refuse = function (title, text, acts) { return '<h3>Can’t install this file</h3><div class="st-refuse"><span><b>' + title + '</b><p>' + text + '</p></span></div><div class="form-actions bk-foot"><span></span>' + (acts || '') + '<button class="g-btn" data-st-dlg="close">OK</button></div>'; };
  if (f.kind === 'mac') return refuse('This is a macOS installer', 'GRID installs GOG’s Linux (.sh) and Windows (.exe) installers. Download one of those from your GOG library.', '<button class="g-btn secondary" data-st-dlg="picker">Choose another file…</button>');
  if (f.kind === 'unknown') return refuse('This isn’t a GOG installer', esc(f.n) + ' has no GOG game info in it. GRID reads GOG’s own <code>setup_….exe</code> and <code>….sh</code> installers.', '<button class="g-btn secondary" data-st-dlg="picker">Choose another file…</button>');
  if (f.kind === 'nobase') return refuse('This is DLC for ' + esc(f.base) + ', which isn’t installed', 'Install ' + esc(f.base) + ' first, from GOG or from its own installer. Then install this DLC.', '<button class="g-btn secondary" data-st-dlg="picker">Choose another file…</button>');
  if (f.kind === 'missingpart') return refuse('Part of this installer is missing', 'This installer comes in 6 parts. <code>setup_marrowind_keep_1.2_(64bit)_(70555)-1.bin</code> isn’t in the same folder. Download every part from your GOG library and put them beside the .exe.', '<button class="g-btn secondary" data-st-dlg="picker">Choose another file…</button>');
  if (f.kind === 'otheros') return refuse(esc(g.t) + ' is installed here as the Linux version', 'This is the Windows version’s installer. To switch, uninstall ' + esc(g.t) + ' first, then open this installer again.', '<button class="g-btn secondary" data-st-dlg="picker">Choose another file…</button>');
  var win = f.os === 'windows', dir = gog.folder + '/' + g.t.replace(/[^A-Za-z0-9]+/g, '');
  var title = f.kind === 'dlc' ? 'Install DLC for ' + esc(g.t) : f.kind === 'update' ? 'Update ' + esc(g.t) + ' to ' + f.ver : 'Install ' + esc(g.t);
  var what = f.kind === 'dlc' ? 'Digital Artbook · DLC for ' + esc(g.t) + ' · ' + f.size
    : (win ? 'Windows version' : 'Linux version') + ' · v' + f.ver + ' · ' + f.size;
  var how = f.kind === 'dlc' ? 'Adds to the installed ' + esc(g.t) + ' (the Linux version). GRID unpacks its files into the game’s folder; the installer script isn’t run.'
    : win ? 'GRID runs the installer silently under ' + esc(ST.compat) + '. If it can’t run silently, its own setup window opens: finish it there and GRID adds the game when it closes.'
    : 'GRID unpacks the game files from it. The installer script isn’t run.';
  var already = f.kind === 'update' ? '<p class="bk-warn">' + esc(g.t) + ' is installed already (v' + esc(g.ver) + '). Installing this replaces its files with v' + f.ver + '. Saves and settings stay.</p>' : '';
  return '<h3>' + title + '</h3><div class="st-found"><span class="st-cover-sm" style="background:' + g.c + '">' + esc(g.t) + '</span><span><b>' + esc(g.t) + '</b><span class="note">' + what + '</span><span class="note">From a GOG offline installer · ' + esc(f.n) + '</span></span></div>' +
    '<p class="note">' + how + '</p>' + already +
    (f.kind !== 'dlc' && win ? '<label class="st-field"><span>Compatibility tool</span><span class="g-control"><select data-st-f="compat"><option>GE-Proton10-17</option><option>Proton Experimental (umu)</option><option>Wine 10.0 (system)</option></select></span></label>' : '') +
    (f.kind === 'game' ? '<div class="st-field"><span>Install folder</span><span class="inline"><code>' + esc(dir) + '</code><button class="small-btn" data-st-folder>Change…</button></span></div>' : '') +
    (f.kind !== 'game' ? '' : '<p class="note">' + (gog.conn === 'ok' ? 'It joins your GOG games. Updates and cloud saves come from GOG while you’re signed in.' : 'It joins GOG › Installed. Sign in to GOG later for its updates and cloud saves.') + '</p>') +
    '<div class="form-actions bk-foot"><span class="note">Needs ' + (f.kind === 'dlc' ? '160 MB' : stFmt(g.disk)) + ' · 412 GB free</span><button class="g-btn secondary" data-st-dlg="close">Cancel</button><button class="g-btn" data-st-dlg="run-installer">' + (f.kind === 'dlc' ? 'Install DLC' : f.kind === 'update' ? 'Update' : 'Install') + '</button></div>';
}
function stRunInstaller() {
  var f = ST_FILES[ST.dlg.pick], g = stG(f.gid); stClose();
  if (f.kind === 'dlc') { stQueue(g, 'dlc', { t: 'Ashen Tide — Digital Artbook' }); stToast('Ashen Tide — Digital Artbook added to download queue'); return; }
  var j = stQueue(g, f.kind === 'update' ? 'installer' : 'installer', { os: f.os });
  if (f.kind === 'update') g.ver = f.ver;
  stToast(g.t + ' added to download queue', '', ['Open Downloads', function () { S.view = 'downloads'; renderAll(); }]);
  if (f.wizard) setTimeout(function () { if (j.st === 'active') { j.wizard = true; stToast('The ' + g.t + ' installer couldn’t run silently, so its setup window opened. Finish it there; GRID adds the game when it closes.'); renderDownloads(); } }, 2400);
}

/* ---- Downloads: store rows ---- */
var stOrigDl = renderDownloads;
renderDownloads = function () {
  stOrigDl();
  var act = ST.jobs.filter(function (j) { return j.st === 'active'; }), q = ST.jobs.filter(function (j) { return j.st === 'queued'; });
  var row = function (j, live) {
    var romm = j.s === 'romm', s = romm ? null : stS(j.s), steam = j.kind === 'steam', kind = ST_KIND[j.kind];
    var line = j.wizard ? 'Waiting for the installer’s setup window · finish it there' : stJobLine(j);
    if (romm) line = PLAT[j.p] + ' · ' + line;
    return '<div class="dl" style="margin-top:8px"><div><span class="t">' + esc(j.t) + '</span>' + (romm ? '' : '<span class="kind st-schip" style="display:inline-flex;align-items:center;gap:4px">' + stMark(s) + esc(s.name) + '</span>') + (kind ? '<span class="kind">' + kind + '</span>' : '') +
      '<div class="s">' + esc(line) + (steam ? ' · Managed by Steam' : '') + '</div>' + (live ? '<div class="bar"><b style="width:' + Math.round(j.pct) + '%"></b></div>' : '') + '</div>' +
      (live ? '<svg width="120" height="38" viewBox="0 0 120 18" preserveAspectRatio="none"><polyline fill="none" stroke="' + (j.pct > 85 || j.kind === 'installer' || j.kind === 'repair' ? 'var(--graph-disk)' : 'var(--primary)') + '" stroke-width="1.2" points="0,12 20,8 40,10 60,6 80,9 100,7 120,8"/></svg>' : '<span></span>') +
      (steam ? '<button class="small-btn" disabled title="Managed by Steam">Cancel</button>' : '<button class="small-btn" data-st-cancel="' + j.id + '">Cancel</button>') + '</div>';
  };
  var fixN = function (id, add) { var e = $(id); e.textContent = +e.textContent + add; };
  if (act.length) { var a = $('dl-active'); if (a.querySelector('.note')) a.innerHTML = ''; a.insertAdjacentHTML('beforeend', act.map(function (j) { return row(j, true); }).join('')); fixN('dl-active-n', act.length); }
  if (q.length) { var qq = $('dl-queued'); if (qq.querySelector('.note')) qq.innerHTML = ''; qq.insertAdjacentHTML('beforeend', q.map(function (j) { return row(j, false); }).join('')); fixN('dl-queued-n', q.length); }
  if (ST.done.length) {
    var d = $('dl-done'); if (d.querySelector('.note')) d.innerHTML = '';
    d.insertAdjacentHTML('afterbegin', ST.done.map(function (x, i) { var s = stS(x.s); return '<div class="dl" style="margin-bottom:8px"><div><span class="t">' + esc(x.t) + '</span>' + (s ? '<span class="kind" style="display:inline-flex;align-items:center;gap:4px">' + stMark(s) + esc(s.name) + '</span>' : '') + (ST_KIND[x.kind] ? '<span class="kind">' + ST_KIND[x.kind] + '</span>' : '') + '<div class="s' + (x.err ? ' err' : '') + '">' + esc(x.line) + '</div></div><span></span>' + (x.err ? '<span style="display:flex;gap:6px"><button class="small-btn" data-st-retry="' + i + '">Retry</button><button class="small-btn" data-st-undone="' + i + '">Dismiss</button></span>' : '<button class="small-btn" data-st-undone="' + i + '">Dismiss</button>') + '</div>'; }).join(''));
    fixN('dl-done-n', ST.done.length);
  }
  var first = act.filter(function (j) { return j.kind !== 'steam'; })[0] || act[0];
  if (first && !S.dl && !BK.job) {
    $('strip').hidden = false;
    $('strip-line').innerHTML = esc(first.t) + ' · ' + esc(first.s === 'romm' ? PLAT[first.p] : stS(first.s).name) + ' · ' + esc(stJobLine(first));
  }
};

/* ---- Settings › Stores ---- */
function stToolLine(s) {
  if (s.tool === 'installing') { var j = ST.jobs.filter(function (x) { return x.kind === 'tool' && x.s === s.id; })[0]; return '<span class="st-tool"><span>Installing ' + s.toolName + '…</span><span class="bar"><b style="width:' + (j ? Math.round(j.pct) : 10) + '%"></b></span></span>'; }
  if (s.tool === 'failed') return '<span class="st-tool"><span class="st-errline">Couldn’t install ' + s.toolName + ': the download didn’t match its checksum, so GRID deleted it.</span><button class="small-btn primary" data-st-tool="' + s.id + '">Try again</button></span>';
  if (s.tool === 'missing') return '<span class="st-tool"><span>Not installed · ' + s.toolSize + '</span><button class="small-btn primary" data-st-tool="' + s.id + '">Install</button></span>';
  return '<span class="st-tool"><span>' + s.toolName + ' ' + s.toolVer + ' · installed</span></span>';
}
function stStoreCard(s) {
  var on = s.conn !== 'off';
  var pill = s.conn === 'ok' ? '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Connected</span>' : s.conn === 'expired' ? '<span class="pill-s warn">Login expired</span>' : '<span class="pill-s">Not connected</span>';
  var head = '<header class="st-store-head">' + stMark(s, 'lg') + '<div class="st-store-id"><b>' + esc(s.name) + '</b><span class="note">' + (on ? 'Signed in as ' + esc(s.account) : s.id === 'gog' && stInst(s).length ? 'Not signed in · ' + stPlural(stInst(s).length, 'game') + ' installed here' : 'Not signed in') + '</span></div>' + pill + '</header>';
  var src = '<a href="#" class="linkish" data-toast="Opens https://' + s.src + ' in your browser">' + s.src + '</a>';
  var toolRow = '<div><dt>Store tool</dt><dd>' + stToolLine(s) + '</dd></div><div><dt>License</dt><dd>' + s.toolPkg + ' · ' + s.license + ' · ' + src + '</dd></div>';
  var folderRow = '<div><dt>Install folder</dt><dd><code>' + esc(s.folder) + '</code><button class="small-btn" data-toast="Opens the folder chooser; new ' + syEsc(s.name) + ' installs go there">Change…</button></dd></div>';
  var body = '';
  if (ST.confirm === 'disc:' + s.id) {
    body = '<div class="bk-confirm"><p><b>Disconnect ' + esc(s.name) + '?</b> GRID deletes your ' + esc(s.name) + ' login and ' + s.toolName + ' from this computer. ' + (stInst(s).length ? 'The ' + stPlural(stInst(s).length, 'installed game') + ' stay and still play; ' + (s.cloud ? 'they can’t install, update or sync cloud saves' : 'they can’t install or update') + ' until you connect again.' : '') + '</p>' +
      '<div class="form-actions"><button class="g-btn destructive" data-st-disc-go="' + s.id + '">Disconnect</button><button class="g-btn secondary" data-st-confirm-cancel>Cancel</button></div></div>';
  } else if (!on) {
    body = '<p class="note st-lede">Sign in to ' + esc(s.name) + ' to see the games you own in the Library and install them. ' + (s.id === 'gog' ? 'You can install GOG offline installers without signing in. ' : '') + 'GRID installs with ' + s.toolName + ' (' + s.toolPkg + ', ' + s.license + ', ' + s.toolSize + '), which it downloads when you connect.</p>' +
      (ST.loginErr && ST.loginErr.s === s.id ? '<p class="st-errline">' + esc(ST.loginErr.msg) + '</p>' : '') +
      '<div class="form-actions"><button class="g-btn" data-st-connect="' + s.id + '">Connect ' + esc(s.name) + '…</button>' + (s.id === 'gog' ? '<button class="g-btn secondary" data-st-installer>Install from installer…</button>' : '') + '</div>' +
      '<dl class="kv">' + toolRow.replace(stToolLine(s), '<span class="st-tool"><span>Installs when you connect · ' + s.toolSize + '</span></span>') + '</dl>';
  } else {
    body = (s.conn === 'expired' ? '<p class="bk-warn"><b>Your ' + esc(s.name) + ' login expired.</b> Sign in again to see new games, install and update. Installed games still play.</p>' : '') +
      '<dl class="kv"><div><dt>Account</dt><dd>' + esc(s.account) + '</dd></div><div><dt>Games you own</dt><dd>' + (s.loading ? '<span class="st-spin"></span> Loading…' : stOf(s.id).length + ' · ' + stInst(s).length + ' installed · updated ' + s.refreshed) + '</dd></div>' + toolRow + folderRow +
      (s.cloud ? '<div><dt>Cloud saves</dt><dd><label class="check"><input type="checkbox" checked data-st-cloudall="' + s.id + '"> Sync before a game starts and after it closes</label></dd></div>' : '') + '</dl>' +
      '<div class="form-actions">' + (s.conn === 'expired' ? '<button class="g-btn" data-st-login="' + s.id + '">Sign in again…</button>' : '<button class="g-btn" data-st-show="' + s.id + '">Show in Library</button>') +
      (s.id === 'gog' ? '<button class="g-btn secondary" data-st-installer>Install from installer…</button>' : '') + '<button class="g-btn secondary" data-st-refresh="' + s.id + '">Refresh games</button><button class="g-btn secondary destructive" data-st-disc="' + s.id + '">Disconnect…</button></div>';
  }
  return '<article class="st-store' + (on ? '' : ' off') + '">' + head + body + '<span class="bk-proto-corner">' + stProtoSelect(s) + '</span></article>';
}
function stSteamCard(s) {
  var on = s.conn !== 'off';
  var pill = on ? (s.key === 'ok' ? '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Connected</span>' : '<span class="pill-s warn">Installed games only</span>') : '<span class="pill-s">Not connected</span>';
  var head = '<header class="st-store-head">' + stMark(s, 'lg') + '<div class="st-store-id"><b>Steam</b><span class="note">' + (on ? 'SteamID64 ' + esc(s.steamId) + ' · ' + esc(s.account) : 'Not set up') + '</span></div>' + pill + '</header>';
  if (ST.confirm === 'disc:steam') {
    return '<article class="st-store">' + head + '<div class="bk-confirm"><p><b>Disconnect Steam?</b> GRID forgets your SteamID64 and deletes your Web API key from the OS keyring. Steam games leave the Library; they stay installed in Steam.</p><div class="form-actions"><button class="g-btn destructive" data-st-disc-go="steam">Disconnect</button><button class="g-btn secondary" data-st-confirm-cancel>Cancel</button></div></div></article>';
  }
  var client = s.client ? '<p class="st-okline"><span data-icon="check" data-size="14"></span>Steam client found · <code>' + esc(s.clientPath) + '</code></p>'
    : '<p class="st-errline">Steam isn’t installed. GRID installs, starts and uninstalls Steam games through the Steam client. Install Steam, sign in once, then come back. <button class="small-btn" data-st-recheck>Check again</button></p>';
  var form = ST.steamForm || (ST.steamForm = { pick: s.steamId, manual: '', key: '', test: null, editKey: s.key !== 'ok' });
  var accts = '<fieldset class="st-acct"><legend>Account</legend>' + s.accounts.map(function (a) {
      return '<label><input type="radio" name="st-acct" value="' + a[1] + '"' + (form.pick === a[1] ? ' checked' : '') + '> <b>' + esc(a[0]) + '</b> <code>' + a[1] + '</code> <span class="note">' + esc(a[2]) + '</span></label>';
    }).join('') + '<label><input type="radio" name="st-acct" value="manual"' + (form.pick === 'manual' ? ' checked' : '') + '> Enter a SteamID64</label>' +
    (form.pick === 'manual' ? '<input class="field" id="st-sid" placeholder="17 digits, starting 7656" value="' + syEsc(form.manual) + '" style="max-width:320px">' : '') +
    '<span class="help">Found in Steam’s files on this computer. A SteamID64 is the long number in your profile’s address.</span></fieldset>';
  var key = s.key === 'ok' && !form.editKey
    ? '<div class="st-keyrow"><input class="field" value="••••••••••••••••••••••••••••' + s.keyTail + '" disabled aria-label="Web API key, saved"><button class="small-btn" data-st-key="edit">Change…</button><button class="small-btn" data-st-key="remove">Remove</button></div>'
    : '<div class="st-keyrow"><input class="field" id="st-key" type="password" placeholder="Paste your 32-character key" value="' + syEsc(form.key) + '" autocomplete="off"></div>';
  var test = form.test === 'ok' ? '<p class="st-okline"><span data-icon="check" data-size="14"></span>The key works · ' + stOf('steam').length + ' games owned</p>'
    : form.test === 'bad' ? '<p class="st-errline">Steam rejected the key (403 Forbidden). Check that you copied all 32 characters, or make a new key.</p>'
    : form.test === 'private' ? '<p class="st-errline">The key works, but this profile’s game details are private, so Steam lists no games.</p><p class="help">In Steam: your profile › Edit Profile › Privacy Settings › set Game details to Public. Then test again.</p>'
    : form.test === 'sid' ? '<p class="st-errline">That isn’t a SteamID64. It has 17 digits and starts with 7656.</p>'
    : form.test === 'net' ? '<p class="st-errline">Couldn’t reach api.steampowered.com. Check your connection and try again.</p>'
    : form.test === 'busy' ? '<p class="note"><span class="st-spin"></span> Asking Steam…</p>' : '';
  return '<article class="st-store' + (on ? '' : ' off') + '">' + head +
    '<p class="note st-lede">Installed Steam games show in the Library as soon as Steam is set up here, including games you install in Steam itself. To list the games you own but haven’t installed, GRID needs your SteamID64 and your own Steam Web API key.</p>' + client + accts +
    '<div class="sub"><h4 style="margin:0;font-size:13px;color:var(--text-h)">Web API key</h4>' + key +
    '<span class="help">Make one at <a href="#" class="linkish" data-toast="Opens https://steamcommunity.com/dev/apikey in your browser">steamcommunity.com/dev/apikey</a> (any domain name works). GRID keeps it in the OS keyring and sends it only to Steam.</span>' + test + '</div>' +
    '<div class="form-actions"><button class="g-btn" data-st-steam="save">' + (on ? 'Save' : 'Connect Steam') + '</button><button class="g-btn secondary" data-st-steam="test">Test</button>' + (on ? '<button class="g-btn secondary" data-st-show="steam">Show in Library</button><button class="g-btn secondary destructive" data-st-disc="steam">Disconnect…</button>' : '') + '</div>' +
    '<button class="small-btn bk-proto" data-st-steamtest style="align-self:flex-start">Prototype: next Test result (' + ['works', 'rejected', 'private', 'not a SteamID64', 'no network'][ST.testIdx || 0] + ')</button>' +
    '<span class="bk-proto-corner">' + stProtoSelect(s) + '</span></article>';
}
function renderStoresPane(el) {
  el.innerHTML = '<h2>Stores</h2><p class="note st-lede">Connect the stores you buy games from. Each connected store gets a group in the Library with its Installed and Available games. GRID installs GOG, Epic and Amazon games with the open-source tools Heroic Games Launcher uses; their logins stay in those tools’ files in GRID’s data folder. Steam games install and start through the Steam client.</p>' +
    ST.order.map(function (id) { return id === 'steam' ? stSteamCard(stS(id)) : stStoreCard(stS(id)); }).join('');
  gIcons(el);
}

/* ---- the store login window ---- */
function stLoginOpen(sid) {
  var s = stS(sid);
  ST.login = { s: sid, phase: 'loading', err: null };
  stLoginRender();
  setTimeout(function () { if (ST.login && ST.login.phase === 'loading') { ST.login.phase = 'page'; stLoginRender(); } }, 700);
}
function stLoginRender() {
  var L = ST.login, box = $('stlogin'); if (!L) { box.hidden = true; return; }
  var s = stS(L.s), host = s.id === 'epic' ? 'www.epicgames.com/id/login' : s.id === 'amazon' ? 'www.amazon.com/ap/signin' : 'login.gog.com/auth';
  var page = L.phase === 'loading' ? '<div class="st-reading"><span class="st-spin"></span>Loading ' + esc(host.split('/')[0]) + '…</div>'
    : L.phase === 'done' ? '<div class="st-reading"><span data-icon="check" data-size="18"></span>Signed in. Closing…</div>'
    : '<div class="st-win-ph"><b>' + esc(s.name) + ' sign-in page</b><span class="note">The store’s page loads here, as it would in a browser: email, password, two-step codes, captchas. GRID never sees what you type.</span><input class="field" placeholder="(the store’s fields)" disabled><input class="field" placeholder="" disabled></div>';
  var err = L.err ? '<p class="bk-warn st-win-err">' + esc(L.err) + '</p>' : '';
  box.innerHTML = '<div class="st-win" role="dialog" aria-modal="true" aria-label="Sign in to ' + syEsc(s.name) + '"><div class="st-win-bar">' + stMark(s) + '<span>Sign in to ' + esc(s.name) + ' — GRID</span><button class="g-icon-btn" aria-label="Close" data-st-login-x><span data-icon="close" data-size="18"></span></button></div>' +
    '<div class="st-win-url"><span class="lock">🔒</span>https://' + esc(host) + '</div><div class="st-win-page">' + err + page + '</div>' +
    '<div class="st-win-foot"><span class="note">GRID never sees your password. This window closes on its own when you’ve signed in.</span><button class="small-btn bk-proto" data-st-login-ok>Prototype: finish sign-in</button><button class="small-btn bk-proto" data-st-login-fail>Prototype: fail</button><button class="small-btn" data-st-login-x>Cancel</button></div></div>';
  box.hidden = false; gIcons(box);
}
function stLoginDone() {
  var L = ST.login, s = stS(L.s); L.phase = 'done'; stLoginRender();
  setTimeout(function () {
    ST.login = null; stLoginRender();
    var was = s.conn; s.conn = 'ok'; ST.loginErr = null; ST.proto[s.id] = '';
    stToast((was === 'expired' ? 'Signed in to ' : 'Connected to ') + s.name + ' as ' + s.account);
    if (s.tool !== 'ok') stInstallTool(s); else stLoadOwned(s);
    renderAll();
  }, 600);
}
function stLoginFail(why) {
  var L = ST.login, s = stS(L.s);
  ST.login = null; stLoginRender();
  ST.loginErr = why ? { s: s.id, msg: why } : null;
  if (why) stToast('Couldn’t sign in to ' + s.name + ': ' + why.replace(/^Sign-in didn’t finish: /, ''), 'error', ['Try again', function () { stLoginOpen(s.id); }]);
  renderAll();
}
function stInstallTool(s) {
  s.tool = 'installing';
  var j = stQueue(null, 'tool', { t: s.toolName, s: s.id });
  stToast('Installing ' + s.toolName + ' (' + s.toolSize + ') for ' + s.name + '…');
  renderAll();
}
function stLoadOwned(s) {
  s.loading = true; renderAll();
  setTimeout(function () { s.loading = false; s.refreshed = 'just now'; stToast(s.name + ': ' + stPlural(stOf(s.id).length, 'game') + ' found · ' + stAvail(s).length + ' to install'); renderAll(); }, 1800);
}

/* ---- events ---- */
document.addEventListener('click', function (e) {
  var t = e.target, b;
  var card = t.closest('[data-sgame]');
  if (card && !card.classList.contains('st-skel')) {
    var g = stG(card.getAttribute('data-sgame')), a = t.closest('[data-st-act]'), act = a ? a.getAttribute('data-st-act') : 'details';
    if (ST.sel && card.closest('.grid.selecting')) { var si = ST.sel.ids.indexOf(g.id); if (si >= 0) ST.sel.ids.splice(si, 1); else if (!card.classList.contains('st-nosel')) ST.sel.ids.push(g.id); renderLibrary(); return; }
    var dv = card.getAttribute('data-sdrive');
    if (dv) { ST.detFrom = dv; if (act === 'install') { stRestore(g, bkDrive(dv)); return; } if (act === 'play') { stPlay(g); return; } stOpenDetails(g); return; }
    ST.detFrom = null;
    if (a && a.disabled) return;
    if (act === 'play') stPlay(g); else if (act === 'install') stOpenInstall(g); else if (act === 'cloud') stOpenDetails(g, 'cloud'); else stOpenDetails(g);
    return;
  }
  if ((b = t.closest('[data-st-go]'))) { S.libFilter = b.getAttribute('data-st-go'); renderLibrary(); return; }
  if ((b = t.closest('[data-st-goset]'))) { stClose(); closeDetails(); S.view = 'settings'; S.setPane = 'stores'; renderAll(); return; }
  if ((b = t.closest('[data-st-show]'))) { S.view = 'library'; S.libFilter = 'st:' + b.getAttribute('data-st-show') + ':inst'; renderAll(); return; }
  if ((b = t.closest('[data-st-refresh]'))) { var rs = stS(b.getAttribute('data-st-refresh')); if (rs.id === 'steam' && rs.key === 'private') { stToast('Steam still lists no games: the profile’s game details are private', 'error'); return; } stLoadOwned(rs); return; }
  if (t.closest('[data-st-installer]')) { stOpenInstaller(); return; }
  if ((b = t.closest('[data-st-tool]'))) { stInstallTool(stS(b.getAttribute('data-st-tool'))); return; }
  if ((b = t.closest('[data-st-connect]'))) { var cs = stS(b.getAttribute('data-st-connect')); if (cs.id === 'steam') { S.view = 'settings'; S.setPane = 'stores'; renderAll(); return; } stLoginOpen(cs.id); return; }
  if ((b = t.closest('[data-st-login]'))) { stClose(); stLoginOpen(b.getAttribute('data-st-login')); return; }
  if (t.closest('[data-st-login-x]')) { stLoginFail(null); return; }
  if (t.closest('[data-st-login-ok]')) { stLoginDone(); return; }
  if (t.closest('[data-st-login-fail]')) {
    ST.failIdx = ((ST.failIdx || 0) + 1) % 2;
    if (ST.failIdx === 1) { ST.login.err = 'Couldn’t reach ' + stS(ST.login.s).host + '. Check your connection; the page reloads when you try again.'; stLoginRender(); }
    else stLoginFail('Sign-in didn’t finish: ' + stS(ST.login.s).name + ' returned an error. Try again.');
    return;
  }
  if (t.closest('[data-st-recheck]')) { var ss = stS('steam'); ss.client = true; ST.proto.steam = ''; stApplyProto('steam', ''); stToast('Steam client found'); renderAll(); return; }
  /* Details */
  if ((b = t.closest('[data-st-d]'))) {
    var g2 = S.stDetail, d = b.getAttribute('data-st-d'), s2 = stS(g2.s);
    if (d === 'play') stPlay(g2);
    else if (d === 'stop') { if (s2.id === 'steam') { stToast(g2.t + ' is running in Steam. Quit it from the game.'); return; } ST.playing = null; stToast('Stopped ' + g2.t); if (s2.cloud && g2.cloud === 'synced') setTimeout(function () { stToast('Cloud saves uploaded to ' + s2.name + ' for ' + g2.t); }, 800); stRenderDetails(); renderAll(); }
    else if (d === 'install') stOpenInstall(g2);
    else if (d === 'cancel') { var j = stJob(g2); ST.jobs.splice(ST.jobs.indexOf(j), 1); ST.done.unshift({ t: j.t, s: j.s, kind: j.kind, line: 'Cancelled' }); stToast('Cancelled ' + g2.t); renderAll(); stRenderDetails(); }
    else if (d === 'update') { stQueue(g2, 'update'); stToast('Update for ' + g2.t + ' added to download queue'); stRenderDetails(); }
    else if (d === 'verify') { if (s2.id === 'steam') { stToast('Steam is verifying ' + g2.t + '’s files. Watch it in Steam.'); return; } stQueue(g2, 'repair'); stToast('Verifying ' + g2.t + '’s files…'); stRenderDetails(); }
    else if (d === 'settings') { ST.dlg = { kind: 'settings', gid: g2.id }; stRenderDlg(); }
    else if (d === 'backup') stOpenBackup([g2.id], 'Backup ' + g2.t);
    else if (d === 'restore') stRestore(g2, bkDrive(ST.detFrom));
    else if (d === 'drm') { S.stConfirm = 'drm'; stRenderDetails(); }
    else if (d === 'drm-go') { var dd2 = bkDrive(ST.detFrom); dd2.sgames = dd2.sgames.filter(function (x) { return x.id !== g2.id; }); toast('Removed ' + g2.t + ' from ' + dd2.name); closeDetails(); renderAll(); }
    else if (d === 'uninstall') { if (s2.id === 'steam') { stToast('Steam opened its uninstall window for ' + g2.t + '. It leaves the Library once Steam removes it.'); return; } S.stConfirm = 'uninstall'; stRenderDetails(); }
    else if (d === 'uninstall-go') { g2.inst = false; delete g2.upd; S.stConfirm = null; stToast(g2.t + ' uninstalled' + (s2.cloud ? '. Its cloud saves stay in your ' + s2.name + ' account.' : '')); renderAll(); stRenderDetails(); }
    return;
  }
  if (t.closest('[data-st-sync]')) { var g3 = S.stDetail; ST.syncing = g3.id; stRenderDetails(); setTimeout(function () { ST.syncing = null; g3.synced = 'just now'; stToast('Cloud saves synced for ' + g3.t); stRenderDetails(); }, 1200); return; }
  if ((b = t.closest('[data-st-keep]'))) {
    var g4 = ST.dlg && ST.dlg.kind === 'conflict' ? stG(ST.dlg.gid) : S.stDetail, launch = ST.dlg && ST.dlg.launch, keep = b.getAttribute('data-st-keep');
    g4.cloud = 'synced'; g4.synced = 'just now'; stClose();
    stToast(keep === 'local' ? 'Kept this computer’s save for ' + g4.t + ' and uploaded it' : 'Restored the cloud save for ' + g4.t);
    renderAll(); if (S.stDetail) stRenderDetails(); if (launch) stPlay(g4);
    return;
  }
  if ((b = t.closest('[data-st-dlcadd]'))) { var x = S.stDetail.dlc[+b.getAttribute('data-st-dlcadd')]; stQueue(S.stDetail, 'dlc', { t: x.t }); stToast(x.t + ' added to download queue'); stRenderDetails(); return; }
  if ((b = t.closest('[data-st-dlcrm]'))) { var y = S.stDetail.dlc[+b.getAttribute('data-st-dlcrm')]; y.inst = false; stToast(y.t + ' removed'); stRenderDetails(); return; }
  /* Dialogs */
  if ((b = t.closest('[data-st-dlg]'))) {
    var k = b.getAttribute('data-st-dlg');
    if (k === 'close') stClose();
    else if (k === 'backup-go') stBackupGo();
    else if (k === 'install') { var g5 = stG(ST.dlg.gid), dd = ST.dlg; stClose(); stQueue(g5, 'install', { os: dd.os }); (g5.dlc || []).forEach(function (x2, i) { if (dd.dlc[i]) x2.inst = true; }); stToast(g5.t + ' added to download queue', '', ['Open Downloads', function () { S.view = 'downloads'; renderAll(); }]); }
    else if (k === 'save-settings') { var g6 = stG(ST.dlg.gid), c = document.querySelector('[data-st-gcloud]'); if (c) g6.cloud = c.checked ? (g6.cloud === 'off' ? 'synced' : g6.cloud) : 'off'; g6.synced = g6.synced || 'just now'; stClose(); stToast('Game Settings saved'); if (S.stDetail) stRenderDetails(); }
    else if (k === 'picker') { ST.dlg = { kind: 'picker', pick: null }; stRenderDlg(); }
    else if (k === 'read') { ST.dlg.kind = 'reading'; stRenderDlg(); setTimeout(function () { if (ST.dlg && ST.dlg.kind === 'reading') { ST.dlg.kind = 'found'; stRenderDlg(); } }, 800); }
    else if (k === 'run-installer') stRunInstaller();
    return;
  }
  if (t === $('stscrim')) { stClose(); return; }
  if ((b = t.closest('[data-st-file]'))) { ST.dlg.pick = +b.getAttribute('data-st-file'); stRenderDlg(); return; }
  if (t.closest('[data-st-folder]')) { toast('Opens the folder chooser'); return; }
  if (t.closest('[data-st-small]')) { ST.dlg.small = !ST.dlg.small; stRenderDlg(); return; }
  if ((b = t.closest('[data-st-cancel]'))) { var id = b.getAttribute('data-st-cancel'), jj = ST.jobs.filter(function (q) { return q.id === id; })[0]; if (jj) { ST.jobs.splice(ST.jobs.indexOf(jj), 1); if (jj.kind === 'tool') stS(jj.s).tool = 'missing'; ST.done.unshift({ t: jj.t, s: jj.s, kind: jj.kind, line: 'Cancelled' }); toast('Cancelled ' + jj.t); renderAll(); } return; }
  if ((b = t.closest('[data-st-undone]'))) { ST.done.splice(+b.getAttribute('data-st-undone'), 1); renderDownloads(); return; }
  if ((b = t.closest('[data-st-retry]'))) { var r = ST.done.splice(+b.getAttribute('data-st-retry'), 1)[0]; if (r.s === 'romm') { startInstall(G[r.gid]); return; } var rg = ST.games.filter(function (q) { return q.t === r.t; })[0]; if (rg) { stQueue(rg, 'install', { os: rg.builds[0] }); toast(rg.t + ' added to download queue'); } return; }
  /* Settings › Stores */
  if ((b = t.closest('[data-st-disc]'))) { ST.confirm = 'disc:' + b.getAttribute('data-st-disc'); renderSettings(); return; }
  if (t.closest('[data-st-confirm-cancel]')) { ST.confirm = null; renderSettings(); return; }
  if ((b = t.closest('[data-st-disc-go]'))) {
    var ds = stS(b.getAttribute('data-st-disc-go')); ds.conn = 'off'; ST.confirm = null; ST.proto[ds.id] = 'off';
    if (ds.id === 'steam') { ST.games.forEach(function (q) { if (q.s === 'steam') q.inst = false; }); ds.key = 'none'; ST.steamForm = null; stToast('Steam disconnected. Your key was deleted from the keyring.'); }
    else { ds.tool = 'missing'; stToast(ds.name + ' disconnected. Its login and ' + ds.toolName + ' were deleted; installed games stay.'); }
    renderAll(); return;
  }
  if ((b = t.closest('[data-st-key]'))) { var kk = b.getAttribute('data-st-key'), st = stS('steam'); if (kk === 'edit') ST.steamForm.editKey = true; else { st.key = 'none'; ST.steamForm.editKey = true; stToast('Web API key removed from the keyring. Steam’s Available list needs it.'); } renderAll(); return; }
  if (t.closest('[data-st-steamtest]')) { ST.testIdx = ((ST.testIdx || 0) + 1) % 5; renderSettings(); return; }
  if ((b = t.closest('[data-st-steam]'))) {
    var f = ST.steamForm, stm = stS('steam'), kv = $('st-key'); if (kv) f.key = kv.value; var sv = $('st-sid'); if (sv) f.manual = sv.value;
    var res = ['ok', 'bad', 'private', 'sid', 'net'][ST.testIdx || 0];
    f.test = 'busy'; renderSettings();
    setTimeout(function () {
      f.test = res;
      if (b.getAttribute('data-st-steam') === 'save' && (res === 'ok' || res === 'private')) {
        stApplyProto('steam', res === 'ok' ? '' : 'private');
        f.editKey = false; f.key = '';
        if (f.pick !== 'manual') stm.steamId = f.pick;
        stToast(res === 'ok' ? 'Steam connected · ' + stOf('steam').length + ' games owned' : 'Steam connected, but your game details are private, so Available is empty', res === 'ok' ? '' : 'error');
      }
      renderAll();
    }, 700);
    return;
  }
}, false);
document.addEventListener('change', function (e) {
  var t = e.target, p = t.getAttribute && t.getAttribute('data-st-proto');
  if (p) { stApplyProto(p, t.value); if (t.value === 'tool' || t.value === 'toolfail') { /* nothing more */ } renderAll(); return; }
  if (t.name === 'st-os') { ST.dlg.os = t.value; stRenderDlg(); return; }
  if (t.name === 'st-acct') { ST.steamForm.pick = t.value; ST.steamForm.test = null; renderSettings(); return; }
  var f = t.getAttribute && t.getAttribute('data-st-f'); if (f && ST.dlg) { ST.dlg[f] = t.value; if (ST.dlg.kind === 'install') stRenderDlg(); return; }
  var dl = t.getAttribute && t.getAttribute('data-st-dlc'); if (dl != null && ST.dlg) { ST.dlg.dlc[+dl] = t.checked; stRenderDlg(); return; }
  var ca = t.getAttribute && t.getAttribute('data-st-cloudall'); if (ca) { stToast(t.checked ? 'Cloud saves on for ' + stS(ca).name + ' games' : 'Cloud saves off for ' + stS(ca).name + ' games. Your saves stay in your account.'); return; }
});
document.addEventListener('keydown', function (e) {
  if (e.key !== 'Escape') return;
  if (ST.login) { e.stopImmediatePropagation(); stLoginFail(null); }
  else if (ST.dlg) { e.stopImmediatePropagation(); stClose(); }
  else if (S.stDetail) { e.stopImmediatePropagation(); closeDetails(); }
  else if (ST.sel) { e.stopImmediatePropagation(); ST.sel = null; renderLibrary(); }
}, true);
var stOrigDet = renderDetails;
renderDetails = function () { if (S.stDetail) { stRenderDetails(); return; } stOrigDet(); riDetails(); };
var stOrigClose = closeDetails;
closeDetails = function () { S.stDetail = null; S.stConfirm = null; ST.detFrom = null; stOrigClose(); };

function stBoot() {
  S.view = 'library'; S.libFilter = 'st:gog:inst'; S.platform = 'PS';
  /* One GOG install is running and one is queued, so progress shows on its card. */
  var cp = stG('g4'); ST.jobs.push({ id: 'jboot', gid: 'g4', t: cp.t, s: 'gog', kind: 'install', st: 'active', pct: 38, os: 'linux' });
  var hv = stG('g5'); ST.jobs.push({ id: 'jboot2', gid: 'g5', t: hv.t, s: 'gog', kind: 'install', st: 'queued', pct: 0, os: 'windows' });
  var qc = stG('s4'); ST.jobs.push({ id: 'jboot3', gid: 's4', t: qc.t, s: 'steam', kind: 'steam', st: 'active', pct: 52 });
  ST.done.push({ t: 'Glimmer Deep', s: 'epic', kind: 'install', line: 'Failed · legendary stopped: Epic’s download server didn’t answer', err: true });
  paintArt(stG('g1')); bkSession(); renderAll(); gIcons(); stRun();
  setTimeout(function () { stToast('Lanternfall: update 2.2 is available on GOG', '', ['Show', function () { S.view = 'library'; S.libFilter = 'updates'; renderAll(); }]); }, 900);
}
