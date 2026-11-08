/* ==== Server › Discover: the first rail item and the default view.
   New releases (a coverflow of the newest games on the preferred platforms)
   and Recently added (newest on the server, minus the coverflow's games). ==== */
var DISC = { cf: 0, ra: 0, sig: '' };
var TODAY = '2026-11-07';

/* Preferred platforms: every system with an emulator, Windows once a compat
   tool is picked for it, plus the games native to this computer (Linux here;
   on a Windows computer that would be the Windows games). */
function prefPlatforms() {
  var out = Object.keys(SY.pick).filter(function (k) { return SY.pick[k] && PLAT[k]; });
  if (SY.winSet) out.push('WIN');
  out.push('LINUX');
  return out;
}
/* New releases: games on the preferred platforms released in the last six
   months, newest first, at most 25. */
var NEW_MONTHS = 6, NEW_MAX = 25;
function monthsBefore(day, months) { var d = new Date(day + 'T00:00:00Z'); d.setUTCMonth(d.getUTCMonth() - months); return d.toISOString().slice(0, 10); }
function discNew() {
  var pref = prefPlatforms(), since = monthsBefore(TODAY, NEW_MONTHS);
  return G.filter(function (g) { return pref.indexOf(g.p) >= 0 && g.rel >= since && g.rel <= TODAY; })
    .sort(function (a, b) { return b.rel.localeCompare(a.rel); }).slice(0, NEW_MAX);
}
function discRecent(newest) {
  var ids = newest.map(function (g) { return g.id; });
  return G.filter(function (g) { return ids.indexOf(g.id) < 0; })
    .sort(function (a, b) { return b.added.localeCompare(a.added); }).slice(0, 8);
}
var MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
function fmtDate(d) { var p = d.split('-'); return (+p[2]) + ' ' + MONTHS[+p[1] - 1] + ' ' + p[0]; }
function addedAgo(d) {
  var days = Math.round((Date.parse(TODAY) - Date.parse(d)) / 86400000);
  if (days <= 0) return 'Added today';
  if (days === 1) return 'Added yesterday';
  if (days < 14) return 'Added ' + days + ' days ago';
  if (days < 60) return 'Added ' + Math.round(days / 7) + ' weeks ago';
  return 'Added ' + fmtDate(d);
}
function discAction(g) {
  if (g.missing) return 'Details';
  if (g.installed) return 'Play';
  if (S.dl && S.dl.id === g.id) return 'Installing…';
  if (DLQ.indexOf(g.id) >= 0) return 'Queued';
  return 'Install';
}
/* Screenshot shape per game (the prototype's: by era, with a few modern
   exceptions), and whether the game has a clear logo. */
var SHOT = { SNES: [8, 7], N64: [4, 3], PS: [4, 3], SAT: [4, 3], PS2: [4, 3], GC: [4, 3], WIN: [16, 9], LINUX: [16, 9] };
var SHOT_OVERRIDE = { 'Orbit Bloom': [16, 9], 'Signal Fires': [16, 9], 'Grey Meridian': [21, 9], 'Paper Orbit': [16, 10] };
var LOGO_PLATFORMS = ['WIN', 'LINUX', 'PS2', 'GC'], LOGO_TITLES = ['Cinder Peaks', 'Coral Static'];
function shotRatio(g) { return SHOT_OVERRIDE[g.t] || SHOT[g.p] || [16, 9]; }
function hasLogo(g) { return LOGO_PLATFORMS.indexOf(g.p) >= 0 || LOGO_TITLES.indexOf(g.t) >= 0; }
/* Every game gets a logo on its screenshot. A server clear logo is a
   transparent image (here a styled-text stand-in in fixed colours). Without
   one, GRID draws the title in one of the same three styles, coloured from the
   screenshot: a light tint of its dominant shade for the letters and a deep
   shade of it for the outline and shadow, so it matches the art and reads. */
function clearLogo(g) {
  var style = 'abc'.charAt(g.id % 3), vars = '';
  if (!hasLogo(g)) {
    var hsl = rgbToHsl(dominantShade(g));
    vars = ' data-generated style="--lf:hsl(' + hsl[0] + ' ' + Math.min(hsl[1], 70) + '% 88%);--ls:hsl(' + hsl[0] + ' ' + Math.min(hsl[1], 60) + '% 16%)"';
  }
  return '<div class="ra-logo logo-' + style + '" role="img" aria-label="' + syEsc(g.t) + '"' + vars + '>' + esc(g.t) + '</div>';
}

/* The screenshot's dominant shade: sample its own pixels (never the blurred
   fill around an off-ratio shot), group them into 4-bit-per-channel buckets,
   take the most common bucket and average its pixels. The prototype draws its
   placeholder screenshot to a small canvas first; the app samples the image. */
var SHADE_CACHE = {};
function dominantShade(g) {
  if (SHADE_CACHE[g.id]) return SHADE_CACHE[g.id];
  var r = shotRatio(g), w = 64, h = Math.round(64 * r[1] / r[0]);
  var cv = document.createElement('canvas'); cv.width = w; cv.height = h;
  var x = cv.getContext('2d');
  x.fillStyle = g.c; x.fillRect(0, 0, w, h);
  x.fillStyle = 'rgba(255,255,255,.14)'; x.fillRect(w * .08, h * .52, w * .46, h * .34);
  x.fillStyle = 'rgba(255,255,255,.18)'; x.beginPath(); x.arc(w * .77, h * .16 + w * .13, w * .13, 0, Math.PI * 2); x.fill();
  var px = x.getImageData(0, 0, w, h).data, buckets = {}, best = null;
  for (var i = 0; i < px.length; i += 4) {
    var k = (px[i] >> 4) << 8 | (px[i + 1] >> 4) << 4 | (px[i + 2] >> 4), b = buckets[k] || (buckets[k] = [0, 0, 0, 0]);
    b[0]++; b[1] += px[i]; b[2] += px[i + 1]; b[3] += px[i + 2];
    if (!best || b[0] > best[0]) best = b;
  }
  return (SHADE_CACHE[g.id] = [Math.round(best[1] / best[0]), Math.round(best[2] / best[0]), Math.round(best[3] / best[0])]);
}
function rgbToHsl(c) {
  var r = c[0] / 255, gg = c[1] / 255, b = c[2] / 255, mx = Math.max(r, gg, b), mn = Math.min(r, gg, b), l = (mx + mn) / 2, h = 0, s = 0;
  if (mx !== mn) { var d = mx - mn; s = l > .5 ? d / (2 - mx - mn) : d / (mx + mn);
    h = mx === r ? (gg - b) / d + (gg < b ? 6 : 0) : mx === gg ? (b - r) / d + 2 : (r - gg) / d + 4; h *= 60; }
  return [Math.round(h), Math.round(s * 100), Math.round(l * 100)];
}
function discArt(g, extra) { return '<div class="g-art" style="background:' + g.c + (extra || '') + '"><span>' + esc(g.t) + '</span></div>'; }

function renderDiscover() {
  var el = $('discover'), newest = discNew(), recent = discRecent(newest);
  var sig = newest.map(function (g) { return g.id; }).join(',') + '|' + recent.map(function (g) { return g.id; }).join(',');
  if (sig !== DISC.sig || !el.firstChild) {
    DISC.sig = sig; DISC.cf = Math.min(DISC.cf, Math.max(newest.length - 1, 0)); DISC.ra = Math.min(DISC.ra, Math.max(recent.length - 1, 0));
    DISC.newest = newest; DISC.recent = recent;
    var cf = newest.length ? '<div class="cf" tabindex="0" aria-label="New releases" aria-roledescription="carousel">' +
      '<button class="g-icon-btn cf-arrow prev" aria-label="Previous" data-cf-prev><span data-icon="chevronLeft" data-size="20"></span></button>' +
      '<div class="cf-stage">' + newest.map(function (g, i) { return '<div class="cf-item" data-cf-i="' + i + '"><div class="cf-cover">' + discArt(g) + '</div></div>'; }).join('') + '</div>' +
      '<button class="g-icon-btn cf-arrow next" aria-label="Next" data-cf-next><span data-icon="chevronRight" data-size="20"></span></button></div>' +
      '<div class="cf-caption" id="cf-caption"></div>'
      : '<p class="note">Nothing released in the last six months for your systems. Add systems in Settings › Systems to see more.</p>';
    var ra = recent.length ? '<div class="ra">' +
      '<div class="ra-main"><div class="ra-shot" data-ra-open role="button" tabindex="0" aria-label="Open details"><div class="ra-layer" id="ra-a"></div><div class="ra-layer" id="ra-b"></div></div><div class="ra-caption" id="ra-caption"></div></div>' +
      '<div class="ra-side"><div class="ra-list" id="ra-list"><div class="ra-hl" id="ra-hl"><span class="ra-bar"></span></div>' +
      recent.map(function (g, i) { return '<button class="ra-row" data-ra-i="' + i + '"><span class="ra-cover">' + discArt(g) + '</span><span class="ra-text"><b>' + esc(g.t) + '</b><span>' + PLAT[g.p] + '</span></span></button>'; }).join('') +
      '</div></div></div>'
      : '<p class="note">Nothing new on your server yet.</p>';
    el.innerHTML = '<section class="g-panel disc-sec"><header class="disc-head"><h2>New releases</h2><p class="lede">Released in the last six months for your systems.</p></header>' + cf + '</section>' +
      '<section class="g-panel disc-sec"><header class="disc-head"><h2>Recently added</h2><p class="lede">New on your server.</p></header>' + ra + '</section>';
    gIcons(el);
    el.onclick = discClick;
    var stage = el.querySelector('.cf');
    var shot = el.querySelector('[data-ra-open]');
    if (shot) shot.onkeydown = function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openDetails(DISC.recent[DISC.ra]); } };
    if (stage) stage.onkeydown = function (e) { if (e.key === 'Enter') { e.preventDefault(); openDetails(DISC.newest[DISC.cf]); return; } if (e.key === 'ArrowLeft') { e.preventDefault(); cfGo(DISC.cf - 1); } if (e.key === 'ArrowRight') { e.preventDefault(); cfGo(DISC.cf + 1); } };
    var hl = $('ra-hl'); if (hl) hl.addEventListener('animationend', function () { raGo(DISC.ra + 1, true); });
    DISC.layer = 'a';
    cfLayout(); raGo(DISC.ra, false, true);
  } else { cfCaption(); raCaption(); }
}

/* Coverflow: the centre cover flat and large; the rest tilt back and recede
   in 3D, fading with distance. Only transforms change, so moves glide. */
function cfLayout() {
  var items = document.querySelectorAll('#discover .cf-item');
  items.forEach(function (it, i) {
    var d = i - DISC.cf, a = Math.abs(d), s = d < 0 ? -1 : 1;
    var t = d === 0 ? 'translateX(0) translateZ(0) rotateY(0deg)'
      : 'translateX(' + s * (150 + (a - 1) * 66) + 'px) translateZ(' + (-140 - (a - 1) * 40) + 'px) rotateY(' + (-s * 52) + 'deg)';
    it.style.transform = t;
    it.style.zIndex = 50 - a;
    it.style.opacity = a > 4 ? 0 : 1;
    it.classList.toggle('center', d === 0);
    it.setAttribute('aria-hidden', a > 4 ? 'true' : 'false');
  });
  var p = document.querySelector('[data-cf-prev]'), n = document.querySelector('[data-cf-next]');
  if (p) p.disabled = DISC.cf === 0; if (n) n.disabled = DISC.cf >= DISC.newest.length - 1;
  cfCaption();
}
function cfCaption() {
  var g = DISC.newest && DISC.newest[DISC.cf], c = $('cf-caption'); if (!g || !c) return;
  c.innerHTML = '<b>' + esc(g.t) + '</b><span class="meta2">' + PLAT[g.p] + ' · Released ' + fmtDate(g.rel) + '</span>' +
    '<div class="form-actions"><button class="g-btn" data-disc-act="' + g.id + '">' + discAction(g) + '</button></div>';
}
function cfGo(i) { DISC.cf = Math.max(0, Math.min(i, DISC.newest.length - 1)); cfLayout(); }

/* Recently added: the highlight frame glides to the active row; its bar fills
   over 10 s and, when it ends, moves on. Hover pauses the bar (CSS). */
function raGo(i, auto, first) {
  var list = DISC.recent; if (!list || !list.length) return;
  DISC.ra = ((i % list.length) + list.length) % list.length;
  var g = list[DISC.ra], rows = document.querySelectorAll('#discover .ra-row'), row = rows[DISC.ra], hl = $('ra-hl'), lst = $('ra-list');
  rows.forEach(function (r, k) { r.classList.toggle('active', k === DISC.ra); r.setAttribute('aria-current', k === DISC.ra ? 'true' : 'false'); });
  hl.style.height = row.offsetHeight + 'px';
  hl.style.transform = 'translateY(' + row.offsetTop + 'px)';
  var bar = hl.querySelector('.ra-bar'), nb = bar.cloneNode(); bar.replaceWith(nb);   // restart the 10 s fill
  lst.scrollTo({ top: Math.max(0, row.offsetTop - (lst.clientHeight - row.offsetHeight) / 2), behavior: first ? 'auto' : 'smooth' });
  var next = DISC.layer === 'a' ? 'b' : 'a', shot = $('ra-' + next);
  /* The frame is 16:9; a screenshot of another shape is fitted inside it,
     uncropped, over a blurred, darkened copy of itself. */
  var r = shotRatio(g), wide = r[0] / r[1] >= 16 / 9;
  shot.innerHTML = '<div class="ra-fill" style="background:' + g.c + '"><span></span><i></i></div>' +
    '<div class="ra-pic ' + (wide ? 'wide' : 'tall') + '" style="aspect-ratio:' + r[0] + ' / ' + r[1] + '"><div class="ra-art" style="background:' + g.c + '"><span></span><i></i></div></div>' +
    '<div class="ra-logo-wrap">' + clearLogo(g) + '</div>';
  shot.dataset.ratio = r[0] + ':' + r[1];
  if (first) { $('ra-a').classList.remove('on'); $('ra-b').classList.remove('on'); }
  shot.classList.add('on'); $('ra-' + DISC.layer).classList.remove('on'); DISC.layer = next;
  raCaption();
}
function raCaption() {
  var g = DISC.recent && DISC.recent[DISC.ra], c = $('ra-caption'); if (!g || !c) return;
  /* The title is the logo on the screenshot; the caption has the rest. */
  c.innerHTML = '<div><span class="meta2">' + PLAT[g.p] + ' · ' + addedAgo(g.added) + '</span></div>' +
    '<div class="form-actions"><button class="g-btn" data-disc-act="' + g.id + '">' + discAction(g) + '</button></div>';
}

function discClick(e) {
  var t = e.target, b;
  if (t.closest('[data-cf-prev]')) { cfGo(DISC.cf - 1); return; }
  if (t.closest('[data-cf-next]')) { cfGo(DISC.cf + 1); return; }
  if ((b = t.closest('[data-cf-i]'))) { var i = +b.getAttribute('data-cf-i'); if (i === DISC.cf) openDetails(DISC.newest[i]); else cfGo(i); return; }
  /* A game opens its Details when clicked: the centre cover, the screenshot,
     or the highlighted row; a click on another row selects it first. */
  if ((b = t.closest('[data-ra-i]'))) { var k = +b.getAttribute('data-ra-i'); if (k === DISC.ra) openDetails(DISC.recent[k]); else raGo(k); return; }
  if (t.closest('[data-ra-open]')) { openDetails(DISC.recent[DISC.ra]); return; }
  if ((b = t.closest('[data-disc-act]'))) {
    var g = G[+b.getAttribute('data-disc-act')];
    if (g.missing) openDetails(g); else if (g.installed) { S.playing = g.id; toast('Launching ' + g.t + '…'); } else startInstall(g);
    return;
  }
}

/* Discover is the first Server rail row, with a divider under it, and the
   default selection. */
function srvRail() {
  var plats = Object.keys(PLAT).sort(function (a, b) { return PLAT[a].localeCompare(PLAT[b]); });
  rail('srv-rail', [{ key: 'DISCOVER', label: 'Discover', sel: S.platform === 'DISCOVER', divider: true }].concat(
    plats.map(function (p) { return { key: p, label: PLAT[p], count: G.filter(function (g) { return g.p === p; }).length * 8, sel: S.platform === p }; })),
    function (k) { S.platform = k; renderServer(); });
}
var discRenderServer = renderServer;
renderServer = function () {
  var on = S.platform === 'DISCOVER';
  $('srv-head').hidden = on; $('srv-scroll').hidden = on; $('discover').hidden = !on;
  if (on) { srvRail(); renderDiscover(); return; }
  discRenderServer(); srvRail();
};
