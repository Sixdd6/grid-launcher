/* ---- Library rail: storefront groups in their own panels, folds remembered (2026-11-16) ----
   TV mode's folders already open and close with A. Now each folder (Backup drives,
   Amazon Games, Epic Games, GOG, RomM, Steam) sits in its own inset panel, a closed
   folder shows its count, a closed folder holding the selected row wears the selected
   fill, and which folders are closed is one remembered list shared with desktop mode
   (the app keeps it in `[ui] rail_closed`; the prototype in localStorage
   `grid.railClosed`, which the desktop card reads too). A folder never seen is open. */
var TRF_KEY = 'grid.railClosed';
var TRF_IDS = ['drives', 'amazon', 'epic', 'gog', 'romm', 'steam'];
function trfFold(id) { return id === 'drives' || id === 'romm' ? id : 'st-' + id; }
function trfLoad() {
  var closed = [];
  try { var v = JSON.parse(localStorage.getItem(TRF_KEY) || '[]'); if (Array.isArray(v)) closed = v; } catch (e) { /* none remembered */ }
  TRF_IDS.forEach(function (id) { S.folds[trfFold(id)] = closed.indexOf(id) < 0; });
  return closed;
}
/* Ids the prototype doesn't know (a future store) stay in the list untouched. */
function trfSave() {
  var keep = [];
  try { var v = JSON.parse(localStorage.getItem(TRF_KEY) || '[]'); if (Array.isArray(v)) keep = v.filter(function (x) { return TRF_IDS.indexOf(x) < 0; }); } catch (e) { /* start clean */ }
  var closed = keep.concat(TRF_IDS.filter(function (id) { return S.folds[trfFold(id)] === false; }));
  try { localStorage.setItem(TRF_KEY, JSON.stringify(closed)); } catch (e) { /* this session only */ }
}
trfLoad();

/* Which folder holds a rail key. */
function trfGroupOf(key) {
  if (key.indexOf('drv:') === 0) return 'drives';
  if (key.indexOf('romm:') === 0 || key.indexOf('platform:') === 0) return 'romm';
  var m = /^st:([a-z]+):/.exec(key); return m ? m[1] : null;
}
/* A closed folder's count: Installed (stores, RomM), drives (Backup drives). */
function trfCount(id) {
  if (id === 'drives') { var n = readyDrives().length; return n ? n : null; }
  if (id === 'romm') return rInstalled().length;
  var s = sst(id); if (id === 'steam' && !s.client) return null;
  return sInst(s).length;
}
var trfRail = libRailHTML;
libRailHTML = function () {
  var t = document.createElement('template'); t.innerHTML = trfRail();
  var kids = Array.prototype.slice.call(t.content.childNodes), out = document.createDocumentFragment(), box = null;
  kids.forEach(function (el) {
    var fold = el.nodeType === 1 && el.matches('.ri.fold');
    if (fold) {
      var fid = el.getAttribute('data-fold'), id = fid.indexOf('st-') === 0 ? fid.slice(3) : fid, open = S.folds[fid] !== false;
      box = document.createElement('div');
      box.className = 'rgrp' + (open ? '' : ' closed');
      box.setAttribute('data-grp', id);
      if (!open) {
        var n = trfCount(id);
        if (n != null) el.insertAdjacentHTML('beforeend', '<span class="n" aria-label="' + (id === 'drives' ? plural(n, 'drive') : n + ' installed') + '">' + n + '</span>');
        /* Holding the selection: B / Left into the rail and arrival land here (`data-enter="sel"`). */
        if (trfGroupOf(S.libRail) === id) { el.classList.add('sel', 'holds'); el.setAttribute('data-holds-sel', ''); }
      }
      out.appendChild(box); box.appendChild(el); return;
    }
    if (box && el.nodeType === 1 && (el.matches('.ri.sub') || el.matches('.ri-empty'))) { box.appendChild(el); return; }
    box = null; out.appendChild(el);
  });
  var d = document.createElement('div'); d.appendChild(out);
  trfSave();
  return d.innerHTML;
};
/* Startup never opens a folder: Automatic landing on RomM › Discover keeps RomM as the
   player left it (its closed heading then holds the selection). */
var trfStart = openStart;
openStart = function () { var r = S.folds.romm; trfStart(); S.folds.romm = r; if (S.tab === 'library') repaint(); };
