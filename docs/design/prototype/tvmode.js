/* ==== GRID TV mode prototype. A controller drives everything: the d-pad (or left
   stick) moves focus, A selects, B goes back, X and Y are the screen's two extra
   actions, LB / RB switch tabs, Menu opens the Guide menu, View acts on the newest
   notification. In this preview the keyboard stands in for a controller (arrows,
   Enter = A, Esc = B, X, Y, Q = LB, E = RB, M = Menu, V = View), and a real
   controller works too through the browser's Gamepad API. Placeholder titles only. ==== */
(function () {
var $ = function (id) { return document.getElementById(id); };
var esc = function (s) { return String(s).replace(/[&<>"]/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]; }); };

/* ------------------------------------------------------------------ data */
var PLAT = { PS: 'PlayStation', PS2: 'PlayStation 2', N64: 'Nintendo 64', SAT: 'Sega Saturn', GC: 'GameCube', SNES: 'Super Nintendo', WIN: 'Windows', LINUX: 'Linux' };
var SUM = 'A courier crosses a drowned continent on a hovercycle, racing storms between lighthouse towns. Each run reshapes the coast, and every town remembers what you carried.\nTwelve circuits, a split-screen mode for two players and a soundtrack recorded live.';
var G = [
  [1, 'Starfall Circuit', 'hsl(258 45% 38%)', 'PS', { inst: 1, played: 0, ach: 42, rel: 400 }],
  [2, 'Lantern Vale', 'hsl(32 70% 42%)', 'N64', { inst: 1, played: 2, rel: 900 }],
  [3, 'Ironclad Drift', 'hsl(205 55% 34%)', 'SAT', { inst: 1, played: 5, upd: 1, rel: 700 }],
  [4, 'Moss & Marrow', 'hsl(140 35% 30%)', 'GC', { rel: 310, added: 1 }],
  [5, 'Pixel Harbor', 'hsl(350 55% 42%)', 'SNES', { inst: 1, played: 9, rel: 1200 }],
  [6, 'Cinder Peaks', 'hsl(15 60% 40%)', 'PS', { rel: 20, added: 2 }],
  [7, 'Echo Garden', 'hsl(175 45% 30%)', 'N64', { rel: 40, added: 3 }],
  [8, 'Velvet Static', 'hsl(290 35% 36%)', 'PS2', { inst: 1, missing: 1, rel: 800 }],
  [9, 'Tidewalker', 'hsl(195 60% 32%)', 'GC', { rel: 60, added: 4 }],
  [10, 'Brass Comet', 'hsl(45 65% 36%)', 'SAT', { inst: 1, unlinked: 1, rel: 1500 }],
  [11, 'Hollow Signal', 'hsl(220 30% 28%)', 'PS', { rel: 500, added: 9 }],
  [12, 'Kite Season', 'hsl(95 40% 34%)', 'SNES', { rel: 90, added: 6 }],
  [13, 'Neon Orchard', 'hsl(320 50% 40%)', 'PS2', { inst: 1, played: 14, rel: 1000 }],
  [14, 'Glass Monsoon', 'hsl(185 35% 38%)', 'N64', { rel: 650, added: 12 }],
  [15, 'Quiet Engine', 'hsl(0 0% 30%)', 'SAT', { rel: 120, added: 8 }],
  [16, 'Ember Choir', 'hsl(8 65% 38%)', 'GC', { inst: 1, played: 30, upd: 1, rel: 1100 }],
  [17, 'Night Ferry', 'hsl(250 40% 30%)', 'LINUX', { inst: 1, played: 1, native: 1, rel: 5 }],
  [18, 'Copperline', 'hsl(25 55% 36%)', 'WIN', { native: 1, rel: 9, added: 5 }],
  [19, 'Static Bloom', 'hsl(315 45% 38%)', 'WIN', { inst: 1, native: 1, played: 20, rel: 38 }],
  [20, 'Wren Hollow', 'hsl(120 30% 30%)', 'LINUX', { native: 1, rel: 25, added: 7 }]
].map(function (r) { var g = { id: r[0], t: r[1], c: r[2], p: r[3] }; for (var k in r[4]) g[k] = r[4][k]; return g; });
/* A long summary, so the prototype shows how a description longer than the tab reads. */
G[0].sum = SUM + '\nThe coast changes with every storm season. Towns flood and rise again on stilts, ferries reroute, and the lighthouse keepers trade rumours about the routes that still hold. Your hovercycle can be rebuilt between runs from salvage you carry home, trading top speed for grip or cargo space.\nA story mode follows the courier Mara Quell across three seasons, with choices that decide which towns survive the last storm. Time trials, a daily coastline and ghost races against friends keep the circuits fresh after the credits.\nAccessibility options include a steering assist, a slower storm clock, colour-blind palettes for hazard markers and full controller remapping.';
var byId = function (id) { return G.filter(function (g) { return g.id === +id; })[0]; };

var SYS = [
  { id: 'SAT', name: 'Sega Saturn', emu: null, rec: 'Ymir', status: 'No emulator' },
  { id: 'GC', name: 'GameCube', emu: 'Dolphin', ver: '2412', status: 'Ready', fw: 'none' },
  { id: 'N64', name: 'Nintendo 64', emu: 'RetroArch', core: 'Mupen64Plus-Next', ver: '1.20.0', status: 'Ready', fw: 'none' },
  { id: 'PS', name: 'PlayStation', emu: 'DuckStation', ver: '0.1-8675', status: 'Ready', fw: 'installed' },
  { id: 'PS2', name: 'PlayStation 2', emu: 'PCSX2', ver: '2.3.1', upd: '2.3.4', status: 'Update available', fw: 'needed' },
  { id: 'SNES', name: 'Super Nintendo', emu: 'RetroArch', core: 'Snes9x', ver: '1.20.0', status: 'Ready', fw: 'none' },
  { id: 'WIN', name: 'Windows', emu: 'GE-Proton9-20', status: 'Ready', proton: 1 }
];
var EMUS = { SAT: [['Ymir', 'Download', 1], ['Mednafen', 'Download'], ['RetroArch · Beetle Saturn', 'Download']], GC: [['Dolphin', 'Installed', 1]], N64: [['RetroArch', 'Installed'], ['Rosalie\'s Mupen GUI', 'Download', 1]], PS: [['DuckStation', 'Installed', 1], ['RetroArch · Beetle PSX', 'Installed']], PS2: [['PCSX2', 'Installed', 1]], SNES: [['RetroArch', 'Installed', 1], ['bsnes', 'Download']], WIN: [['GE-Proton9-20', 'Installed'], ['Proton 9.0 (Steam)', 'Installed'], ['System Wine', 'Installed'], ['GE-Proton (latest)', 'Download', 1]] };
var sysOf = function (p) { return SYS.filter(function (s) { return s.id === p; })[0]; };

var DL = [
  { id: 'd1', gid: 6, t: 'Cinder Peaks', p: 'PS', st: 'active', pct: 38, kind: '' },
  { id: 'd2', gid: 9, t: 'Tidewalker', p: 'GC', st: 'queued', pct: 0, kind: '' },
  { id: 'd3', gid: 3, t: 'Ironclad Drift', p: 'SAT', st: 'done', kind: 'Update', line: 'Updated · 1.4 GB' },
  { id: 'd4', gid: 12, t: 'Kite Season', p: 'SNES', st: 'failed', kind: '', line: 'Failed · The server returned 503 (try again later)' }
];

var CFG = { theme: 'dark', art: true, fade: 45, blur: 40, hw: true, restore: true, upload: true, skipNewer: true, delay: 10, retain: 10,
  raUser: 'sam_plays', raToken: '', tvStart: 'last', prompts: 'auto', url: 'https://romm.home.lan', user: 'sam', lib: '/home/sam/Games/GRID', exitBtn: 'menu' };

var S = { tab: 'home', pages: [], layers: [], offline: false, libRail: 'all', srvRail: 'PS', libQ: '', srvQ: '', libSort: 'Recently played', detTab: 'Overview', saveKind: 'Saves', notes: [], scheme: 'xbox', padName: '' };

/* ------------------------------------------------------------------ glyphs */
var GL = {
  xbox: { A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'LB', RB: 'RB', LT: 'LT', RT: 'RT', VIEW: '⧉', MENU: '≡', GUIDE: '⌂', LS: 'L3', RS: 'RS', DPAD: '✚' },
  ps: { A: '✕', B: '○', X: '□', Y: '△', LB: 'L1', RB: 'R1', LT: 'L2', RT: 'R2', VIEW: '⧉', MENU: '≡', GUIDE: '⌂', LS: 'L3', RS: 'RS', DPAD: '✚' }
};
function scheme() { return CFG.prompts === 'auto' ? S.scheme : CFG.prompts; }
function gl(b) { var t = GL[scheme()][b]; var round = t.length === 1; return '<span class="gl' + (round ? ' round' : '') + (scheme() === 'ps' && 'ABXY'.indexOf(b) >= 0 && b.length === 1 ? ' sh' : '') + '">' + t + '</span>'; }

/* ------------------------------------------------------------------ helpers */
function ago(d) { return d === 0 ? 'today' : d === 1 ? 'yesterday' : d < 14 ? d + ' days ago' : Math.round(d / 7) + ' weeks ago'; }
function relDate(d) { var x = new Date(2026, 10, 9 - d); return x.getDate() + ' ' + ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'][x.getMonth()] + ' ' + x.getFullYear(); }
function emuFor(g) { if (g.native) return g.p === 'WIN' ? 'GE-Proton9-20' : 'Native'; var s = sysOf(g.p); return s && s.emu ? s.emu + (s.core ? ' · ' + s.core : '') : null; }
function dlOf(g) { return DL.filter(function (d) { return d.gid === g.id && (d.st === 'active' || d.st === 'queued'); })[0]; }
function cover(g, o) {
  o = o || {};
  var b = '';
  if (g.missing) b += '<span class="tag miss">MISSING</span>'; else if (g.upd) b += '<span class="tag upd">UPDATE</span>';
  if (g.inst) b += '<i class="idot" aria-label="Installed"></i>';
  if (o.plat) b += '<span class="tag plat">' + g.p + '</span>';
  var d = dlOf(g); if (d) b += '<span class="tag q">' + (d.st === 'active' ? d.pct + '%' : 'Queued') + '</span>';
  return '<button class="cv" data-f data-k="' + (o.k || 'g') + g.id + '" data-a="details" data-id="' + g.id + '" data-x="quick" data-hero="' + g.id + '" style="--c:' + g.c + '">' +
    '<span class="cv-art">' + b + '<span class="ttl">' + esc(g.t) + '</span></span><span class="cv-l">' + esc(g.t) + '</span></button>';
}
function quickLabel(g) { if (!g) return null; if (dlOf(g)) return null; if (g.missing || g.unlinked) return null; return g.inst ? 'Play' : (S.offline ? null : 'Install'); }

/* ------------------------------------------------------------------ notifications */
function toast(t, o) {
  o = o || {};
  var n = { t: t, s: o.s || '', err: !!o.err, act: o.act, ms: o.ms || (o.err ? 15000 : 10000), segs: o.segs, id: Math.random() };
  S.notes.unshift({ t: t, s: n.s, err: n.err, at: new Date() }); if (S.notes.length > 20) S.notes.pop();
  /* At most three cards. The setup card is pinned to the top and is never the one dropped. */
  var box = $('toasts');
  while (box.children.length >= 3) { var drop = Array.prototype.filter.call(box.children, function (c) { return !c._pinned; }).pop(); if (!drop) break; drop.remove(); }
  var el = document.createElement('div'); el.className = 'toast' + (n.err ? ' err' : ''); el.setAttribute('role', n.err ? 'alert' : 'status');
  el.innerHTML = '<div class="tt">' + esc(t) + '</div>' + (n.s ? '<div class="ts">' + esc(n.s) + '</div>' : '') + (n.segs ? '<div class="segs">' + n.segs + '</div>' : '') +
    (n.act ? '<div class="ta">' + gl('VIEW') + ' ' + esc(n.act[0]) + '</div>' : '');
  el._act = n.act; el._pinned = !!o.pinned;
  var after = Array.prototype.filter.call(box.children, function (c) { return c._pinned; }).pop();
  box.insertBefore(el, after ? after.nextSibling : box.firstChild);
  /* The countdown bar and the timer start now, or, for a held card, when it is released. */
  el.release = function (ms) {
    el.insertAdjacentHTML('beforeend', '<i class="tbar" style="animation-duration:' + (ms || n.ms) + 'ms"></i>');
    setTimeout(function () { el.remove(); paintHints(); }, ms || n.ms);
  };
  if (!o.hold) el.release();
  paintHints();
  return el;
}
function newestAction() { var c = $('toasts').children; for (var i = 0; i < c.length; i++) if (c[i]._act) return c[i]; return null; }

/* ------------------------------------------------------------------ focus engine */
var cur = null, mouseMoved = false;
function scope() { return S.layers.length ? S.layers[S.layers.length - 1].el : $('stage'); }
function focusables(root) { return Array.prototype.filter.call((root || scope()).querySelectorAll('[data-f]'), function (e) { return e.offsetParent !== null || e.getClientRects().length; }); }
function setFocus(el, noScroll) {
  if (!el) return;
  var prev = cur;
  if (cur && cur !== el) cur.classList.remove('f');
  cur = el; el.classList.add('f');
  /* Up from a Details tab into the buttons: the tab scrolls back to its top. */
  var tbd = $('tbody'); if (tbd && prev && tbd.contains(prev) && el.closest('.det-r .btns')) tbd.scrollTo({ top: 0, behavior: glide() ? 'smooth' : 'auto' });
  var c = el.closest('[data-nav]'); if (c) c._last = el;
  if (!noScroll) reveal(el);
  var top = S.layers.length ? S.layers[S.layers.length - 1] : (S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab]);
  if (el.dataset.k) top._k = el.dataset.k;
  if (el.dataset.hero) hero(byId(el.dataset.hero));
  if (el.dataset.onfocus && ONF[el.dataset.onfocus]) ONF[el.dataset.onfocus](el);
  paintHints();
}
/* Scrolling. Every move scrolls by a rule, never by the browser's "nearest":
   - A row that scrolls sideways (Home shelves, Related, Media) keeps the focused
     item a full row-padding (2.4rem) inside its edges, so a grown, ringed cover is
     never cut at the left or right.
   - On Home, a shelf snaps its title to the top of the rows area whenever focus
     moves into it, so a row is always seen whole with its title, and the first
     shelf sits at the very top.
   - Anywhere else (grids, lists, panes, sheets), the focused item stays 1.2rem
     inside the scrolling area, which clears its focus ring and lift; an item taller
     than the area lines up with its top.
   Redraws keep each screen's scroll positions (see renderStage). */
var INSTANT = false;
function remPx() { return parseFloat(getComputedStyle(document.documentElement).fontSize); }
function glide() { return !INSTANT && !(window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches); }
function vScroller(el) {
  for (var v = el.parentElement; v && v !== document.body; v = v.parentElement) {
    var oy = getComputedStyle(v).overflowY;
    if ((oy === 'auto' || oy === 'scroll') && v.scrollHeight > v.clientHeight + 1 && !v.classList.contains('track') && !v.classList.contains('mtrack')) return v;
  }
  return null;
}
function reveal(el) {
  var rem = remPx(), how = glide() ? 'smooth' : 'auto';
  var ht = el.closest('.track, .mtrack');
  if (ht) {
    var tr = ht.getBoundingClientRect(), r = el.getBoundingClientRect(), edge = parseFloat(getComputedStyle(ht).paddingLeft) || 2.4 * rem, x = ht.scrollLeft;
    if (r.left - tr.left < edge) x += (r.left - tr.left) - edge;
    else if (tr.right - r.right < edge) x += (r.right - tr.right) + edge;
    x = Math.max(0, Math.min(x, ht.scrollWidth - ht.clientWidth));
    if (Math.abs(x - ht.scrollLeft) > 1) ht.scrollTo({ left: x, behavior: how });
  }
  var v = vScroller(el); if (!v) return;
  var vr = v.getBoundingClientRect(), y = v.scrollTop, shelf = v.classList.contains('rows') && el.closest('.shelf');
  if (shelf) y = shelf.previousElementSibling ? y + (shelf.getBoundingClientRect().top - vr.top) : 0;
  else {
    var b = el.getBoundingClientRect(), pad = 1.2 * rem, top = b.top - pad, bot = b.bottom + pad;
    if (top < vr.top) y += top - vr.top; else if (bot > vr.bottom) y += Math.min(bot - vr.bottom, top - vr.top);
    /* The first or last item scrolls all the way to that end, so headings above it
       and notes below it ("No Backup drives connected") are never left out of view. */
    var all = focusables(v); if (el === all[0]) y = 0; else if (el === all[all.length - 1]) y = v.scrollHeight;
  }
  y = Math.max(0, Math.min(y, v.scrollHeight - v.clientHeight));
  if (Math.abs(y - v.scrollTop) > 1) v.scrollTo({ top: y, behavior: how });
}
var SCROLLERS = '.rows, .track, .mtrack, .scroll, .tbody, .list, .pane, .page, .rail, .start-body';
function saveScroll() { return Array.prototype.map.call($('stage').querySelectorAll(SCROLLERS), function (e) { return [e.scrollLeft, e.scrollTop]; }); }
function loadScroll(saved) { if (!saved) return; var L = $('stage').querySelectorAll(SCROLLERS); if (L.length !== saved.length) return; for (var i = 0; i < L.length; i++) { L[i].scrollLeft = saved[i][0]; L[i].scrollTop = saved[i][1]; } }

function restoreFocus(obj) {
  var root = scope(), el = null;
  if (obj && obj._k) el = root.querySelector('[data-k="' + obj._k + '"]');
  if (!el) el = root.querySelector('[data-autofocus]');
  if (!el) el = focusables(root)[0];
  if (el && (el.disabled || el.classList.contains('off'))) { var all = focusables(root); el = all.filter(function (e) { return !e.disabled; })[0] || el; }
  if (el) setFocus(el, false); else { cur = null; paintHints(); }
}
function rc(e) { return e.getBoundingClientRect(); }
function move(dir) {
  var all = focusables(); if (!cur || all.indexOf(cur) < 0) { restoreFocus(); return; }
  /* An explicit link wins over geometry: data-up / data-down on the item or a container names the data-k to go to. */
  var link = cur.closest('[data-' + dir + ']'), to = link && scope().querySelector('[data-k="' + link.getAttribute('data-' + dir) + '"]');
  if (to && to !== cur) { setFocus(to); return; }
  var a = rc(cur), ax = a.left + a.width / 2, ay = a.top + a.height / 2;
  var cont = cur.closest('[data-nav]'), axis = cont && cont.dataset.nav;
  var along = (axis === 'h' && (dir === 'left' || dir === 'right')) || (axis === 'v' && (dir === 'up' || dir === 'down'));
  function pick(list) {
    var best = null, bs = 1e9;
    list.forEach(function (e) {
      if (e === cur) return; var b = rc(e), bx = b.left + b.width / 2, by = b.top + b.height / 2, p, s;
      if (dir === 'right') { if (bx <= ax + 2 || b.left < a.left + 2) return; p = Math.max(0, b.left - a.right); s = Math.abs(by - ay); }
      if (dir === 'left') { if (bx >= ax - 2 || b.right > a.right - 2) return; p = Math.max(0, a.left - b.right); s = Math.abs(by - ay); }
      if (dir === 'down') { if (by <= ay + 2 || b.top < a.top + 2) return; p = Math.max(0, b.top - a.bottom); s = Math.abs(bx - ax); }
      if (dir === 'up') { if (by >= ay - 2 || b.bottom > a.bottom - 2) return; p = Math.max(0, a.top - b.bottom); s = Math.abs(bx - ax); }
      var sc = p + s * 2.2; if (sc < bs) { bs = sc; best = e; }
    });
    return best;
  }
  var t;
  if (along) {
    t = pick(all.filter(function (e) { return cont.contains(e); }));
    /* At a list's end, carry on to what sits past it in the same column (the
       buttons under a list), never sideways into a neighbouring panel. */
    if (!t) {
      var cr = rc(cont), fwd = dir === 'down' || dir === 'right';
      var out = all.filter(function (e) { if (cont.contains(e)) return false; var b = rc(e);
        if (axis === 'v') return b.right > cr.left && b.left < cr.right && (fwd ? b.top >= cr.bottom - 2 : b.bottom <= cr.top + 2);
        return b.bottom > cr.top && b.top < cr.bottom && (fwd ? b.left >= cr.right - 2 : b.right <= cr.left + 2); });
      /* The nearest line past the list, then its first item in reading order. */
      out.sort(function (x, y) { var X = rc(x), Y = rc(y); var d = axis === 'v' ? (fwd ? X.top - Y.top : Y.bottom - X.bottom) : (fwd ? X.left - Y.left : Y.right - X.right); return Math.abs(d) > 8 ? d : (axis === 'v' ? X.left - Y.left : X.top - Y.top); });
      t = out[0];
      /* Back into a button group: the button you left it from, while it is still there. */
      var tg = t && t.closest('[data-nav]'); if (tg && tg._last && tg.contains(tg._last) && tg._last.offsetParent) t = tg._last;
    }
    if (!t) return;
  }
  else {
    if (cont && axis === 'grid') t = pick(all.filter(function (e) { return cont.contains(e); }));
    if (!t) t = pick(all);
    if (t) { var tc = t.closest('[data-nav]'), live = tc && tc._last && tc.contains(tc._last) && tc._last.offsetParent;
      if (tc && tc !== cont) { if (tc.dataset.enter === 'first') t = live ? tc._last : focusables(tc)[0]; else if (live && tc.dataset.nav !== 'grid') t = tc._last; } }
  }
  if (t) setFocus(t);
}

/* ------------------------------------------------------------------ chrome */
var TAB_ORDER = ['home', 'library', 'server', 'downloads', 'settings'];
var TAB_NAME = { home: 'Home', library: 'Library', server: 'Server', downloads: 'Downloads', settings: 'Settings' };
function paintTop() {
  var mid;
  if (S.startup) mid = '<div class="crumb"></div>';
  else if (S.pages.length) { var p = S.pages[S.pages.length - 1]; mid = '<div class="crumb">' + gl('B') + ' ' + TAB_NAME[S.tab] + ' › <b>' + esc(p.title || '') + '</b></div>'; }
  else mid = '<nav class="tabs">' + gl('LB') + TAB_ORDER.map(function (t) { return '<button class="tab' + (t === S.tab ? ' on' : '') + '" data-tab="' + t + '">' + TAB_NAME[t] + '</button>'; }).join('') + gl('RB') + '</nav>';
  var act = DL.filter(function (d) { return d.st === 'active'; })[0];
  var d = new Date(), clock = ('0' + d.getHours()).slice(-2) + ':' + ('0' + d.getMinutes()).slice(-2);
  $('top').innerHTML = '<div class="brand">' + gIcon('grid', 22) + 'GRID <small>TV</small></div>' + mid +
    '<div class="status">' + (act && !S.startup ? '<span class="dlmini">' + gIcon('download', 16) + esc(act.t) + ' <i><b style="width:' + act.pct + '%"></b></i> ' + act.pct + '%</span>' : '') +
    (S.startup ? '' : '<span><i class="dot' + (S.offline ? ' offline' : '') + '"></i>' + (S.offline ? 'Offline' : esc(CFG.user)) + '</span>') + '<span class="clock">' + clock + '</span></div>';
}
function paintHints() {
  var h = hintsNow(), left = '';
  if (S.offline && !S.startup) left = '<span class="offline-pill">Offline</span><span>Your installed games still play. Server features return when GRID reconnects.</span>';
  else if (S.padName) left = '<span>' + esc(S.padName) + '</span>';
  $('hints').innerHTML = '<div class="left">' + left + '</div>' + h.map(function (x) { return '<button class="hint" data-btn="' + x[0] + '">' + gl(x[0]) + esc(x[1]) + '</button>'; }).join('');
}
function hintsNow() {
  var top = S.layers[S.layers.length - 1];
  if (top && top.hints) return top.hints();
  if (top) return [['A', 'Select'], ['B', top.blocking ? '' : 'Back']].filter(function (x) { return x[1]; });
  var h = [], el = cur;
  var al = el && (el.dataset.al || (el.dataset.a === 'details' ? 'Details' : 'Select'));
  if (el && !el.disabled) h.push(['A', al]);
  if (el && el.dataset.x === 'quick') { var q = quickLabel(byId(el.dataset.id)); if (q) h.push(['X', q]); }
  else if (el && el.dataset.xl) h.push(['X', el.dataset.xl]);
  var scr = S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab];
  if (scr.hints) h = h.concat(scr.hints(el));
  if (S.pages.length && !S.startup) h.push(['B', 'Back']);
  if (!S.startup) h.push(['MENU', 'Menu']);
  return h;
}
var artIdx = 0;
function hero(g) {
  if (!g) return;
  var art = $('art'); art.innerHTML = '<i style="left:5%;top:0;width:55%;height:70%;background:' + g.c + '"></i><i style="right:0;bottom:0;width:50%;height:65%;background:' + g.c + ';filter:hue-rotate(40deg)"></i>';
  art.style.opacity = CFG.art ? 1 : 0; art.style.setProperty('--fade', CFG.fade / 100);
  var h = $('hero'); if (!h) return;
  var line = g.missing ? 'Files not found' : g.inst ? (g.played != null ? 'Played ' + ago(g.played) : 'Installed') + ' · ' + (emuFor(g) || 'No emulator') : (dlOf(g) ? (dlOf(g).st === 'active' ? 'Installing… ' + dlOf(g).pct + '%' : 'Queued') : 'Released ' + relDate(g.rel) + ' · Not installed');
  h.innerHTML = '<div class="hero-box"><div class="plat">' + PLAT[g.p] + '</div><div class="logo">' + esc(g.t) + '</div><div class="meta">' + line + '</div><p class="sum">' + esc(SUM.split('\n')[0]) + '</p></div>';
}

/* ------------------------------------------------------------------ screens */
var TABS = {};
TABS.home = {
  render: function () {
    var lib = G.filter(function (g) { return g.inst; });
    var cont = lib.filter(function (g) { return g.played != null; }).sort(function (a, b) { return a.played - b.played; });
    var pref = ['PS', 'PS2', 'N64', 'GC', 'SNES', 'WIN', 'LINUX'];
    var nr = G.filter(function (g) { return g.rel <= 180 && pref.indexOf(g.p) >= 0; }).sort(function (a, b) { return a.rel - b.rel; });
    var ra = G.filter(function (g) { return g.added && nr.indexOf(g) < 0; }).sort(function (a, b) { return a.added - b.added; });
    var up = lib.filter(function (g) { return g.upd; });
    function shelf(id, name, list, extra) { return '<section class="shelf"><h2>' + name + (extra ? '<span>' + extra + '</span>' : '') + '</h2>' + (list.length ? '<div class="track" data-nav="h">' + list.map(function (g) { return cover(g, { k: id, plat: true }); }).join('') + '</div>' : '<p class="empty">' + (S.offline ? 'Needs the server. It comes back when GRID reconnects.' : 'Nothing here yet.') + '</p>') + '</section>'; }
    return '<div class="home"><div class="hero" id="hero"></div><div class="rows">' +
      shelf('c', 'Continue playing', cont) +
      (S.offline ? shelf('n', 'New releases', []) : shelf('n', 'New releases', nr, 'the last six months, on your systems')) +
      (S.offline ? '' : shelf('r', 'Recently added', ra, 'to your server')) +
      (up.length ? shelf('u', 'Updates', up, up.length + ' games') : '') +
      shelf('l', 'Your library', lib, lib.length + ' games') + '</div></div>';
  },
  hints: function () { return [['Y', 'Search']]; },
  y: function () { searchAll(); }
};

/* A folder in the Library rail: a header row that A opens and closes (the chevron
   turns), with its rows indented under it. Closed, a folder holding the current
   filter names it ("Platforms · PlayStation") and carries the selected mark, so
   the choice never disappears. Open / closed is kept for the session. */
S.folds = { plat: true, drives: true };
function folder(id, label, n, holds, rows) {
  var open = S.folds[id];
  return '<button class="ri fold' + (!open && holds ? ' sel' : '') + '" data-f data-k="fold-' + id + '" data-a="fold" data-fold="' + id + '" data-al="' + (open ? 'Close' : 'Open') + '" aria-expanded="' + open + '">' +
    '<span class="chev' + (open ? ' open' : '') + '">' + gIcon('chevronRight', 16) + '</span>' + label + (!open && holds ? '<span class="holds">· ' + holds + '</span>' : '') + '</button>' +
    (open ? rows : '');
}
function railItem(key, label, n, on, nav) { return '<button class="ri' + (on ? ' sel' : '') + '" data-f data-k="r-' + key + '" data-onfocus="' + nav + '" data-key="' + key + '" data-a="railOpen" data-al="Open">' + label + (n != null ? '<span class="n">' + n + '</span>' : '') + '</button>'; }
function libList() {
  var L = G.filter(function (g) { return g.inst; }), r = S.libRail;
  if (r === 'recent') L = L.filter(function (g) { return g.played != null && g.played <= 30; });
  else if (r === 'updates') L = L.filter(function (g) { return g.upd; });
  else if (r !== 'all') L = L.filter(function (g) { return g.p === r; });
  if (S.libQ) L = L.filter(function (g) { return g.t.toLowerCase().indexOf(S.libQ.toLowerCase()) >= 0; });
  var by = { 'Recently played': function (a, b) { return (a.played == null ? 999 : a.played) - (b.played == null ? 999 : b.played); }, 'Title': function (a, b) { return a.t < b.t ? -1 : 1; }, 'Platform': function (a, b) { return a.p < b.p ? -1 : a.p > b.p ? 1 : 0; }, 'Recently installed': function (a, b) { return b.id - a.id; } };
  return L.sort(by[S.libSort]);
}
var LIB_NAMES = { all: 'All games', recent: 'Recent', updates: 'Updates' };
TABS.library = {
  render: function () {
    var inst = G.filter(function (g) { return g.inst; }), plats = Object.keys(PLAT).filter(function (p) { return inst.some(function (g) { return g.p === p; }); });
    var rail = railItem('all', 'All games', inst.length, S.libRail === 'all', 'lib') + railItem('recent', 'Recent', inst.filter(function (g) { return g.played != null && g.played <= 30; }).length, S.libRail === 'recent', 'lib') +
      railItem('updates', 'Updates', inst.filter(function (g) { return g.upd; }).length, S.libRail === 'updates', 'lib') + '<hr>' +
      folder('plat', 'Platforms', plats.length, plats.indexOf(S.libRail) >= 0 ? PLAT[S.libRail] : '',
        plats.map(function (p) { return railItem(p, PLAT[p], inst.filter(function (g) { return g.p === p; }).length, S.libRail === p, 'lib').replace('class="ri', 'class="ri sub'); }).join('')) +
      folder('drives', 'Backup drives', 0, '', '<div class="ri-empty">No Backup drives connected</div>');
    return '<div class="split"><nav class="rail" data-nav="v">' + rail + '</nav><div class="main"><div class="vh" id="lib-h"></div><div class="scroll"><div class="grid" id="lib-grid" data-nav="grid" data-enter="first"></div></div></div></div>';
  },
  after: function () { libPaint(); },
  hints: function () { return [['Y', 'Search'], ['LT', 'Sort']]; },
  y: function () { openOSK({ title: 'Search your library', value: S.libQ, kind: 'search', onInput: function (v) { S.libQ = v; libPaint(); }, onDone: function (v) { S.libQ = v; libPaint(); } }); },
  lt: function () { sortSheet(); }
};
function sortSheet() { openSheet({ title: 'Sort by', items: ['Recently played', 'Recently installed', 'Title', 'Platform'].map(function (s) { return { label: s, checked: S.libSort === s, fn: function () { S.libSort = s; libPaint(); } }; }) }); }
function libPaint() {
  var L = libList(), name = LIB_NAMES[S.libRail] || PLAT[S.libRail];
  $('lib-h').innerHTML = '<h1>' + name + '</h1><span class="cnt">' + L.length + ' game' + (L.length === 1 ? '' : 's') + '</span>' + (S.libQ ? '<span class="chip">Search: <b>' + esc(S.libQ) + '</b></span>' : '') + '<span class="sp"></span><span class="chip">' + gl('LT') + ' Sort: <b>' + S.libSort + '</b></span><span class="chip">' + gl('Y') + ' Search</span>';
  $('lib-grid').innerHTML = L.length ? L.map(function (g) { return cover(g, { k: 'lg', plat: S.libRail === 'all' || S.libRail === 'recent' || S.libRail === 'updates' }); }).join('') :
    '<div class="empty">' + (S.libQ ? '<h2>No games match “' + esc(S.libQ) + '”</h2>Press ' + gl('Y') + ' to change the search.' : S.libRail === 'updates' ? '<h2>Everything is up to date</h2>Games with a newer copy on your server show here.' : '<h2>No games here yet</h2>Install games from the Server tab.') + '</div>';
}
var ONF = {
  lib: function (el) { if (S.libRail !== el.dataset.key) { S.libRail = el.dataset.key; S.libQ = ''; markRail(el); libPaint(); } },
  srv: function (el) { if (S.srvRail !== el.dataset.key) { S.srvRail = el.dataset.key; S.srvQ = ''; markRail(el); srvPaint(); } },
  set: function (el) { if (S.setPane !== el.dataset.key) { S.setPane = el.dataset.key; markRail(el); setPaint(); } }
};
function markRail(el) { el.parentNode.querySelectorAll('.ri').forEach(function (r) { r.classList.toggle('sel', r === el); }); }

TABS.server = {
  render: function () {
    if (S.offline) return '<div class="list"><div class="list-in"><div class="panel empty"><h2>Can\'t reach your server</h2>' + esc(CFG.url) + ' isn\'t answering. Your installed games still play from the Library.<div class="btns" style="margin-top:1.4rem"><button class="btn pri" data-f data-a="retry">Try again</button><button class="btn" data-f data-a="gotab" data-tab="library">Open Library</button></div></div></div></div>';
    var plats = Object.keys(PLAT);
    return '<div class="split"><nav class="rail" data-nav="v"><h3>PLATFORMS</h3>' + plats.map(function (p) { return railItem(p, PLAT[p], G.filter(function (g) { return g.p === p; }).length * 12, S.srvRail === p, 'srv'); }).join('') + '</nav><div class="main"><div class="vh" id="srv-h"></div><div class="scroll"><div class="grid" id="srv-grid" data-nav="grid" data-enter="first"></div></div></div></div>';
  },
  after: function () { if (!S.offline) srvPaint(); },
  hints: function () { return S.offline ? [] : [['Y', 'Search']]; },
  y: function () { if (S.offline) return; openOSK({ title: 'Search ' + PLAT[S.srvRail], value: S.srvQ, kind: 'search', onInput: function (v) { S.srvQ = v; srvPaint(); }, onDone: function (v) { S.srvQ = v; srvPaint(); } }); }
};
function srvPaint() {
  var p = S.srvRail, L = G.filter(function (g) { return g.p === p && (!S.srvQ || g.t.toLowerCase().indexOf(S.srvQ.toLowerCase()) >= 0); }), s = sysOf(p), nat = p === 'WIN' || p === 'LINUX';
  var chips = '';
  if (!nat && s) {
    var fw = s.fw === 'installed' ? 'Firmware: 3 files · installed' : s.fw === 'needed' ? 'Firmware: 2 files' : 'No server firmware';
    chips = '<button class="chip' + (s.emu ? '' : ' warn') + '" data-f data-k="ch-emu" data-a="system" data-id="' + p + '" data-al="Open in Systems">' + (s.emu ? 'Emulator: <b>' + s.emu + '</b>' : 'No default emulator') + '</button>' +
      (s.fw === 'none' ? '<span class="chip">' + fw + '</span>' : '<button class="chip" data-f data-k="ch-fw" data-a="system" data-id="' + p + '" data-al="Open in Systems">' + fw + '</button>');
  }
  $('srv-h').innerHTML = '<h1>' + PLAT[p] + '</h1><span class="cnt">' + L.length * 12 + ' games · ' + L.filter(function (g) { return g.inst; }).length + ' installed</span>' + chips + (S.srvQ ? '<span class="chip">Search: <b>' + esc(S.srvQ) + '</b></span>' : '') + '<span class="sp"></span><span class="chip">' + gl('Y') + ' Search</span>';
  $('srv-grid').innerHTML = L.length ? L.map(function (g) { return cover(g, { k: 'sg' }); }).join('') : '<div class="empty"><h2>No games match “' + esc(S.srvQ) + '”</h2></div>';
}

TABS.downloads = {
  render: function () {
    function row(d) {
      var line = d.st === 'active' ? 'Downloading · ' + d.pct + '% · 38.2 MB/s · 2 min left' : d.st === 'queued' ? 'Queued' : d.line;
      var spark = d.st === 'active' ? '<div class="spark"><svg viewBox="0 0 120 38" preserveAspectRatio="none"><polyline fill="none" stroke="var(--primary)" stroke-width="2" points="0,30 10,22 20,25 30,14 40,18 50,10 60,16 70,9 80,12 90,8 100,13 110,7 120,10"/><polyline fill="none" stroke="var(--graph-disk)" stroke-width="1.5" points="0,34 15,30 30,28 45,26 60,27 75,22 90,24 105,20 120,22"/></svg></div>' : '<div></div>';
      var al = d.st === 'active' || d.st === 'queued' ? 'Cancel' : d.st === 'failed' ? 'Retry or dismiss' : 'Dismiss';
      return '<div class="lrow dl" data-f data-k="dl-' + d.id + '" data-a="dlrow" data-id="' + d.id + '" data-al="' + al + '"><div><div class="t">' + esc(d.t) + (d.kind ? '<span class="kind">' + d.kind + '</span>' : '') + '</div><div class="s' + (d.st === 'failed' ? ' err' : '') + '">' + PLAT[d.p] + ' · ' + line + '</div>' + (d.st === 'active' ? '<div class="pbar"><b style="width:' + d.pct + '%"></b></div>' : '') + '</div>' + spark + '<div class="s">' + (d.st === 'active' ? '38.2 MB/s' : '') + '</div></div>';
    }
    function seg(name, st) { var L = DL.filter(function (d) { return st.indexOf(d.st) >= 0; }); return '<section class="panel"><h2>' + name + '<span>' + L.length + '</span></h2><div data-nav="v">' + (L.length ? L.map(row).join('') : '<p class="note">Nothing ' + name.toLowerCase() + '.</p>') + '</div></section>'; }
    return '<div class="list"><div class="list-in">' + seg('Active', ['active']) + seg('Queued', ['queued']) + seg('Completed', ['done', 'failed', 'cancelled']) + '</div></div>';
  },
  hints: function () { return DL.some(function (d) { return d.st === 'done' || d.st === 'failed' || d.st === 'cancelled'; }) ? [['Y', 'Clear completed']] : []; },
  y: function () { DL = DL.filter(function (d) { return d.st === 'active' || d.st === 'queued'; }); render(); }
};

/* ---- Settings */
var PANES = [['appearance', 'Appearance'], ['cloud', 'Cloud saves'], ['connection', 'Connection'], ['library', 'Library'], ['ra', 'RetroAchievements'], ['systems', 'Systems'], ['tv', 'TV mode'], ['updates', 'Updates']];
S.setPane = 'appearance';
TABS.settings = {
  render: function () { return '<div class="split"><nav class="rail" data-nav="v">' + PANES.map(function (p) { return railItem(p[0], p[1], null, S.setPane === p[0], 'set'); }).join('') + '</nav><div class="pane" id="pane" data-nav="v" data-enter="first"></div></div>'; },
  after: function () { setPaint(); },
  hints: function (el) { return el && el.dataset.adj ? [['DPAD', 'Adjust']] : []; }
};
function paneRows(id) {
  var on = function (b) { return b; };
  if (id === 'appearance') return [
    { type: 'choice', key: 'theme', label: 'Theme', opts: [['system', 'Follow system'], ['dark', 'Dark'], ['light', 'Light']] },
    { type: 'toggle', key: 'art', label: 'Background art', sub: 'The focused game\'s art, blurred, behind everything' },
    { type: 'slider', key: 'fade', label: 'Background fade', min: 0, max: 100, step: 5, fmt: function (v) { return v === 100 ? '100% · off' : v + '%'; } },
    { type: 'slider', key: 'blur', label: 'Background blur', min: 0, max: 100, step: 10, fmt: function (v) { return v + '%'; } },
    { type: 'toggle', key: 'hw', label: 'Hardware acceleration', sub: 'Renders on the graphics card. Applies after a restart.' },
    { type: 'info', label: 'Card size, game labels and card display', value: 'Desktop grids only' }];
  if (id === 'cloud') return [
    { type: 'toggle', key: 'restore', label: 'Restore before launch', sub: 'Fetch the latest cloud save before a game starts' },
    { type: 'toggle', key: 'upload', label: 'Upload after exit', sub: 'Send new saves when a game closes' },
    { type: 'toggle', key: 'skipNewer', label: 'Skip when the local save is newer' },
    { type: 'slider', key: 'delay', label: 'Upload delay', min: 0, max: 60, step: 5, fmt: function (v) { return v + ' s'; } },
    { type: 'slider', key: 'retain', label: 'Saves to keep per game', min: 0, max: 50, step: 1, fmt: function (v) { return v === 0 ? 'Unlimited' : v; } }];
  if (id === 'connection') return [
    { type: 'info', label: 'Server', value: CFG.url }, { type: 'info', label: 'User', value: CFG.user },
    { type: 'info', label: 'Credential', value: S.offline ? 'Stored in the system keyring · not verified' : 'Stored in the system keyring · verified' },
    { type: 'action', label: 'Reconnect', fn: reconnect },
    { type: 'action', label: 'Disconnect', sub: 'Returns to Connect. Your library and settings stay.', danger: 1, fn: function () { confirmBox({ title: 'Disconnect from ' + CFG.url + '?', body: 'GRID returns to Connect. Installed games, saves and settings stay on this computer.', ok: 'Disconnect', danger: 1, fn: function () { startConnect(); } }); } }];
  if (id === 'library') return [
    { type: 'info', label: 'Library folder', value: CFG.lib },
    { type: 'action', label: 'Change library folder…', sub: 'Move your games, emulators and saves, or start fresh', fn: changeLibrary }];
  if (id === 'ra') return [
    { type: 'text', key: 'raUser', label: 'Username' },
    { type: 'text', key: 'raToken', label: 'Token', mask: 1, sub: 'Write-only: GRID never shows it again' },
    { type: 'action', label: 'Save', sub: 'Sets the login in every emulator that supports it', fn: function () { toast('RetroAchievements login saved', { s: 'Sent to 3 emulators and your server account' }); } },
    { type: 'action', label: 'Clear', danger: 1, fn: function () { CFG.raToken = ''; setPaint(); toast('RetroAchievements login cleared'); } }];
  if (id === 'tv') return [
    { type: 'choice', key: 'tvStart', label: 'Open GRID in', opts: [['last', 'The mode it was closed in'], ['tv', 'TV mode'], ['desktop', 'Desktop mode']] },
    { type: 'choice', key: 'prompts', label: 'Button prompts', opts: [['auto', 'Match my controller'], ['xbox', 'A B X Y'], ['ps', '✕ ○ □ △']] },
    { type: 'info', label: 'Controller', value: S.padName || 'None found · the keyboard works too' },
    { type: 'action', label: 'Switch to desktop mode', sub: 'Also from the Menu, or F11 / the TV button on the desktop', fn: toDesktop },
    { type: 'action', label: 'Quit GRID', fn: quitBox }];
  if (id === 'updates') return [
    { type: 'info', label: 'Version', value: 'GRID 0.9.12' }, { type: 'info', label: 'Last checked', value: 'Today at 09:14' },
    { type: 'action', label: 'GRID 0.9.13 is out', sub: 'GRID only checks. Update it the way you installed it.', fn: function () { openSheet({ title: 'GRID 0.9.13', sub: 'github.com/…/releases/tag/v0.9.13', items: [{ label: 'Open release in desktop mode', fn: toDesktop }, { label: 'Dismiss this notice', fn: function () { toast('Update notice dismissed'); } }] }); } },
    { type: 'toggle', key: 'debug', label: 'Debug logging' }];
  return [];
}
function setPaint() {
  var id = S.setPane, pane = $('pane'); if (!pane) return;
  if (id === 'systems') { pane.innerHTML = systemsPane(); return; }
  var rows = paneRows(id);
  pane.innerHTML = '<div class="pane-h"><h1>' + PANES.filter(function (p) { return p[0] === id; })[0][1] + '</h1></div>' + rows.map(function (r, i) { return srow(r, id + i); }).join('');
  pane._rows = rows;
}
function srow(r, k) {
  var v = '';
  if (r.type === 'toggle') v = '<span class="sw' + (CFG[r.key] ? ' on' : '') + '"></span>';
  if (r.type === 'choice') v = r.opts.filter(function (o) { return o[0] === CFG[r.key]; })[0][1] + ' <span class="chev">' + gIcon('chevronRight', 18) + '</span>';
  if (r.type === 'slider') { var pc = (CFG[r.key] - r.min) / (r.max - r.min) * 100; v = '<span class="adj-arrows">◀</span><span class="sl"><b style="width:' + pc + '%"></b><i style="left:' + pc + '%"></i></span><span style="min-width:6rem">' + r.fmt(CFG[r.key]) + '</span><span class="adj-arrows">▶</span>'; }
  if (r.type === 'text') v = (CFG[r.key] ? (r.mask ? '••••••••' : esc(CFG[r.key])) : '<i>Not set</i>') + ' <span class="chev">' + gIcon('chevronRight', 18) + '</span>';
  if (r.type === 'info') v = esc(r.value);
  if (r.type === 'action') v = '<span class="chev">' + gIcon('chevronRight', 18) + '</span>';
  var f = r.type === 'info' ? '' : ' data-f data-a="srow" data-al="' + (r.type === 'toggle' ? 'Switch' : r.type === 'slider' ? 'Select' : r.type === 'text' ? 'Edit' : 'Select') + '"';
  return '<div class="srow ' + (r.type === 'action' ? 'act' : '') + (r.danger ? ' dng' : '') + (r.type === 'info' ? ' info' : '') + '"' + f + ' data-k="s-' + k + '"' + (r.type === 'slider' ? ' data-adj="1"' : '') + ' data-r="' + r.key + '"><div class="lab"><div class="t">' + esc(r.label) + '</div>' + (r.sub ? '<div class="s">' + esc(r.sub) + '</div>' : '') + '</div><div class="val">' + v + '</div></div>';
}
function rowOf(el) { var pane = el.closest('#pane, .page-in'); var rows = pane._rows; var idx = Array.prototype.indexOf.call(pane.querySelectorAll('.srow'), el); return rows[idx]; }
function srowAct(el) {
  var r = rowOf(el); if (!r) return;
  if (r.type === 'toggle') { CFG[r.key] = !CFG[r.key]; repaintKeep(); if (r.key === 'art') hero(byId(1)); }
  else if (r.type === 'choice') openSheet({ title: r.label, items: r.opts.map(function (o) { return { label: o[1], checked: CFG[r.key] === o[0], fn: function () { CFG[r.key] = o[0]; if (r.key === 'theme') applyTheme(); repaintKeep(); } }; }) });
  else if (r.type === 'text') openOSK({ title: r.label, value: r.mask ? '' : CFG[r.key], mask: r.mask, onDone: function (v) { CFG[r.key] = v; repaintKeep(); } });
  else if (r.type === 'action') r.fn(el);
}
function srowAdj(el, d) { var r = rowOf(el); if (!r || r.type !== 'slider') return false; CFG[r.key] = Math.max(r.min, Math.min(r.max, CFG[r.key] + d * r.step)); if (r.key === 'fade') $('art').style.setProperty('--fade', CFG.fade / 100); repaintKeep(); return true; }
function repaintKeep() { repaint(); }
function applyTheme() { var t = CFG.theme === 'system' ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark') : CFG.theme; document.documentElement.setAttribute('data-theme', t); }

function systemsPane() {
  var miss = SYS.filter(function (s) { return !s.emu; }), todo = miss.filter(function (s) { return s.status !== 'Installing…'; });
  var sorted = miss.concat(SYS.filter(function (s) { return s.emu && !s.proton; })).concat(SYS.filter(function (s) { return s.proton; }));
  return '<div class="pane-h"><h1>Systems</h1>' + (todo.length ? '<button class="btn pri" data-f data-k="sy-rec" data-a="installRec">Install recommended (' + todo.length + ')</button>' : miss.length ? '<button class="btn pri" data-f data-k="sy-rec" disabled>Installing ' + miss.length + ' emulator' + (miss.length === 1 ? '' : 's') + '…</button>' : '') + '<button class="btn" data-f data-k="sy-chk" data-a="checkUpd">Check for updates</button><button class="btn" data-f data-k="sy-add" data-a="addEmu">Add manually</button></div>' +
    sorted.map(function (s) {
      var st = s.status === 'Ready' ? '<span class="st ok">Ready</span>' : '<span class="st warn">' + s.status + '</span>';
      var sub = s.emu ? s.emu + (s.core ? ' · ' + s.core : '') : 'Games for this system can\'t start yet. ' + s.rec + ' is recommended.';
      return '<div class="srow" data-f data-k="sy-' + s.id + '" data-a="system" data-id="' + s.id + '" data-al="Open" data-xl="' + (s.emu ? 'Change emulator' : 'Install ' + s.rec) + '" data-xa="' + (s.emu ? 'pickEmu' : 'installEmu') + '"><div class="lab"><div class="t">' + s.name + '</div><div class="s">' + sub + '</div></div><div class="val">' + st + '<span class="chev">' + gIcon('chevronRight', 18) + '</span></div></div>';
    }).join('') + '<div class="sgroup">Other emulators</div><p class="note" style="margin-left:1.2rem">Every installed emulator is used by a system.</p>';
}

/* ---- Pages (pushed over a tab; B goes back) */
function push(p) { S.pages.push(p); render(); }
function pop() { var p = S.pages.pop(); if (p && p.onClose) p.onClose(); render(); }

function details(g) {
  return {
    title: g.t, id: g.id,
    render: function () {
      var s = sysOf(g.p), d = dlOf(g), btn = [];
      var B = function (lab, a, pri, extra) { btn.push('<button class="btn' + (pri ? ' pri' : '') + (/Uninstall|Remove/.test(lab) ? ' dng' : '') + '" data-f data-k="db-' + a + '" data-a="' + a + '" data-id="' + g.id + '"' + (extra || '') + '>' + lab + '</button>'); };
      if (g.unlinked) { B('Link to server game', 'link', 1); B('Remove from library (keeps files)', 'removeRow'); }
      else if (g.missing) { B('Locate…', 'locate', 1); B('Remove from library (keeps files)', 'removeRow'); }
      else if (d) B(d.st === 'active' ? 'Cancel install · ' + d.pct + '%' : 'Cancel · queued', 'cancelDl', 1);
      else if (g.inst) B(gIcon('play', 18) + 'Play', 'play', 1);
      else B(gIcon('download', 18) + (g.native ? 'Install app' : 'Install'), 'install', 1, S.offline ? ' disabled' : '');
      if (g.inst && g.upd && !g.missing) B('Update', 'update');
      if (g.inst && g.native && !g.missing) B('Game settings', 'gameSettings');
      if (g.inst && !g.missing && !g.unlinked) B('Uninstall', 'uninstall');
      var tabs = ['Overview', 'Media', 'Saves'].concat(g.ach ? ['Achievements'] : []);
      if (tabs.indexOf(S.detTab) < 0) S.detTab = 'Overview';
      var lp = g.missing ? '<b>Files not found.</b> Play and Uninstall are unavailable until the files are found.' : g.unlinked ? '<b>Not linked to a server game.</b> Play, cloud saves and updates need the link.' : g.inst ? 'Last played <b>' + (g.played != null ? ago(g.played) : 'never') + '</b><br>Launches with <b>' + (emuFor(g) || 'no emulator') + '</b>' : (!g.native && s && !s.emu ? '<b>No emulator for ' + s.name + '.</b> Install installs ' + s.rec + ' too.' : 'Not installed · 1.2 GB');
      return '<div class="det"><div class="det-l">' + cover(g, { k: 'dc' }).replace('data-f ', '').replace('data-a="details"', '') + '<div class="lp">' + lp + '</div></div>' +
        '<div class="det-r"><h1>' + esc(g.t) + '</h1><div class="hl">' + PLAT[g.p] + ' · ' + (2026 - Math.floor(g.rel / 365)) + ' · Lighthouse Works · Racing, Adventure · 8.4</div>' +
        '<div class="chips">' + (g.inst ? '<span class="chip"><b>Installed</b></span>' : '') + (g.upd ? '<span class="chip warn">Update available</span>' : '') + '<span class="chip">Identified</span><span class="chip">USA · EN</span></div>' +
        '<div class="btns" data-nav="h">' + btn.join('') + '</div>' +
        '<div class="ttabs">' + gl('LB') + tabs.map(function (t) { return '<button class="ttab' + (t === S.detTab ? ' on' : '') + '" data-dtab="' + t + '">' + t + '</button>'; }).join('') + gl('RB') + '</div>' +
        '<div class="tbody" id="tbody">' + detBody(g) + '</div></div></div>';
    },
    after: function () { hero(g); },
    hints: function (el) { return el && el.classList.contains('prose') ? [['DPAD', 'Read']] : [['RS', 'Scroll']]; },
    lb: function () { detTabStep(g, -1); }, rb: function () { detTabStep(g, 1); }
  };
}
/* Each game's cloud saves and states, newest first; the prototype keeps them for
   the session so deleting and uploading show in the list. */
function savesOf(g) {
  S.saves = S.saves || {}; var key = g.id + ':' + S.saveKind;
  if (!S.saves[key]) S.saves[key] = [['2 hours ago', '128 KB', 1], ['Yesterday, 21:40', '128 KB', 2], ['3 days ago', '126 KB', 3], ['12 Oct 2026', '120 KB', 4]].map(function (r) { return { when: r[0], size: r[1], slot: r[2] }; });
  return S.saves[key];
}
function detTabStep(g, d) { var tabs = ['Overview', 'Media', 'Saves'].concat(g.ach ? ['Achievements'] : []); detShowTab(tabs[(tabs.indexOf(S.detTab) + d + tabs.length) % tabs.length]); }
/* Switching a Details tab (LB / RB, or a click) focuses the tab's first item, at
   the top of the tab; a tab with nothing to focus leaves focus where it was. */
function detShowTab(t) {
  S.detTab = t; renderStage();
  var tb = $('tbody'); if (!tb) return; tb.scrollTop = 0;
  /* Nothing to focus in the tab (Saves on an unlinked game): the first button. */
  var first = focusables(tb)[0] || focusables(document.querySelector('.det-r .btns'))[0];
  if (first) { INSTANT = true; setFocus(first); INSTANT = false; }
}
function detBody(g) {
  if (S.detTab === 'Overview') {
    var rel = G.filter(function (x) { return x.p === g.p && x.id !== g.id; }).slice(0, 6);
    return '<div class="ov"><div><div class="prose" data-f data-k="prose" data-al="Read" tabindex="-1">' + (g.sum || SUM).split('\n').map(function (p) { return '<p>' + esc(p) + '</p>'; }).join('') + '</div>' +
      (rel.length ? '<div class="sub-h">Related<span>' + rel.length + '</span></div><div class="track" data-nav="h" data-up="prose">' + rel.map(function (x) { return cover(x, { k: 'rel' }); }).join('') + '</div>' : '') + '</div>' +
      '<dl class="facts"><div><dt>Developer</dt><dd>Lighthouse Works</dd></div><div><dt>Release</dt><dd>' + relDate(g.rel) + '</dd></div><div><dt>Genres</dt><dd>Racing, Adventure</dd></div><div><dt>Players</dt><dd>1–2</dd></div><div><dt>Size</dt><dd>1.2 GB</dd></div></dl></div>';
  }
  if (S.detTab === 'Media') {
    var shots = [16 / 9, 4 / 3, 16 / 9, 8 / 7, 16 / 9].map(function (r, i) { return '<button class="mtile" data-f data-k="m' + i + '" data-a="viewer" data-i="' + i + '" data-al="View" style="--c:' + g.c + ';width:' + (8 * r) + 'rem;filter:hue-rotate(' + i * 25 + 'deg)"></button>'; }).join('');
    return '<div class="sub-h">Screenshots<span>5</span></div><div class="mtrack" data-nav="h">' + shots + '</div><div class="sub-h">Videos<span>1</span></div><div class="mtrack" data-nav="h"><button class="mtile" data-f data-k="mv" data-a="viewer" data-i="5" data-al="Play video" style="--c:' + g.c + ';width:14.2rem"><span class="play">' + gIcon('play', 36) + '</span></button></div>';
  }
  if (S.detTab === 'Saves') {
    if (g.unlinked || !g.inst && !g.played) return '<p class="note">' + (g.unlinked ? 'Cloud saves need the link to a server game.' : 'No cloud saves yet. They appear after you play.') + '</p>';
    var kinds = '<div class="seg" data-nav="h">' + ['Saves', 'States'].map(function (k) { return '<button data-f data-k="sk-' + k + '" data-a="saveKind" data-v="' + k + '" class="' + (S.saveKind === k ? 'on' : '') + '">' + k + '</button>'; }).join('') + '</div>';
    var list = savesOf(g);
    return '<div class="bar-h">' + kinds + '<h3 style="margin:0;color:var(--text-h);flex:1">' + (S.saveKind === 'Saves' ? 'Cloud Saves' : 'Cloud States') + '</h3><button class="btn" data-f data-k="up" data-a="uploadSave" data-id="' + g.id + '">' + gIcon('cloud', 18) + 'Upload latest ' + (S.saveKind === 'Saves' ? 'save' : 'state') + '</button></div><div data-nav="v">' +
      (list.length ? list.map(function (r, i) { return '<div class="lrow" data-f data-k="sv' + i + '" data-a="saveRow" data-i="' + i + '" data-al="Restore or delete"><div class="grow"><div class="t">' + (emuFor(g) || 'DuckStation') + (i === 0 ? ' <span class="latest">LATEST</span>' : '') + '</div><div class="s">' + r.when + ' · ' + r.size + (S.saveKind === 'States' ? ' · slot ' + r.slot : '') + '</div></div></div>'; }).join('') : '<p class="note">No cloud ' + (S.saveKind === 'Saves' ? 'saves' : 'states') + ' for this game. Upload the latest one, or play and GRID uploads it when the game closes.</p>') + '</div>';
  }
  if (S.detTab === 'Achievements') {
    var rows = []; for (var i = 0; i < 8; i++) rows.push('<div class="lrow" data-f data-k="a' + i + '" data-al="Select"><span class="badge' + (i > 2 ? ' locked' : '') + '" style="--c:' + g.c + '"></span><div class="grow"><div class="t">' + ['First Light', 'Storm Chaser', 'Lighthouse Keeper', 'No Brakes', 'Coastline', 'Perfect Run', 'Night Courier', 'Full Cargo'][i] + '</div><div class="s">' + (i > 2 ? 'Finish a circuit without touching the water' : 'Unlocked ' + ago(i * 3 + 1) + (i === 0 ? ' · Hardcore' : '')) + '</div></div><span class="muted">' + (i + 1) * 5 + ' pts</span></div>');
    return '<p class="note">3 of ' + g.ach + ' unlocked · 30 of 400 points · 1 hardcore</p><div data-nav="v">' + rows.join('') + '</div>';
  }
  return '';
}

function playing(g) {
  var t0 = Date.now();
  return {
    title: 'Playing', render: function () {
      return '<div class="playing"><div class="box"><div class="lbl">NOW PLAYING</div><div class="logo">' + esc(g.t) + '</div><div class="muted">' + PLAT[g.p] + ' · ' + emuFor(g) + ' · <span id="ses">0:00</span></div>' +
        '<p class="note" style="margin:1.2rem auto 0">The game has the controller while it runs. GRID listens again when it closes' + (CFG.upload ? ' and uploads your save' : '') + '.</p>' +
        '<div class="btns"><button class="btn" data-f data-a="resume" data-autofocus>Back to the game</button><button class="btn dng" data-f data-a="stop" data-id="' + g.id + '">Stop game</button></div></div></div>';
    },
    after: function () { hero(g); clearInterval(S.sesT); S.sesT = setInterval(function () { var e = $('ses'); if (!e) return clearInterval(S.sesT); var s = Math.floor((Date.now() - t0) / 1000); e.textContent = Math.floor(s / 60) + ':' + ('0' + s % 60).slice(-2); }, 1000); },
    onClose: function () { clearInterval(S.sesT); }
  };
}

function systemPage(s) {
  return {
    title: s.name, render: function () {
      var rows = [];
      if (s.proton) {
        rows.push({ type: 'action', label: 'Default Proton version', sub: 'Runs Windows games on Linux', val: s.emu, fn: function () { pickEmu(s); } });
        rows.push({ type: 'info', label: 'Proton launcher (umu-run)', value: 'GRID-managed · 1.2.6' });
        rows.push({ type: 'action', label: 'Reinstall umu-launcher', fn: function () { toast('Installing umu-launcher…'); } });
        rows.push({ type: 'info', label: 'First Windows game', value: 'Downloads the Steam Linux Runtime (about 200 MB)' });
      } else if (!s.emu) {
        rows.push({ type: 'action', label: 'Install ' + s.rec + ' (recommended)', fn: function () { installEmu(s); } });
        rows.push({ type: 'action', label: 'Choose another emulator', fn: function () { pickEmu(s); } });
      } else {
        rows.push({ type: 'action', label: 'Emulator', val: s.emu, sub: 'Installed and Download groups, like the desktop picker', fn: function () { pickEmu(s); } });
        rows.push({ type: 'info', label: 'Version', value: s.ver + (s.upd ? '' : ' · up to date') });
        if (s.upd) rows.push({ type: 'action', label: 'Update to ' + s.upd, fn: function () { toast('Updating ' + s.emu + '…'); s.ver = s.upd; s.upd = null; s.status = s.fw === 'needed' ? 'Firmware needed' : 'Ready'; renderStage(); } });
        if (s.core) rows.push({ type: 'action', label: 'RetroArch core', val: s.core, fn: function () { openSheet({ title: 'Core for ' + s.name, items: [s.core, 'Parallel-N64', 'Mupen64Plus'].map(function (c) { return { label: c, checked: c === s.core, fn: function () { s.core = c; renderStage(); } }; }) }); } });
        if (s.fw === 'needed') rows.push({ type: 'action', label: s.name + ' firmware · 2 files · from your server', sub: 'Each file is checked against the size and MD5 the server lists', val: 'Install', fn: function () { toast('Installing ' + s.name + ' firmware for ' + s.emu + '…'); setTimeout(function () { s.fw = 'installed'; s.status = s.upd ? 'Update available' : 'Ready'; toast(s.name + ' firmware installed for ' + s.emu); if (S.pages.length) renderStage(); }, 1800); } });
        if (s.fw === 'installed') rows.push({ type: 'info', label: s.name + ' firmware', value: 'Installed' });
        rows.push({ type: 'action', label: 'Open ' + s.emu, sub: 'Opens the emulator on its own to change its settings. Switches to desktop mode while it runs.', fn: function () { toast('Opening ' + s.emu + '…'); } });
        rows.push({ type: 'action', label: 'Emulator settings', sub: 'Program, launch options, cloud save folders', fn: function () { push(emuForm(s.emu)); } });
        rows.push({ type: 'action', label: 'Remove ' + s.emu, danger: 1, fn: function () { confirmBox({ title: 'Remove ' + s.emu + '?', body: 'Games and saves stay. ' + s.name + ' games can\'t start until another emulator is picked.', ok: 'Remove', danger: 1, fn: function () { toast(s.emu + ' removed'); s.emu = null; s.status = 'No emulator'; s.rec = s.rec || 'DuckStation'; pop(); } }); } });
        rows.push({ type: 'info', label: 'Setup tips', value: 'Map your controller once in ' + s.emu + '; GRID keeps it.' });
      }
      this._rows = rows;
      return '<div class="page"><div class="page-in" data-nav="v"><h1>' + s.name + '</h1><p class="sub">' + (s.emu ? s.emu + ' · ' : '') + s.status + '</p>' + rows.map(function (r, i) { var h = srow(r, 'sp' + i); return r.val ? h.replace('<div class="val">', '<div class="val">' + esc(r.val) + ' ') : h; }).join('') + '</div></div>';
    },
    after: function () { var pi = document.querySelector('.page-in'); pi._rows = this._rows; }
  };
}
function emuForm(name) {
  var F = { which: name || 'Something else', ename: name || '', prog: name ? '/home/sam/Games/GRID/emulators/' + name + '/' + name.toLowerCase() : '', args: '%rom%', strat: 'auto' };
  return {
    title: name ? name + ' settings' : 'Add an emulator', render: function () {
      var rows = [];
      if (!name) rows.push({ type: 'action', label: 'Which emulator is it?', val: F.which, fn: function () { openSheet({ title: 'Which emulator is it?', items: ['DuckStation', 'PCSX2', 'Dolphin', 'Something else'].map(function (n) { return { label: n, checked: F.which === n, fn: function () { F.which = n; if (n !== 'Something else') F.ename = n; renderStage(); } }; }) }); } });
      rows.push({ type: 'action', label: 'Name', val: F.ename || 'Not set', fn: function () { openOSK({ title: 'Name', value: F.ename, onDone: function (v) { F.ename = v; renderStage(); } }); } });
      rows.push({ type: 'action', label: 'Program', val: F.prog ? F.prog.split('/').pop() : 'Not set', sub: F.prog || 'Browse for the emulator\'s program', fn: function () { openBrowser({ title: 'Choose the program', start: '/home/sam/Applications', files: true, onPick: function (p) { F.prog = p; renderStage(); } }); } });
      rows.push({ type: 'action', label: 'Launch options', val: F.args, sub: '%rom% is replaced by the game file', fn: function () { openOSK({ title: 'Launch options', value: F.args, onDone: function (v) { F.args = v; renderStage(); } }); } });
      rows.push({ type: 'action', label: 'Cloud saves (advanced)', val: 'Save strategy: ' + F.strat, sub: 'Save and state folders, files not to upload', fn: function () { openSheet({ title: 'How does it store saves?', items: [['auto', 'Automatic'], ['single_file', 'One file per game'], ['folder', 'A folder per game']].map(function (o) { return { label: o[1], checked: F.strat === o[0], fn: function () { F.strat = o[0]; renderStage(); } }; }) }); } });
      rows.push({ type: 'action', label: name ? 'Save' : 'Add emulator', fn: function () { toast(name ? name + ' settings saved' : (F.ename || 'Emulator') + ' added'); pop(); } });
      this._rows = rows;
      return '<div class="page"><div class="page-in" data-nav="v"><h1>' + (name ? name + ' settings' : 'Add an emulator manually') + '</h1><p class="sub">' + (name ? 'The same sheet as the desktop\'s Settings…' : 'For an emulator GRID doesn\'t install. It never becomes a system\'s default by itself.') + '</p>' + rows.map(function (r, i) { var h = srow(r, 'ef' + i); return r.val ? h.replace('<div class="val">', '<div class="val">' + esc(r.val) + ' ') : h; }).join('') + '</div></div>';
    },
    after: function () { document.querySelector('.page-in')._rows = this._rows; }
  };
}
function gameSettings(g) {
  var F = { exe: g.p === 'WIN' ? 'StaticBloom.exe' : 'start.sh', args: '', fromJson: true };
  return {
    title: 'Game settings', render: function () {
      var rows = [
        { type: 'action', label: 'Launch file', val: F.exe, sub: F.fromJson ? 'Set by game.json' : 'Your choice', fn: function () { openSheet({ title: 'Launch file', items: (g.p === 'WIN' ? ['StaticBloom.exe', 'Launcher.exe', 'tools/config.bat'] : ['start.sh', 'bin/nightferry', 'tools/setup.sh']).map(function (x) { return { label: x, checked: x === F.exe, fn: function () { F.exe = x; F.fromJson = false; renderStage(); } }; }) }); } },
        { type: 'action', label: 'Launch parameters', val: F.args || 'None', sub: 'Quotes group words', fn: function () { openOSK({ title: 'Launch parameters', value: F.args, onDone: function (v) { F.args = v; renderStage(); } }); } },
        { type: 'action', label: 'Reload launch info from server', fn: function () { toast('Launch info reloaded', { s: 'game.json sets ' + F.exe }); } },
        { type: 'action', label: 'Save', fn: function () { toast('Game settings saved'); pop(); } }];
      this._rows = rows;
      return '<div class="page"><div class="page-in" data-nav="v"><h1>' + esc(g.t) + '</h1><p class="sub">Game settings</p>' + rows.map(function (r, i) { var h = srow(r, 'gs' + i); return r.val ? h.replace('<div class="val">', '<div class="val">' + esc(r.val) + ' ') : h; }).join('') + '</div></div>';
    },
    after: function () { document.querySelector('.page-in')._rows = this._rows; }
  };
}
function linkPage(g) {
  var q = '';
  return {
    title: 'Link to server game', render: function () {
      var L = G.filter(function (x) { return x.p === g.p && x.id !== g.id && !x.inst && (!q || x.t.toLowerCase().indexOf(q.toLowerCase()) >= 0); });
      return '<div class="page"><div class="page-in"><h1>Link ' + esc(g.t) + '</h1><p class="sub">Pick the ' + PLAT[g.p] + ' game on your server that this is. ' + (q ? 'Search: <b>' + esc(q) + '</b>' : '') + '</p><div data-nav="v">' +
        (L.length ? L.map(function (x) { return '<div class="lrow" data-f data-k="lk' + x.id + '" data-a="doLink" data-id="' + x.id + '" data-al="Link"><span class="badge" style="--c:' + x.c + '"></span><div class="grow"><div class="t">' + esc(x.t) + '</div><div class="s">' + PLAT[x.p] + ' · ' + (2026 - Math.floor(x.rel / 365)) + '</div></div></div>'; }).join('') : '<p class="note">No games match.</p>') + '</div></div></div>';
    },
    hints: function () { return [['Y', 'Search']]; },
    y: function () { var self = this; openOSK({ title: 'Search ' + PLAT[g.p], value: q, kind: 'search', onInput: function (v) { q = v; renderStage(); }, onDone: function (v) { q = v; renderStage(); } }); },
    gid: g.id
  };
}

/* ---- Folder browser: Locate…, Change library, Relocate, Program */
var FS = { '/': ['home', 'mnt', 'run/media'], '/home': ['sam'], '/home/sam': ['Applications', 'Downloads', 'Games'], '/home/sam/Games': ['GRID', 'Old'], '/home/sam/Games/GRID': ['emulators', 'games', 'saves'], '/home/sam/Games/GRID/games': ['PlayStation 2', 'Sega Saturn'], '/home/sam/Games/GRID/games/PlayStation 2': ['Velvet Static'], '/home/sam/Applications': ['bsnes.AppImage', 'mednafen'], '/mnt': ['storage'], '/mnt/storage': ['Games', 'Media'], '/mnt/storage/Games': ['GRID', 'Velvet Static'], '/run/media': ['USB-1TB'], '/run/media/USB-1TB': ['GRID'] };
function openBrowser(o) {
  var path = o.start;
  push({
    title: o.title, render: function () {
      var kids = FS[path] || [];
      return '<div class="page"><div class="page-in"><h1>' + esc(o.title) + '</h1><div class="path">' + esc(path) + '</div><div data-nav="v">' +
        (path !== '/' ? '<div class="lrow" data-f data-k="up" data-a="fsGo" data-p=".." data-al="Up">' + gIcon('arrowLeft', 20) + '<div class="grow t">Up one folder</div></div>' : '') +
        kids.map(function (k) { var p = (path === '/' ? '' : path) + '/' + k; var isFile = /\.|mednafen/.test(k) && !FS[p]; return '<div class="lrow" data-f data-k="fs-' + esc(k) + '" data-a="fsGo" data-p="' + esc(p) + '" data-file="' + (isFile ? 1 : '') + '" data-al="' + (isFile ? 'Choose' : 'Open') + '"><div class="grow t">' + esc(k) + '</div>' + (isFile ? '' : gIcon('chevronRight', 18)) + '</div>'; }).join('') + '</div></div></div>';
    },
    hints: function () { return o.files ? [] : [['Y', 'Use this folder']]; },
    y: function () { if (!o.files) { pop(); o.onPick(path); } },
    go: function (p, file) { if (file) { pop(); o.onPick(p); return; } path = p === '..' ? (path.split('/').slice(0, -1).join('/') || '/') : p; this._k = null; renderStage(); restoreFocus(); }
  });
}

/* ---- Startup: Connect and Choose your systems */
function startConnect(reason) {
  S.startup = true; S.pages = []; S.connReason = reason || '';
  push(connectPage());
}
function connectPage() {
  var F = { url: CFG.url, how: 'token', token: '', lib: CFG.lib };
  return {
    title: 'Connect', render: function () {
      var ready = F.url && F.token && F.lib;
      function field(k, lab, val, mask, ph) { return '<div class="field" data-f data-k="cf-' + k + '" data-a="cfield" data-key="' + k + '" data-al="Edit"><span class="lab">' + lab + '</span><span class="v' + (val ? '' : ' ph') + '">' + (val ? (mask ? '••••••••••••' : esc(val)) : ph) + '</span></div>'; }
      return '<div class="start"><div class="start-box" data-nav="v"><h1>Connect to your RomM server</h1><p class="sub">Everything here works with a controller. A keyboard works too.</p>' +
        (S.connReason ? '<p class="errline">' + esc(S.connReason) + '</p>' : '') +
        '<div class="start-body">' + field('url', 'Server', F.url, 0, 'https://…') +
        '<div class="field" data-f data-k="cf-how" data-a="cfield" data-key="how" data-al="Change"><span class="lab">Sign in with</span><span class="v">' + (F.how === 'token' ? 'API token' : 'Username and password') + '</span></div>' +
        (F.how === 'token' ? field('token', 'API token', F.token, 1, 'Paste or type your token') : field('user', 'Username', F.user || '', 0, 'Your RomM user') + field('token', 'Password', F.token, 1, 'Your password')) +
        field('lib', 'Library folder', F.lib, 0, '') + '</div>' +
        '<div class="btns start-foot"><button class="btn pri" data-f data-k="cf-go" data-a="connectGo"' + (ready ? '' : ' disabled') + '>Connect</button></div></div></div>';
    },
    F: F, hints: function () { return [['VIEW', 'Paste']]; }
  };
}
function choosePage() {
  var pick = {}; SYS.forEach(function (s) { pick[s.id] = { on: false, emu: (EMUS[s.id].filter(function (e) { return e[2]; })[0] || EMUS[s.id][0])[0] }; });
  return {
    title: 'Choose your systems', render: function () {
      var n = Object.keys(pick).filter(function (k) { return pick[k].on; }).length;
      return '<div class="start"><div class="start-box wide"><h1>Choose your systems</h1><p class="sub">Tick the systems you want to play. GRID installs the recommended emulator for each. ' + gl('X') + ' changes the emulator; ' + gl('MENU') + ' skips this step. You can change this later in Settings › Systems.</p><div class="start-body" data-nav="v">' +
        SYS.map(function (s) { var p = pick[s.id]; return '<div class="srow" data-f data-k="ch-' + s.id + '" data-a="chTick" data-id="' + s.id + '" data-al="' + (p.on ? 'Untick' : 'Tick') + '" data-xl="Change emulator" data-xa="chEmu"><span class="sw' + (p.on ? ' on' : '') + '"></span><div class="lab"><div class="t">' + s.name + '</div><div class="s">' + (s.id === 'WIN' ? 'Proton for Windows games' : (G.filter(function (g) { return g.p === s.id; }).length * 12) + ' games') + '</div></div><div class="val">' + esc(p.emu) + '</div></div>'; }).join('') +
        '</div><div class="btns start-foot" data-nav="h"><button class="btn pri" data-f data-k="ch-go" data-a="chGo"' + (n ? '' : ' disabled') + '>' + (n ? 'Install ' + n + ' emulator' + (n === 1 ? '' : 's') : 'Install emulators') + '</button><button class="btn" data-f data-k="ch-skip" data-a="chSkip">' + gl('MENU') + 'Skip for now</button></div></div></div>';
    },
    pick: pick,
    hints: function () { return [['MENU', 'Skip for now']]; },
    menu: function () { ACT.chSkip(); }
  };
}

/* ------------------------------------------------------------------ overlays */
function openLayer(o) {
  var el = document.createElement('div'); el.className = 'layer' + (o.cls ? ' ' + o.cls : ''); el.innerHTML = o.html;
  o.el = o.el || el; o.prev = cur; $('layers').appendChild(el); o.el = el; S.layers.push(o);
  el.addEventListener('click', function (e) { if (e.target === el && !o.blocking) closeLayer(); });
  restoreFocus(o);
  return o;
}
function closeLayer() {
  var o = S.layers.pop(); if (!o) return; o.el.remove();
  var k = o.prev && o.prev.dataset.k, el = o.prev && document.body.contains(o.prev) ? o.prev : k && scope().querySelector('[data-k="' + k + '"]');
  if (el) setFocus(el, true); else restoreFocus(S.pages[S.pages.length - 1] || TABS[S.tab]);
  if (o.onClose) o.onClose();
}
function openSheet(o) {
  var items = o.items;
  var html = '<div class="sheet" data-nav="v"><h2>' + esc(o.title) + '</h2>' + (o.sub ? '<p class="sub">' + esc(o.sub) + '</p>' : '') + items.map(function (it, i) {
    if (it.group) return '<div class="sgroup">' + esc(it.group) + '</div>';
    return '<button class="opt' + (it.danger ? ' dng' : '') + '" data-f data-k="o' + i + '" data-opt="' + i + '"' + (it.checked ? ' data-autofocus' : '') + '>' + (o.items.some(function (x) { return 'checked' in x; }) ? '<span class="ck">' + (it.checked ? gIcon('check', 20) : '') + '</span>' : '') + esc(it.label) + (it.sub ? '<span class="s">' + esc(it.sub) + '</span>' : '') + '</button>';
  }).join('') + '</div>';
  var L = openLayer({ html: html, items: items });
  return L;
}
function confirmBox(o) {
  openLayer({ html: '<div class="dialog" role="alertdialog"><h2>' + esc(o.title) + '</h2><p>' + esc(o.body) + '</p><div class="btns" data-nav="h"><button class="btn" data-f data-autofocus data-cl="1">Cancel</button><button class="btn ' + (o.danger ? 'dng' : 'pri') + '" data-f data-ok="1">' + esc(o.ok) + '</button></div></div>', ok: o.fn });
}
function quitBox() { confirmBox({ title: 'Quit GRID?', body: 'Downloads in progress pause and resume the next time GRID starts.', ok: 'Quit', fn: function () { $('desk').hidden = false; $('desk').innerHTML = '<div class="box"><h1>GRID has quit</h1><p class="muted">Prototype: press A to start it again.</p></div>'; S.desk = true; } }); }
function toDesktop() { S.desk = true; $('desk').hidden = false; $('desk').innerHTML = '<div class="box"><h1>Desktop mode</h1><p class="muted">The desktop window opens here, at its last size, on the same view. Its top bar gains a TV button (and F11) to come back.</p><p class="muted">Prototype: press A or click to return to TV mode.</p></div>'; }

/* On-screen keyboard. X deletes, Y adds a space, LB / RB move the caret, LT is
   Shift, Menu is Done, B cancels. A physical keyboard types straight in. */
var KEYS = [['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'], ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'], ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', '-'], ['z', 'x', 'c', 'v', 'b', 'n', 'm', '.', '/', ':']];
function openOSK(o) {
  var K = { v: o.value || '', caret: (o.value || '').length, shift: false };
  var L;
  function draw() {
    var shown = o.mask ? K.v.replace(/./g, '•') : K.v;
    var html = '<div class="osk"><div class="osk-head"><div class="t">' + esc(o.title) + '</div><div class="osk-val">' + esc(shown.slice(0, K.caret)) + '<span class="caret"></span>' + esc(shown.slice(K.caret)) + '</div></div><div class="keys">' +
      KEYS.map(function (r, ri) { return '<div class="krow" data-nav="h">' + r.map(function (k) { var c = K.shift ? k.toUpperCase() : k; return '<button class="key" data-f data-k="k' + k + '" data-key="' + esc(c) + '">' + esc(c) + '</button>'; }).join('') + '</div>'; }).join('') +
      '<div class="krow" data-nav="h"><button class="key w2' + (K.shift ? ' on' : '') + '" data-f data-k="kshift" data-key="SHIFT">' + gl('LT') + 'Shift</button><button class="key w2" data-f data-k="ksym" data-key="@">@ _ #</button><button class="key w5" data-f data-k="kspace" data-key=" ">' + gl('Y') + 'Space</button><button class="key w2" data-f data-k="kback" data-key="BACK">' + gl('X') + '⌫</button><button class="key w2 pri" data-f data-k="kdone" data-key="DONE">' + gl('MENU') + 'Done</button></div></div></div>';
    if (!L) { L = openLayer({ html: html, cls: 'kb', osk: K, hints: function () { return [['A', 'Type'], ['X', 'Delete'], ['Y', 'Space'], ['LB', '◀'], ['RB', '▶'], ['MENU', 'Done'], ['B', 'Cancel']]; } }); L.draw = draw; L.o = o; }
    else { var k = cur && cur.dataset.k; L.el.innerHTML = html; var el = k && L.el.querySelector('[data-k="' + k + '"]'); setFocus(el || L.el.querySelector('[data-f]'), true); }
  }
  draw();
  L.type = function (ch) {
    if (ch === 'SHIFT') { K.shift = !K.shift; }
    else if (ch === 'BACK') { if (K.caret > 0) { K.v = K.v.slice(0, K.caret - 1) + K.v.slice(K.caret); K.caret--; } }
    else if (ch === 'DONE') { closeLayer(); if (o.onDone) o.onDone(K.v); return; }
    else if (ch === 'L') { K.caret = Math.max(0, K.caret - 1); }
    else if (ch === 'R') { K.caret = Math.min(K.v.length, K.caret + 1); }
    else if (ch === '@') { openSheet({ title: 'Symbols', items: ['@', '_', '#', '%', '?', '=', '&', '"', '!', '+'].map(function (c) { return { label: c, fn: function () { L.type(c); } }; }) }); return; }
    else { K.v = K.v.slice(0, K.caret) + ch + K.v.slice(K.caret); K.caret += ch.length; if (K.shift && ch.length === 1) K.shift = false; }
    draw(); if (o.onInput) o.onInput(K.v);
  };
}
function searchAll() {
  openOSK({ title: 'Search everything', kind: 'search', onDone: function (v) {
    var L = G.filter(function (g) { return g.t.toLowerCase().indexOf(v.toLowerCase()) >= 0; });
    push({ title: 'Search: ' + v, render: function () { return '<div class="list"><div class="list-in"><div class="panel"><h2>' + L.length + ' result' + (L.length === 1 ? '' : 's') + ' for “' + esc(v) + '”</h2><div class="grid" data-nav="grid">' + (L.map(function (g) { return cover(g, { k: 'sr', plat: true }); }).join('') || '<p class="note">Nothing matches. Try fewer letters.</p>') + '</div></div></div></div>'; } });
  } });
}

function guideMenu() {
  var act = DL.filter(function (d) { return d.st === 'active' || d.st === 'queued'; }).length;
  var d = new Date(), clock = ('0' + d.getHours()).slice(-2) + ':' + ('0' + d.getMinutes()).slice(-2);
  var items = [
    ['Home', function () { S.pages = []; S.tab = 'home'; render(); }],
    ['Downloads' + (act ? ' · ' + act + ' active' : ''), function () { S.pages = []; S.tab = 'downloads'; render(); }],
    ['Notifications · ' + S.notes.length, notesPage],
    ['Settings', function () { S.pages = []; S.tab = 'settings'; render(); }],
    S.offline ? ['Reconnect', reconnect] : ['Reconnect', reconnect],
    ['Switch to desktop mode', toDesktop],
    ['Quit GRID', quitBox]
  ];
  var proto = [
    ['Show: first run (Connect)', function () { startConnect(); }],
    ['Show: token rejected', function () { startConnect('The server rejected your token. Enter a new one.'); }],
    [S.offline ? 'Show: back online' : 'Show: server unreachable', function () { S.offline = !S.offline; if (!S.offline) toast('Connected to ' + CFG.url); else toast('Can\'t reach ' + CFG.url, { err: 1, s: 'Showing your library. GRID keeps trying.' }); render(); }],
    ['Show: unreachable library', unreachable],
    ['Show: a failed install', function () { toast('Couldn\'t install Kite Season: the server returned 503', { err: 1, act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] }); }]
  ];
  var html = '<aside class="guide" data-nav="v"><div class="gh"><b>' + clock + '</b><span class="muted">' + (S.offline ? 'Offline' : esc(CFG.user) + ' · ' + esc(CFG.url.replace('https://', ''))) + '</span></div>' +
    items.map(function (it, i) { return '<button class="opt" data-f data-k="gm' + i + '" data-gi="' + i + '">' + esc(it[0]) + '</button>'; }).join('') +
    '<hr><div class="proto">PROTOTYPE ONLY</div>' + proto.map(function (it, i) { return '<button class="opt" data-f data-k="gp' + i + '" data-gp="' + i + '">' + esc(it[0]) + '</button>'; }).join('') + '</aside>';
  openLayer({ html: html, cls: 'side', gi: items, gp: proto, hints: function () { return [['A', 'Select'], ['B', 'Close']]; } });
}
function notesPage() {
  S.pages = []; push({ title: 'Notifications', render: function () {
    return '<div class="list"><div class="list-in"><div class="panel"><h2>Notifications<span>' + S.notes.length + '</span></h2><div data-nav="v">' + (S.notes.length ? S.notes.map(function (n, i) { return '<div class="lrow" data-f data-k="n' + i + '"><div class="grow"><div class="t' + (n.err ? ' err' : '') + '">' + esc(n.t) + '</div><div class="s">' + (n.s ? esc(n.s) + ' · ' : '') + n.at.toTimeString().slice(0, 5) + '</div></div></div>'; }).join('') : '<p class="note">Nothing yet. Every notice GRID shows lands here too.</p>') + '</div></div></div></div>';
  }, hints: function () { return [['Y', 'Clear all']]; }, y: function () { S.notes = []; renderStage(); restoreFocus(); } });
}
function unreachable() {
  openLayer({ blocking: true, html: '<div class="dialog" role="alertdialog"><h2>Your library can\'t be reached</h2><p>/run/media/USB-1TB/GRID isn\'t there: its drive may not be connected. GRID can\'t run without it.</p><div class="btns" data-nav="h"><button class="btn pri" data-f data-autofocus data-relocate="1">Relocate the library…</button><button class="btn" data-f data-ok="1">Close without changing anything</button></div></div>', ok: function () { quitBox(); } });
}
function reconnect() { toast('Reconnecting to ' + CFG.url + '…'); setTimeout(function () { S.offline = false; toast('Connected to ' + CFG.url); render(); }, 900); }
function changeLibrary() {
  openBrowser({ title: 'Choose the new library folder', start: '/mnt/storage', onPick: function (p) {
    if (p === CFG.lib || p.indexOf(CFG.lib) === 0) { toast('Can\'t use that folder', { err: 1, s: 'It is the current library or inside it.' }); return; }
    openSheet({ title: 'Move to ' + p + '?', sub: 'games/ · copied and checked · emulators/ · copied and checked · saves/ · copied and checked. Copies 84 GB; 1.2 TB free.', items: [
      { label: 'Move existing files', sub: 'Runs in Downloads', fn: function () { DL.unshift({ id: 'mv', gid: 0, t: 'Library move', p: 'PS', st: 'active', pct: 2, kind: 'Library' }); toast('Moving the library to ' + p + '…', { s: 'Installs and launches wait until it finishes' }); } },
      { label: 'Start fresh', sub: 'Leave or delete the old files', fn: function () { openSheet({ title: 'Delete the old game files, or leave them on disk?', items: [{ label: 'Leave them on disk', fn: function () { CFG.lib = p; setPaint(); toast('Library is now ' + p); } }, { label: 'Delete them (84 GB)', danger: 1, fn: function () { confirmBox({ title: 'Delete 84 GB of game files?', body: 'games/ in ' + CFG.lib + ' is removed. Emulators and saves stay. This can\'t be undone.', ok: 'Delete', danger: 1, fn: function () { CFG.lib = p; toast('Library is now ' + p, { s: 'Old game files deleted' }); } }); } }] }); } }] });
  } });
}

/* ------------------------------------------------------------------ actions */
function install(g) {
  if (S.offline) { toast('Couldn\'t add ' + g.t + ' to the download queue: the server can\'t be reached', { err: 1 }); return; }
  var s = sysOf(g.p), active = DL.some(function (d) { return d.st === 'active'; });
  DL.push({ id: 'd' + Date.now(), gid: g.id, t: g.t, p: g.p, st: active ? 'queued' : 'active', pct: 0, kind: '' });
  toast(g.t + ' added to download queue', { s: !g.native && s && !s.emu ? s.rec + ' installs with it' : '', act: ['Open Downloads', function () { S.pages = []; S.tab = 'downloads'; render(); }] });
  repaint();
}
function play(g) {
  if (g.missing) { toast('Couldn\'t launch ' + g.t + ': files not found', { err: 1 }); return; }
  if (g.unlinked) { toast('Couldn\'t launch ' + g.t + ': not linked to a server game', { err: 1 }); return; }
  if (!emuFor(g)) { toast('Couldn\'t launch ' + g.t + ': no emulator for ' + PLAT[g.p], { err: 1, act: ['Set up ' + PLAT[g.p], function () { push(systemPage(sysOf(g.p))); }] }); return; }
  toast('Launching ' + g.t + '…', CFG.restore ? { s: 'Restored the latest cloud save' } : {});
  g.played = 0; push(playing(g));
}
var ACT = {
  details: function (el) { push(details(byId(el.dataset.id))); },
  quick: function (el) { var g = byId(el.dataset.id), q = quickLabel(g); if (q === 'Play') play(g); else if (q) install(g); },
  play: function (el) { play(byId(el.dataset.id)); },
  install: function (el) { install(byId(el.dataset.id)); },
  cancelDl: function (el) { var g = byId(el.dataset.id), d = dlOf(g); confirmBox({ title: 'Cancel ' + g.t + '?', body: 'The download stops and its files are removed. You can install it again later.', ok: 'Cancel install', danger: 1, fn: function () { d.st = 'cancelled'; d.line = 'Cancelled'; toast('Cancelled ' + g.t); advanceQueue(); repaint(); } }); },
  update: function (el) { var g = byId(el.dataset.id); confirmBox({ title: 'Update ' + g.t + '?', body: 'Saves and configuration are kept.', ok: 'Update', fn: function () { g.upd = 0; DL.push({ id: 'u' + Date.now(), gid: g.id, t: g.t, p: g.p, st: DL.some(function (d) { return d.st === 'active'; }) ? 'queued' : 'active', pct: 0, kind: 'Update' }); toast('Update for ' + g.t + ' added to download queue'); repaint(); } }); },
  uninstall: function (el) { var g = byId(el.dataset.id); confirmBox({ title: 'Uninstall ' + g.t + '?', body: 'Its files are deleted from this computer. Cloud saves stay on your server.', ok: 'Uninstall', danger: 1, fn: function () { g.inst = 0; g.upd = 0; toast(g.t + ' uninstalled'); repaint(); } }); },
  gameSettings: function (el) { push(gameSettings(byId(el.dataset.id))); },
  link: function (el) { push(linkPage(byId(el.dataset.id))); },
  doLink: function (el) { var p = S.pages[S.pages.length - 1], g = byId(p.gid), x = byId(el.dataset.id); g.unlinked = 0; toast(g.t + ' linked to ' + x.t); pop(); },
  removeRow: function (el) { var g = byId(el.dataset.id); confirmBox({ title: 'Remove ' + g.t + ' from the library?', body: 'Only the library entry goes. Its folder stays on disk: /home/sam/Games/GRID/games/' + PLAT[g.p] + '/' + g.t + '.', ok: 'Remove from library', danger: 1, fn: function () { g.inst = 0; g.missing = 0; g.unlinked = 0; toast(g.t + ' removed from the library'); pop(); } }); },
  locate: function (el) { var g = byId(el.dataset.id); openBrowser({ title: 'Where is ' + g.t + '?', start: '/mnt/storage/Games', onPick: function (p) { if (/Velvet Static|storage\/Games$/.test(p)) { g.missing = 0; toast(g.t + ' found', { s: 'Plays from ' + p }); renderStage(); } else toast('Couldn\'t find ' + g.t + ' in ' + p, { err: 1, s: 'Pick the folder that holds the game.' }); } }); },
  viewer: function (el) { viewer(+el.dataset.i); },
  saveKind: function (el) { S.saveKind = el.dataset.v; renderStage(); restoreFocus(); },
  uploadSave: function (el) {
    var g = byId(el.dataset.id), kind = S.saveKind, word = kind === 'Saves' ? 'save' : 'state';
    toast('Uploading the latest ' + word + '…');
    setTimeout(function () {
      var was = S.saveKind; S.saveKind = kind; var list = savesOf(g); S.saveKind = was;
      list.unshift({ when: 'Just now', size: '128 KB', slot: list.length + 1 });
      toast((kind === 'Saves' ? 'Save' : 'State') + ' uploaded for ' + g.t);
      var top = S.pages[S.pages.length - 1]; if (top && top.id === g.id && S.detTab === 'Saves' && S.saveKind === kind) repaint();
    }, 1200);
  },
  saveRow: function (el) { var g = byId(S.pages[S.pages.length - 1].id); openSheet({ title: (S.saveKind === 'Saves' ? 'Save' : 'State') + ' from ' + el.querySelector('.s').textContent.split(' · ')[0], items: [
    { label: 'Restore to this computer', fn: function () { confirmBox({ title: 'Restore this ' + (S.saveKind === 'Saves' ? 'save' : 'state') + '?', body: 'It replaces the ' + (S.saveKind === 'Saves' ? 'save' : 'state') + ' for ' + g.t + ' on this computer. The one there now is overwritten.', ok: 'Restore', fn: function () { toast((S.saveKind === 'Saves' ? 'Save' : 'State') + ' restored for ' + g.t); } }); } },
    { label: 'Delete from the server', danger: 1, fn: function () { confirmBox({ title: 'Delete this ' + (S.saveKind === 'Saves' ? 'save' : 'state') + '?', body: 'It is removed from your server. This can\'t be undone.', ok: 'Delete', danger: 1, fn: function () { savesOf(g).splice(+el.dataset.i, 1); toast((S.saveKind === 'Saves' ? 'Save' : 'State') + ' deleted from the server'); repaint(); } }); } }] }); },
  resume: function () { toast('Back to the game', { s: 'GRID moves behind it' }); },
  stop: function (el) { var g = byId(el.dataset.id); confirmBox({ title: 'Stop ' + g.t + '?', body: 'Unsaved progress in the game is lost.', ok: 'Stop game', danger: 1, fn: function () { pop(); if (CFG.upload) { toast('Uploading the save for ' + g.t + '…'); setTimeout(function () { toast('Save uploaded for ' + g.t); }, 1400); } } }); },
  dlrow: function (el) {
    var d = DL.filter(function (x) { return x.id === el.dataset.id; })[0], items = [];
    if (d.st === 'active' || d.st === 'queued') items.push({ label: 'Cancel', danger: 1, fn: function () { d.st = 'cancelled'; d.line = 'Cancelled'; toast('Cancelled ' + d.t); advanceQueue(); renderStage(); } });
    if (d.st === 'failed' || d.st === 'cancelled') items.push({ label: 'Retry', fn: function () { d.st = DL.some(function (x) { return x.st === 'active'; }) ? 'queued' : 'active'; d.pct = 0; toast(d.t + ' added to download queue'); renderStage(); } });
    if (d.st === 'done' || d.st === 'failed' || d.st === 'cancelled') items.push({ label: 'Dismiss', fn: function () { DL.splice(DL.indexOf(d), 1); renderStage(); restoreFocus(); } });
    if (d.gid) items.push({ label: 'Open ' + d.t, fn: function () { push(details(byId(d.gid))); } });
    openSheet({ title: d.t, sub: PLAT[d.p] + (d.kind ? ' · ' + d.kind : ''), items: items });
  },
  srow: srowAct,
  fold: function (el) { S.folds[el.dataset.fold] = !S.folds[el.dataset.fold]; repaint(); },
  /* A on a rail entry does what Right does: into the page beside it (the place you
     last left there, else its first control). */
  railOpen: function () {
    var box = $('pane') || $('lib-grid') || $('srv-grid'); if (!box) return;
    var L = focusables(box), el = box._last && box.contains(box._last) && box._last.offsetParent ? box._last : L[0];
    if (el) setFocus(el);
  },
  system: function (el) { push(systemPage(sysOf(el.dataset.id))); },
  installRec: function () { SYS.filter(function (s) { return !s.emu && s.status !== 'Installing…'; }).forEach(function (s) { installEmu(s); }); },
  installEmu: function (el) { installEmu(sysOf(el.dataset.id)); },
  pickEmu: function (el) { pickEmu(sysOf(el.dataset.id)); },
  checkUpd: function () { toast('Checking 5 emulators for updates…'); setTimeout(function () { toast('1 update available', { s: 'PCSX2 2.3.4' }); }, 1200); },
  addEmu: function () { push(emuForm(null)); },
  fsGo: function (el) { S.pages[S.pages.length - 1].go(el.dataset.p, !!el.dataset.file); },
  retry: function () { reconnect(); },
  gotab: function (el) { S.tab = el.dataset.tab; render(); },
  cfield: function (el) {
    var P = S.pages[S.pages.length - 1], F = P.F, k = el.dataset.key;
    if (k === 'how') { openSheet({ title: 'Sign in with', items: [{ label: 'API token', checked: F.how === 'token', fn: function () { F.how = 'token'; renderStage(); } }, { label: 'Username and password', checked: F.how !== 'token', fn: function () { F.how = 'pw'; renderStage(); } }] }); return; }
    if (k === 'lib') { openBrowser({ title: 'Library folder', start: '/home/sam/Games', onPick: function (p) { F.lib = p; renderStage(); } }); return; }
    openOSK({ title: { url: 'Server address', token: F.how === 'token' ? 'API token' : 'Password', user: 'Username' }[k], value: k === 'token' ? '' : F[k], mask: k === 'token', kind: k === 'url' ? 'url' : 'text', onDone: function (v) { F[k] = v; renderStage(); } });
  },
  connectGo: function () { var F = S.pages[S.pages.length - 1].F; CFG.url = F.url; CFG.lib = F.lib; toast('Connected to ' + F.url); S.pages = []; push(choosePage()); },
  chTick: function (el) { var P = S.pages[S.pages.length - 1]; P.pick[el.dataset.id].on = !P.pick[el.dataset.id].on; renderStage(); },
  chEmu: function (el) { var P = S.pages[S.pages.length - 1], id = el.dataset.id; openSheet({ title: 'Emulator for ' + sysOf(id).name, items: emuItems(id, P.pick[id].emu, function (e) { P.pick[id].emu = e; P.pick[id].on = true; renderStage(); }) }); },
  chGo: function () {
    var P = S.pages[S.pages.length - 1], names = [];
    Object.keys(P.pick).forEach(function (k) { var e = P.pick[k].emu; if (P.pick[k].on && e && names.indexOf(e) < 0) names.push(e); });
    S.startup = false; S.pages = []; S.tab = 'home'; render(); setupCard(names);
  },
  chSkip: function () { S.startup = false; S.pages = []; S.tab = 'home'; render(); }
};
function emuItems(id, curE, fn) {
  var items = [], groups = ['Installed', 'Download'];
  groups.forEach(function (gname) { var L = EMUS[id].filter(function (e) { return e[1] === gname; }); if (!L.length) return; items.push({ group: gname }); L.forEach(function (e) { items.push({ label: e[0] + (e[2] ? ' (recommended)' : ''), checked: e[0] === curE, fn: function () { fn(e[0], gname); } }); }); });
  items.push({ group: ' ' }); items.push({ label: 'None', checked: curE === null, fn: function () { fn(null); } });
  return items;
}
function pickEmu(s) { openSheet({ title: (s.proton ? 'Default Proton version' : 'Emulator for ' + s.name), items: emuItems(s.id, s.emu, function (e, gname) { if (gname === 'Download') { installEmu(s, e); return; } s.emu = e; s.status = e ? 'Ready' : 'No emulator'; if (!e) s.rec = (EMUS[s.id].filter(function (x) { return x[2]; })[0] || EMUS[s.id][0])[0]; renderStage(); }) }); }
function installEmu(s, name) {
  name = name || s.rec; s.status = 'Installing…'; repaint(); toast('Installing ' + name + '…');
  setTimeout(function () { s.emu = name; s.ver = '0.4.1'; s.status = 'Ready'; toast(name + ' installed', { s: 'Used for ' + s.name }); repaint(); }, 2200);
}
/* The first-run setup card: one segment per install (each emulator once). A
   segment's first half fills while that install downloads and its second half
   while it installs; a line under the bar names the current step. The prototype
   runs the installs one after another. The card holds still, with no countdown,
   until every install is done; then it counts down 10 seconds. */
function setupCard(names) {
  var n = names.length, segs = ''; for (var i = 0; i < n; i++) segs += '<i><b></b></i>';
  var el = toast('Setting up ' + n + ' install' + (n === 1 ? '' : 's') + ' · 0 done', { segs: segs, hold: true, pinned: true });
  el.querySelector('.segs').insertAdjacentHTML('afterend', '<div class="ts" id="setup-step"></div>');
  var plan = names.map(function () { return { dl: 1200 + Math.random() * 1200, inst: 700 + Math.random() * 700 }; });
  var idx = 0, t0 = performance.now();
  var t = setInterval(function () {
    var bars = el.querySelectorAll('.segs b'), step = el.querySelector('#setup-step'), now = performance.now();
    if (idx < n) {
      var e = now - t0, P = plan[idx], pct, word;
      if (e < P.dl) { pct = e / P.dl * 50; word = 'downloading ' + Math.round(e / P.dl * 100) + '%'; }
      else { pct = 50 + Math.min(1, (e - P.dl) / P.inst) * 50; word = 'installing…'; }
      bars[idx].style.width = pct + '%'; bars[idx].parentNode.className = 'run';
      step.textContent = names[idx] + ' · ' + word;
      if (pct >= 100) { bars[idx].parentNode.className = 'done'; idx++; t0 = now; el.querySelector('.tt').textContent = idx >= n ? n + ' install' + (n === 1 ? '' : 's') + ' done' : 'Setting up ' + n + ' installs · ' + idx + ' done'; }
    }
    if (idx >= n) {
      clearInterval(t);
      step.textContent = n + ' system' + (n === 1 ? ' is' : 's are') + ' ready to play.';
      el._pinned = false; el.release(10000);
    }
  }, 50);
}
function viewer(i) {
  var g = byId(S.pages[S.pages.length - 1].id), n = 6, ratios = [16 / 9, 4 / 3, 16 / 9, 8 / 7, 16 / 9, 16 / 9];
  var L = openLayer({ html: '', cls: 'viewer-l', hints: function () { return [['LB', 'Previous'], ['RB', 'Next'], ['B', 'Close']]; } });
  L.el.className = 'viewer';
  function draw() { L.el.innerHTML = '<div class="shot" style="--c:' + g.c + ';aspect-ratio:' + ratios[i] + ';filter:hue-rotate(' + i * 25 + 'deg)">' + (i === 5 ? '<span class="play" style="position:absolute;inset:0;display:flex;align-items:center;justify-content:center">' + gIcon('play', 64) + '</span>' : '') + '</div><div class="cap">' + gl('LB') + '<span>' + (i < 5 ? 'Screenshot ' + (i + 1) + ' of 5' : 'Trailer') + ' · ' + esc(g.t) + '</span>' + gl('RB') + '</div><button data-f class="sr">media</button>'; restoreFocus(); }
  L.lb = function () { if (i > 0) { i--; draw(); } }; L.rb = function () { if (i < n - 1) { i++; draw(); } }; L.left = L.lb; L.right = L.rb;
  draw();
}

/* ------------------------------------------------------------------ render */
var shownScr = null;
function renderStage() {
  var scr = S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab];
  /* Keep each screen's scroll: a redraw, or coming back to it, puts every rail,
     row and list where it was. */
  if (shownScr) shownScr._scroll = saveScroll();
  $('stage').innerHTML = scr.render();
  if (scr.after) scr.after();
  loadScroll(scr._scroll);
  shownScr = scr;
  paintTop();
  if (cur && !document.body.contains(cur)) { var k = cur.dataset.k, el = k && scope().querySelector('[data-k="' + k + '"]'); if (el) { INSTANT = true; setFocus(el); INSTANT = false; } }
}
function render() {
  var scr = S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab];
  renderStage();
  INSTANT = true; if (!S.layers.length) restoreFocus(scr); INSTANT = false;
  if (S.tab === 'home' && !S.pages.length && cur && !cur.dataset.hero) hero(byId(1));
  paintHints();
}
function repaint() {
  if (S.layers.length) { if ($('pane') && S.tab === 'settings' && !S.pages.length) setPaint(); else renderStage(); paintHints(); return; }
  var k = cur && document.body.contains(cur) && cur.dataset.k, box = cur && cur.closest('[id]'), bid = box && box.id, idx = box ? focusables(box).indexOf(cur) : -1;
  if ($('pane') && S.tab === 'settings' && !S.pages.length) setPaint(); else renderStage();
  var el = k && scope().querySelector('[data-k="' + k + '"]');
  if (!el && bid && $(bid)) { var L = focusables($(bid)).filter(function (e) { return !e.disabled; }); el = L[Math.max(0, Math.min(idx, L.length - 1))]; }
  INSTANT = true; if (el) setFocus(el); else restoreFocus(); INSTANT = false;
  paintHints();
}
function advanceQueue() { if (!DL.some(function (d) { return d.st === 'active'; })) { var q = DL.filter(function (d) { return d.st === 'queued'; })[0]; if (q) q.st = 'active'; } }
setInterval(function () {
  var a = DL.filter(function (d) { return d.st === 'active'; })[0]; if (!a) return;
  a.pct = Math.min(100, a.pct + 3 + Math.floor(Math.random() * 4));
  if (a.pct >= 100) { a.st = 'done'; a.line = a.kind === 'Update' ? 'Updated' : a.kind === 'Library' ? 'Moved' : 'Installed · 1.2 GB'; var g = byId(a.gid); if (g) { g.inst = 1; } toast(a.kind === 'Library' ? 'Library moved' : a.t + (a.kind === 'Update' ? ' updated' : ' installed')); advanceQueue(); }
  if (S.layers.length) { paintTop(); return; }
  if (S.tab === 'downloads' && !S.pages.length) repaint();
  else if (S.pages.length && S.pages[S.pages.length - 1].id === a.gid) repaint();
  else { paintTop(); document.querySelectorAll('.cv[data-id="' + a.gid + '"] .tag.q').forEach(function (t) { t.textContent = a.st === 'active' ? a.pct + '%' : ''; }); }
}, 900);
setInterval(paintTop, 30000);

/* ------------------------------------------------------------------ input */
function press(b) {
  if (S.desk) { if (b === 'A' || b === 'B') { S.desk = false; $('desk').hidden = true; } return; }
  var top = S.layers[S.layers.length - 1];
  var scr = S.pages.length ? S.pages[S.pages.length - 1] : TABS[S.tab];
  if (top && top.osk) {
    if (b === 'X') return top.type('BACK'); if (b === 'Y') return top.type(' '); if (b === 'LB') return top.type('L'); if (b === 'RB') return top.type('R'); if (b === 'LT') return top.type('SHIFT');
    if (b === 'MENU') return top.type('DONE'); if (b === 'B') return closeLayer();
    if (b === 'A' && cur && cur.dataset.key) return top.type(cur.dataset.key);
  }
  if (top && top.el.classList.contains('viewer')) { if (b === 'B') return closeLayer(); if ((b === 'LB' || b === 'LEFT') && top.lb) return top.lb(); if ((b === 'RB' || b === 'RIGHT') && top.rb) return top.rb(); return; }
  if (b === 'UP' || b === 'DOWN' || b === 'LEFT' || b === 'RIGHT') {
    if (cur && cur.dataset.adj && (b === 'LEFT' || b === 'RIGHT') && !top) { srowAdj(cur, b === 'LEFT' ? -1 : 1); return; }
    /* The focused description reads like a page: Down / Up scroll it while more of
       it is out of view that way, then move on (Related below, the buttons above). */
    if (cur && cur.classList.contains('prose') && (b === 'UP' || b === 'DOWN')) {
      var tb = $('tbody');
      if (tb) {
        var pr = cur.getBoundingClientRect(), vr = tb.getBoundingClientRect(), rem = remPx(), step = 6 * rem, how = glide() ? 'smooth' : 'auto';
        if (b === 'DOWN' && pr.bottom > vr.bottom + 1) { tb.scrollBy({ top: Math.min(step, pr.bottom - vr.bottom + 1.2 * rem), behavior: how }); return; }
        if (b === 'UP' && pr.top < vr.top - 1) { tb.scrollBy({ top: -Math.min(step, vr.top - pr.top + 1.2 * rem), behavior: how }); return; }
      }
    }
    return move(b.toLowerCase());
  }
  if (b === 'A') {
    if (!cur) return;
    if (top) {
      if (cur.dataset.opt != null) { var it = top.items[+cur.dataset.opt]; closeLayer(); if (it && it.fn) it.fn(); return; }
      if (cur.dataset.cl) return closeLayer();
      if (cur.dataset.ok) { var fn = top.ok; closeLayer(); if (fn) fn(); return; }
      if (cur.dataset.relocate) { closeLayer(); openBrowser({ title: 'Relocate the library', start: '/run/media', onPick: function (p) { CFG.lib = p; toast('Library relocated to ' + p, { s: 'No files were moved' }); } }); return; }
      if (cur.dataset.gi != null) { var g1 = top.gi[+cur.dataset.gi]; closeLayer(); g1[1](); return; }
      if (cur.dataset.gp != null) { var g2 = top.gp[+cur.dataset.gp]; closeLayer(); g2[1](); return; }
    }
    if (cur.disabled) return;
    var a = cur.dataset.a; if (a && ACT[a]) return ACT[a](cur);
    return;
  }
  if (b === 'B') {
    if (top) { if (!top.blocking) closeLayer(); return; }
    if (S.startup) { if (S.pages.length > 1) pop(); return; }
    if (S.pages.length) return pop();
    var rail = document.querySelector('.rail');
    if (rail && cur && !rail.contains(cur)) { var sel = rail.querySelector('.ri.sel') || rail.querySelector('[data-f]'); return setFocus(sel); }
    if (S.tab !== 'home') { S.tab = 'home'; render(); }
    return;
  }
  if (top) return;
  if (b === 'X') { if (cur && cur.dataset.x === 'quick') return ACT.quick(cur); if (cur && cur.dataset.xa && ACT[cur.dataset.xa]) return ACT[cur.dataset.xa](cur); if (scr.x) return scr.x(); return; }
  if (b === 'Y') { if (scr.y) scr.y(); return; }
  if (b === 'LT') { if (scr.lt) scr.lt(); return; }
  if (b === 'LB' || b === 'RB') {
    if (scr.lb && b === 'LB') return scr.lb(); if (scr.rb && b === 'RB') return scr.rb();
    if (S.pages.length || S.startup) return;
    var i = TAB_ORDER.indexOf(S.tab); S.tab = TAB_ORDER[(i + (b === 'LB' ? -1 : 1) + TAB_ORDER.length) % TAB_ORDER.length]; render(); return;
  }
  if (b === 'MENU' && scr.menu) return scr.menu();
  if (b === 'MENU' || b === 'GUIDE') { if (!S.startup) guideMenu(); return; }
  if (b === 'VIEW') { var n = newestAction(); if (n) { n.remove(); n._act[1](); } else if (S.startup && scr.F) toast('Pasted from the clipboard'); return; }
}

var KEYMAP = { ArrowUp: 'UP', ArrowDown: 'DOWN', ArrowLeft: 'LEFT', ArrowRight: 'RIGHT', Enter: 'A', ' ': 'A', Escape: 'B', Backspace: 'B', x: 'X', y: 'Y', q: 'LB', e: 'RB', z: 'LT', m: 'MENU', v: 'VIEW', g: 'GUIDE' };
document.addEventListener('keydown', function (e) {
  if (e.key === 'PageDown' || e.key === 'PageUp') { e.preventDefault(); var v = $('tbody') || (cur && vScroller(cur)); if (v) v.scrollBy({ top: (e.key === 'PageDown' ? 1 : -1) * v.clientHeight * .7, behavior: glide() ? 'smooth' : 'auto' }); return; }
  var top = S.layers[S.layers.length - 1];
  if (top && top.osk && e.key.length === 1 && !e.ctrlKey && !e.metaKey) { e.preventDefault(); top.osk.phys = true; top.type(e.key); return; }
  if (top && top.osk && e.key === 'Enter' && top.osk.phys) { e.preventDefault(); top.type('DONE'); return; }
  if (top && top.osk && e.key === 'Backspace') { e.preventDefault(); top.type('BACK'); return; }
  var b = KEYMAP[e.key] || KEYMAP[e.key.toLowerCase()]; if (!b) return;
  e.preventDefault(); press(b);
});
document.addEventListener('mousemove', function () { mouseMoved = true; });
document.addEventListener('mouseover', function (e) { if (!mouseMoved) return; var f = e.target.closest('[data-f]'); if (f && scope().contains(f) && f !== cur) setFocus(f, true); });
document.addEventListener('click', function (e) {
  var h = e.target.closest('[data-btn]'); if (h) return press(h.dataset.btn);
  var t = e.target.closest('[data-tab]'); if (t && !S.pages.length && t.classList.contains('tab')) { S.tab = t.dataset.tab; return render(); }
  var dt = e.target.closest('[data-dtab]'); if (dt) return detShowTab(dt.dataset.dtab);
  if (S.desk) return press('A');
  var f = e.target.closest('[data-f]'); if (f && scope().contains(f)) { setFocus(f, true); press('A'); }
});

/* Real controllers through the Gamepad API (standard mapping). Held directions
   repeat after 400 ms, every 90 ms; the left stick works like the d-pad. */
var PADB = { 0: 'A', 1: 'B', 2: 'X', 3: 'Y', 4: 'LB', 5: 'RB', 6: 'LT', 7: 'RT', 8: 'VIEW', 9: 'MENU', 12: 'UP', 13: 'DOWN', 14: 'LEFT', 15: 'RIGHT', 16: 'GUIDE' };
var held = {};
/* The right stick scrolls what you're reading without moving focus: the Details
   tab, else the focused item's scrolling area. */
function stickScroll(px) {
  if (S.layers.length) return;
  var v = $('tbody') || (cur && vScroller(cur)); if (v) v.scrollBy({ top: px });
}
function pollPads() {
  var pads = navigator.getGamepads ? navigator.getGamepads() : [], now = performance.now(), any = null;
  for (var i = 0; i < pads.length; i++) { var p = pads[i]; if (!p) continue; any = p;
    var st = {}; for (var k in PADB) if (p.buttons[k] && p.buttons[k].pressed) st[PADB[k]] = 1;
    var ry = p.axes[3] || 0; if (Math.abs(ry) > .25) stickScroll(ry * 22);
    var ax = p.axes[0] || 0, ay = p.axes[1] || 0; if (ax < -.5) st.LEFT = 1; if (ax > .5) st.RIGHT = 1; if (ay < -.5) st.UP = 1; if (ay > .5) st.DOWN = 1;
    for (var b in st) { if (!held[b]) { held[b] = now + 400; press(b); } else if (/UP|DOWN|LEFT|RIGHT/.test(b) && now > held[b]) { held[b] = now + 90; press(b); } }
    for (var h in held) if (!st[h]) delete held[h];
  }
  if (any) { var name = any.id.replace(/\(.*?\)/g, '').trim() || 'Controller'; var ps = /054c|playstation|dualsense|dualshock/i.test(any.id); if (name !== S.padName || (ps ? 'ps' : 'xbox') !== S.scheme) { S.padName = name; S.scheme = ps ? 'ps' : 'xbox'; paintHints(); paintTop(); } }
  requestAnimationFrame(pollPads);
}
requestAnimationFrame(pollPads);

/* ------------------------------------------------------------------ start */
document.documentElement.setAttribute('data-theme', 'dark');
render();
hero(byId(1));
setTimeout(function () { toast('Welcome to TV mode', { s: 'Press ' + GL[scheme()].MENU + ' for the menu. Desktop mode is one press away there.' }); }, 600);
})();
