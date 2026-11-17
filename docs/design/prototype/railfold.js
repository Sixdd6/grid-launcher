/* ---- Library rail: folding storefront groups, remembered (2026-11-16) ----
   Each group (Backup drives, then Amazon Games, Epic Games, GOG, RomM, Steam) sits in
   its own inset panel under a heading button: chevron, mark, name, flag, and while
   closed the group's count. Enter / Space / click opens and closes it. Which groups
   are closed is one remembered list shared with TV mode (the app keeps it in
   `[ui] rail_closed`; the prototype in localStorage `grid.railClosed`, which the TV
   card reads too). A group never seen is open. */
var RF_KEY = 'grid.railClosed';
var RF_NAMES = { drives: 'Backup drives', amazon: 'Amazon Games', epic: 'Epic Games', gog: 'GOG', romm: 'RomM', steam: 'Steam' };
var RF = { closed: (function () { try { var v = JSON.parse(localStorage.getItem(RF_KEY) || '[]'); return Array.isArray(v) ? v : []; } catch (e) { return []; } })() };
function rfSave() { try { localStorage.setItem(RF_KEY, JSON.stringify(RF.closed)); } catch (e) { /* this session only */ } }
function rfIsClosed(id) { return RF.closed.indexOf(id) >= 0; }
function rfSet(id, closed) {
  RF.closed = RF.closed.filter(function (x) { return x !== id; });
  if (closed) RF.closed.push(id);
  rfSave();
}
/* Which group a rail key belongs to (null for All games, Recent, Updates). */
function rfGroupOf(key) {
  if (!key) return null;
  if (key.indexOf('drive:') === 0) return 'drives';
  if (key.indexOf('romm:') === 0 || key.indexOf('platform:') === 0) return 'romm';
  var m = /^st:([a-z]+):/.exec(key); return m ? m[1] : null;
}
/* The closed heading's count: the Installed row's number (stores, RomM); the number
   of drives (Backup drives). None when Installed shows "!" (Steam not installed). */
function rfCount(id, rows) {
  if (id === 'drives') return { n: rows.length, t: rows.length === 1 ? '1 drive' : rows.length + ' drives' };
  var inst = rows.filter(function (r) { var k = r.getAttribute('data-key') || ''; return k === 'romm:installed' || /:inst$/.test(k); })[0];
  var c = inst && inst.querySelector('.g-rail-count');
  if (!c || c.classList.contains('st-warn') || !/^\d+$/.test(c.textContent.trim())) return null;
  var n = +c.textContent.trim(); return { n: n, t: n + ' installed' };
}
function rfDecorate(rail) {
  if (!rail || rail.querySelector('.rf-group')) return;
  var groups = [], g = null;
  Array.prototype.slice.call(rail.children).forEach(function (el) {
    if (el.classList.contains('g-rail-heading')) { g = { head: el, rows: [] }; groups.push(g); }
    else if (g && el.classList.contains('g-rail-item')) g.rows.push(el);
    else g = null;
  });
  groups.forEach(function (gr) {
    var id = rfGroupOf(gr.rows[0] && gr.rows[0].getAttribute('data-key')); if (!id) return;
    var closed = rfIsClosed(id), name = RF_NAMES[id];
    var active = gr.rows.filter(function (r) { return r.classList.contains('active'); })[0];
    var holds = closed && active;
    var mark = gr.head.querySelector('.st-mono'), flag = gr.head.querySelector('.st-flag'), cnt = rfCount(id, gr.rows);
    var sec = document.createElement('section');
    sec.className = 'rf-group' + (closed ? ' closed' : '') + (holds ? ' holds' : '');
    sec.setAttribute('data-rf-id', id);
    sec.setAttribute('aria-label', name);
    var rid = rail.id + '-rf-' + id;
    var head = '<button class="rf-head" data-rf="' + id + '" aria-expanded="' + !closed + '" aria-controls="' + rid + '"' +
      (holds ? ' title="Showing ' + esc(name) + ' › ' + esc(active.querySelector('.g-rail-label').textContent) + '" aria-description="Holds the view you’re in"' : '') + '>' +
      '<span class="rf-chev" data-icon="chevronRight" data-size="12" aria-hidden="true"></span>' +
      (mark ? mark.outerHTML : '') + '<span class="rf-name">' + esc(name) + '</span><span class="rf-sp"></span>' +
      (flag ? flag.outerHTML : '') +
      (cnt ? '<span class="rf-n" title="' + cnt.t + '" aria-label="' + cnt.t + '">' + cnt.n + '</span>' : '') + '</button>';
    sec.innerHTML = head + '<div class="rf-rows" id="' + rid + '" role="group" aria-label="' + esc(name) + '"' + (closed ? ' hidden' : '') + '></div>';
    var box = sec.querySelector('.rf-rows');
    gr.head.parentNode.insertBefore(sec, gr.head);
    gr.head.remove();
    gr.rows.forEach(function (r) { box.appendChild(r); });
  });
  if (typeof gIcons === 'function') gIcons(rail);
}
/* The Discover and platform screens copy the Library rail (romm.js); decorate both. */
var rfOrigSync = rmSyncSrvRail;
rmSyncSrvRail = function () {
  var lr = $('lib-rail'), sr = $('srv-rail');
  rfOrigSync();
  if (sr && lr && lr.querySelector('.rf-group')) {
    /* Copied after decoration: refresh which closed heading holds the selection. */
    sr.querySelectorAll('.rf-group').forEach(function (s) {
      var a = s.querySelector('.g-rail-item.active'), h = s.querySelector('.rf-head');
      var holds = s.classList.contains('closed') && !!a;
      s.classList.toggle('holds', holds);
      h.removeAttribute('title'); h.removeAttribute('aria-description');
      if (holds) { h.title = 'Showing ' + RF_NAMES[s.getAttribute('data-rf-id')] + ' › ' + a.querySelector('.g-rail-label').textContent; h.setAttribute('aria-description', 'Holds the view you’re in'); }
      var rows = s.querySelector('.rf-rows'); rows.id = 'srv-rail-rf-' + s.getAttribute('data-rf-id');
      h.setAttribute('aria-controls', rows.id);
    });
  } else rfDecorate(sr);
};
var rfOrigLib = renderLibrary;
renderLibrary = function () { rfOrigLib(); rfDecorate($('lib-rail')); rmSyncSrvRail(); };

/* A heading opens or closes its group in both rails at once; focus stays on it. */
function rfToggle(id) {
  var closed = !rfIsClosed(id); rfSet(id, closed);
  ['lib-rail', 'srv-rail'].forEach(function (r) {
    var s = $(r) && $(r).querySelector('[data-rf-id="' + id + '"]'); if (!s) return;
    var a = s.querySelector('.g-rail-item.active'), h = s.querySelector('.rf-head');
    s.classList.toggle('closed', closed); s.classList.toggle('holds', closed && !!a);
    s.querySelector('.rf-rows').hidden = closed; h.setAttribute('aria-expanded', String(!closed));
    h.removeAttribute('title'); h.removeAttribute('aria-description');
    if (closed && a) { h.title = 'Showing ' + RF_NAMES[id] + ' › ' + a.querySelector('.g-rail-label').textContent; h.setAttribute('aria-description', 'Holds the view you’re in'); }
  });
}
document.addEventListener('click', function (e) {
  var h = e.target.closest && e.target.closest('[data-rf]');
  if (h) { e.stopPropagation(); rfToggle(h.getAttribute('data-rf')); return; }
  /* A jump from elsewhere to a row (Show in Library, Open Discover, a drive's Show in
     Library) opens that row's group first, and that is remembered too. */
  var j = e.target.closest && e.target.closest('[data-bk-show],[data-st-show],[data-st-go],[data-rm-go]');
  if (!j) return;
  var key = j.hasAttribute('data-bk-show') ? 'drive:' + j.getAttribute('data-bk-show')
    : j.hasAttribute('data-st-show') ? 'st:' + j.getAttribute('data-st-show') + ':inst'
    : j.getAttribute('data-st-go') || j.getAttribute('data-rm-go');
  var id = rfGroupOf(key); if (id && rfIsClosed(id)) rfSet(id, false);
}, true);
