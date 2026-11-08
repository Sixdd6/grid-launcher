/* ==== Proposed: one Systems tab in Settings for every emulator choice,
   install, update and the Windows (Proton) setup. ==== */
var SY = {
  name: { PS: 'PlayStation', PS2: 'PlayStation 2', N64: 'Nintendo 64', SNES: 'Super Nintendo', GC: 'GameCube', SAT: 'Sega Saturn', PC: 'Windows' },
  games: { PS: 48, PS2: 16, N64: 24, SNES: 32, GC: 24, SAT: 16, PC: 9 },
  order: ['GC', 'N64', 'PS', 'PS2', 'SAT', 'SNES'],
  pick: { PS: 'DuckStation', PS2: '', N64: '', SNES: '', GC: 'Dolphin', SAT: 'Ymir' },
  rec: { PS: 'DuckStation', PS2: 'PCSX2', N64: 'RetroArch', SNES: 'RetroArch', GC: 'Dolphin', SAT: 'Ymir' },
  core: { N64: 'Mupen64Plus-Next', SNES: 'Snes9x' },
  /* What each emulator can play (from its profile's platform keywords). */
  can: { DuckStation: ['PS'], PCSX2: ['PS2'], RetroArch: ['N64', 'SNES', 'PS'], Dolphin: ['GC'], Ymir: ['SAT'], Mednafen: ['PS', 'SAT', 'SNES'] },
  ver: { DuckStation: 'v0.1-9226', PCSX2: 'v2.4.0', RetroArch: '1.21.0', Dolphin: '2509', Ymir: 'v0.4.0', Mednafen: '1.32.1' },
  upd: { Ymir: 'v0.4.1' },
  tips: { DuckStation: ['Controller: Settings → Controls → Auto Map', 'Press Esc to close the emulator'], Dolphin: ['Controller: Controllers → Configure → Standard Controller'] },
  paths: { DuckStation: ['~/Games/GRID/emulators/duckstation/DuckStation.AppImage', '-fullscreen "%rom%"'], Dolphin: ['~/Games/GRID/emulators/dolphin/dolphin-emu', '-b -e "%rom%"'], Ymir: ['~/Games/GRID/emulators/ymir/ymir-sdl3', '-f "%rom%"'], Mednafen: ['/usr/bin/mednafen', '"%rom%"'] },
  installed: ['DuckStation', 'Dolphin', 'Ymir', 'Mednafen'],
  fw: { PS: 'installed', SAT: 'install' },
  proton: 'GE-Proton10-17', protonRec: 'GE-Proton10-17',
  protons: ['GE-Proton10-17', 'Wine 10.0 (system)', 'Proton 9.0 (Steam)', 'Proton Experimental (Steam)'], protonGet: ['Proton-CachyOS 10.0-20251031'],
  umu: 'system', open: {}, busy: {}, menu: null, sheet: null, adv: false
};
function syHas(n) { return SY.installed.indexOf(n) >= 0; }
function syNeeds() { return SY.order.filter(function (k) { return !SY.pick[k] && SY.rec[k]; }); }
function syEsc(v) { return esc(v).replace(/"/g, '&quot;'); }

function syStatus(k) {
  var e = SY.pick[k];
  if (k === 'PC') return '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Ready</span>';
  if (SY.busy[k]) return '<span class="pill-s">Installing…</span>';
  if (!e) return '<span class="pill-s warn">! No emulator</span>';
  if (SY.fw[k] === 'install') return '<span class="pill-s warn">! Firmware needed</span>';
  if (SY.fw[k] === 'requested') return '<span class="pill-s">Installing firmware…</span>';
  if (SY.upd[e]) return '<span class="pill-s info">Update available</span>';
  return '<span class="pill-s ok"><span data-icon="check" data-size="12"></span>Ready</span>';
}

function syPicker(k) {
  var cur = SY.pick[k], have = [], get = [];
  Object.keys(SY.can).forEach(function (n) { if (SY.can[n].indexOf(k) < 0) return; (syHas(n) ? have : get).push(n); });
  get.sort(function (a, b) { return (b === SY.rec[k]) - (a === SY.rec[k]); });
  var o = '<option value=""' + (cur ? '' : ' selected') + '>None</option>';
  if (have.length) o += '<optgroup label="Installed">' + have.map(function (n) { return '<option value="' + n + '"' + (n === cur ? ' selected' : '') + '>' + n + (n === SY.rec[k] ? ' (recommended)' : '') + '</option>'; }).join('') + '</optgroup>';
  if (get.length) o += '<optgroup label="Download">' + get.map(function (n) { return '<option value="install:' + n + '">Install ' + n + (n === SY.rec[k] ? ' (recommended)' : '') + '</option>'; }).join('') + '</optgroup>';
  return '<select class="field sys-pick" data-sy-pick="' + k + '" aria-label="Emulator for ' + SY.name[k] + '"' + (SY.busy[k] ? ' disabled' : '') + '>' + o + '</select>';
}

function syEmuBlock(n, where) {
  var menu = SY.menu === where ? '<div class="g-menu row-menu" role="menu"><button data-sy-edit="' + n + '">Settings…</button><button data-sy-folder>Show program folder</button><button class="danger" data-sy-remove="' + n + '">Remove ' + n + '…</button></div>' : '';
  var used = SY.order.filter(function (k) { return SY.pick[k] === n; }).map(function (k) { return SY.name[k]; });
  return '<div class="emu-line"><span><b>' + n + '</b> <span class="muted-s">' + SY.ver[n] + (SY.upd[n] ? '' : ' · up to date') + (used.length > 1 ? ' · also plays ' + used.filter(function (u) { return u !== SY.name[where]; }).join(', ') : '') + '</span></span><span class="acts">' +
    (SY.upd[n] ? '<button class="small-btn primary" data-sy-update="' + n + '">Update to ' + SY.upd[n] + '</button>' : '') +
    '<button class="small-btn" data-toast="Opens ' + n + ' on its own, to change its settings">Open</button>' +
    '<span class="menu-anchor"><button class="g-icon-btn" aria-label="More for ' + n + '" data-sy-more="' + where + '"><span data-icon="more" data-size="16"></span></button>' + menu + '</span></span></div>';
}

function syRow(k) {
  var e = SY.pick[k], open = !!SY.open[k] || !e;
  var body = '';
  if (open) {
    if (!e) {
      body = !SY.rec[k] ? '<p class="note">No emulator GRID installs plays this system. If you have one, use Add an emulator manually.</p>' : SY.busy[k] ? '<p class="note">Installing ' + SY.rec[k] + '…</p>' :
        '<div class="fix"><span>Games for this system can’t start yet. ' + SY.rec[k] + ' is recommended.</span><button class="small-btn primary" data-sy-install="' + SY.rec[k] + '" data-for="' + k + '">Install ' + SY.rec[k] + '</button></div>';
    } else {
      body = syEmuBlock(e, k);
      if (e === 'RetroArch') body += '<div class="dfield"><span>Core</span><select class="field" style="max-width:260px"><option>' + SY.core[k] + '</option></select></div>';
      if (SY.fw[k]) {
        var st = SY.fw[k];
        body += '<div class="check-row">' + (st === 'installed' ? '<span class="st ok" data-icon="check" data-size="14"></span>' : '<span class="st warn">!</span>') +
          '<span>' + SY.name[k] + ' firmware <span class="muted-s">· from your server</span></span><span class="acts">' +
          (st === 'installed' ? '<span class="muted-s">Installed</span>' : st === 'requested' ? '<button class="small-btn" disabled>Installing…</button>' : '<button class="small-btn primary" data-sy-fw="' + k + '">Install</button>') + '</span></div>';
      }
      if (SY.tips[e]) body += '<details class="tips"><summary>Setup tips</summary><ul>' + SY.tips[e].map(function (t) { return '<li>' + esc(t) + '</li>'; }).join('') + '</ul></details>';
    }
  }
  return '<li class="sysrow' + (e ? '' : ' attention') + (open ? ' open' : '') + '"><div class="sys-head"><button class="sys-toggle" data-sy-toggle="' + k + '" aria-expanded="' + open + '"' + (e ? '' : ' disabled') + '><span class="chev" data-icon="chevronRight" data-size="14"></span><b>' + SY.name[k] + '</b><span class="muted-s">' + SY.games[k] + ' games</span></button>' +
    syPicker(k) + syStatus(k) + '</div>' + (body ? '<div class="sys-body">' + body + '</div>' : '') + '</li>';
}

/* Proton choices: what is on this computer (system Wine, Steam's Proton,
   GRID's downloads), then the versions GRID can download, the recommended one
   first. `cur` is an installed name or "install:<name>". */
function syProtonOptions(cur) {
  var rec = function (p) { return p === SY.protonRec ? ' (recommended)' : ''; };
  var get = SY.protonGet.slice().sort(function (a, b) { return (b === SY.protonRec) - (a === SY.protonRec); });
  return '<optgroup label="Installed">' + SY.protons.map(function (p) { return '<option value="' + syEsc(p) + '"' + (p === cur ? ' selected' : '') + '>' + p + rec(p) + '</option>'; }).join('') + '</optgroup>' +
    (get.length ? '<optgroup label="Download">' + get.map(function (p) { return '<option value="install:' + syEsc(p) + '"' + ('install:' + p === cur ? ' selected' : '') + '>Install ' + p + rec(p) + '</option>'; }).join('') + '</optgroup>' : '');
}
function syWindows() {
  var open = !!SY.open.PC;
  var o = syProtonOptions(SY.proton);
  var umu = SY.umu === 'system' ? 'provided by your system' : SY.umu === 'busy' ? 'installing…' : 'installed by GRID · 1.2.9';
  var body = open ? '<div class="sys-body"><p class="note">Windows games run through Proton on Linux. Most people never need to change this.</p>' +
    '<div class="check-row"><span class="st ok" data-icon="check" data-size="14"></span><span>Proton launcher (umu-run) <span class="muted-s">· ' + umu + '</span></span><span class="acts"><button class="small-btn" data-sy-umu' + (SY.umu === 'busy' ? ' disabled' : '') + '>' + (SY.umu === 'system' ? 'Use GRID’s copy' : 'Reinstall') + '</button></span></div>' +
    '<p class="note small">The first Windows game you launch downloads the Steam Linux Runtime (about 200 MB), so it takes longer than later ones.</p></div>' : '';
  return '<li class="sysrow' + (open ? ' open' : '') + '"><div class="sys-head"><button class="sys-toggle" data-sy-toggle="PC" aria-expanded="' + open + '"><span class="chev" data-icon="chevronRight" data-size="14"></span><b>Windows</b><span class="muted-s">' + SY.games.PC + ' games · Linux only</span></button>' +
    '<select class="field sys-pick" data-sy-proton aria-label="Proton version for Windows games"' + (SY.busy.PC ? ' disabled' : '') + '>' + o + '</select>' + (SY.busy.PC ? '<span class="pill-s">Installing…</span>' : syStatus('PC')) + '</div>' + body + '</li>';
}

function sySheet(n) {
  var p = n ? SY.paths[n] || ['', '"%rom%"'] : ['', ''];
  return '<div class="sheet2"><div class="sheet2-head"><h3>' + (n ? esc(n) + ' settings' : 'Add an emulator you already have') + '</h3><button class="g-icon-btn" aria-label="Close" data-sy-close><span data-icon="close" data-size="16"></span></button></div><form class="form" onsubmit="return false">' +
    (n ? '' : '<label>Which emulator is it?<select class="field"><option>Mednafen</option><option>PCSX2</option><option>Dolphin</option><option>Something else</option></select><span class="help">GRID fills in the rest for emulators it knows.</span></label>') +
    '<label>Program<span class="inline"><input class="field" value="' + syEsc(p[0]) + '" placeholder="Choose the emulator’s program file"><button class="g-btn secondary" type="button">Browse…</button></span></label>' +
    '<label>Launch options<input class="field" value="' + syEsc(p[1]) + '"><span class="help">%rom% is replaced by the game file.</span></label>' +
    '<details class="tips"' + (SY.adv ? ' open' : '') + '><summary data-sy-adv>Cloud saves (advanced)</summary><div class="form">' +
    '<label>How this emulator stores saves<select class="field"><option>Find out automatically</option><option>One file per game</option><option>A folder per game</option></select></label>' +
    '<label>Save folders<input class="field" placeholder="memcards; saves"><span class="help">Separate folders with ;</span></label>' +
    '<label>Save state folders<input class="field" placeholder="savestates"></label>' +
    '<label>Don’t upload these files<input class="field" placeholder="settings.ini"></label>' +
    '<label>Don’t upload these file types<input class="field" placeholder=".log; .tmp"></label></div></details>' +
    '<div class="form-actions"><button class="g-btn" type="button" data-sy-save>Save</button><button class="g-btn secondary" type="button" data-sy-close>Cancel</button></div></form></div>';
}

function renderSystems(el) {
  var needs = syNeeds().filter(function (k) { return !SY.busy[k]; });
  var order = SY.order.slice().sort(function (a, b) { return (!!SY.pick[a] - !!SY.pick[b]) || SY.name[a].localeCompare(SY.name[b]); });
  var others = SY.installed.filter(function (n) { return !SY.order.some(function (k) { return SY.pick[k] === n; }); });
  var h = '<div class="pane-head"><div><h2>Systems</h2><p class="lede">Each system on your server needs an emulator to play its games. Pick one here; GRID installs, updates and sets it up.</p></div>' +
    '<div class="head-acts"><button class="g-btn secondary" data-sy-manual>Add an emulator manually</button>' +
    (needs.length ? '<button class="g-btn" data-sy-all>Install recommended (' + needs.length + ')</button>' : '') + '</div></div>' +
    (SY.sheet !== null ? sySheet(SY.sheet === 'new' ? null : SY.sheet) : '') +
    '<ul class="sys-list">' + order.map(syRow).join('') + syWindows() + '</ul>' +
    '<div class="sub" style="margin-top:6px"><h3>Other emulators</h3>' +
    (others.length ? '<ul class="sys-list">' + others.map(function (n) { return '<li class="sysrow open"><div class="sys-body" style="border-top:none">' + syEmuBlock(n, 'other-' + n) + '<p class="meta2">Not used by any system. It can play ' + SY.can[n].map(function (k) { return SY.name[k]; }).join(', ') + '.</p></div></li>'; }).join('') + '</ul>' : '<p class="note">None. Emulators that aren’t picked for a system show here.</p>') +
    '</div>';
  el.innerHTML = h;
  gIcons(el);
  syPlaceMenu();
  el.onclick = function (e) { if (S.setPane === 'systems') syClick(e); };
  el.onchange = function (e) { if (S.setPane === 'systems') syChange(e); };
}

/* The ⋯ menu: below the button, right-aligned, when it fits; otherwise beside
   it (left, else right), shifted to stay on screen. It never covers its
   button and never opens upward over it. "Screen" is the app window. */
var SY_EDGE = 8, SY_GAP = 4;
function syPlaceMenu() {
  var m = document.querySelector('#set-pane .row-menu');
  if (!m) return;
  var b = m.parentNode.querySelector('[data-sy-more]').getBoundingClientRect();
  var win = document.getElementById('app').getBoundingClientRect();
  m.style.position = 'fixed'; m.style.right = 'auto'; m.style.left = '0px'; m.style.top = '0px';
  var w = m.offsetWidth, h = m.offsetHeight, top, left;
  var minX = win.left + SY_EDGE, maxX = win.right - SY_EDGE, minY = win.top + SY_EDGE, maxY = win.bottom - SY_EDGE;
  if (b.bottom + SY_GAP + h <= maxY) {
    top = b.bottom + SY_GAP;
    left = Math.max(minX, Math.min(b.right - w, maxX - w));
    m.dataset.side = 'below';
  } else {
    top = Math.max(minY, Math.min(b.top, maxY - h));
    if (b.left - SY_GAP - w >= minX) { left = b.left - SY_GAP - w; m.dataset.side = 'left'; }
    else { left = Math.min(b.right + SY_GAP, maxX - w); m.dataset.side = 'right'; }
  }
  m.style.left = left + 'px'; m.style.top = top + 'px';
}
/* A fixed menu would drift from its button as the list scrolls: close it. */
document.addEventListener('scroll', function () { if (SY.menu !== null) { SY.menu = null; syRerender(); } }, true);
window.addEventListener('resize', function () { if (SY.menu !== null) syPlaceMenu(); });

function syRerender() { renderSettings(); renderServer(); }

function syInstall(n, forSys) {
  SY.busy[forSys] = true; toast('Installing ' + n + '…'); syRerender();
  setTimeout(function () {
    delete SY.busy[forSys];
    if (forSys === 'PC') { SY.protons.push(n); SY.protonGet = SY.protonGet.filter(function (p) { return p !== n; }); SY.proton = n; toast(n + ' installed and used for Windows games'); syRerender(); return; }
    if (!syHas(n)) SY.installed.push(n);
    SY.order.forEach(function (k) { if (!SY.pick[k] && (k === forSys || SY.rec[k] === n) && SY.can[n].indexOf(k) >= 0) { SY.pick[k] = n; EMU[k] = n; } });
    if (SY.pick[forSys] !== n && SY.can[n].indexOf(forSys) >= 0) { SY.pick[forSys] = n; EMU[forSys] = n; }
    toast(n + ' installed for ' + SY.name[forSys]); syRerender();
  }, 1400);
}

function syClick(e) {
  var t = e.target, b;
  if (t.closest('select')) return;
  if ((b = t.closest('[data-sy-toggle]'))) { var k = b.getAttribute('data-sy-toggle'); SY.open[k] = !SY.open[k]; syRerender(); return; }
  if ((b = t.closest('[data-sy-install]'))) { syInstall(b.getAttribute('data-sy-install'), b.getAttribute('data-for')); return; }
  if (t.closest('[data-sy-all]')) { syNeeds().forEach(function (k, i) { setTimeout(function () { if (!SY.pick[k] && !SY.busy[k]) syInstall(SY.rec[k], k); }, i * 150); }); return; }
  if ((b = t.closest('[data-sy-more]'))) { var w = b.getAttribute('data-sy-more'); SY.menu = SY.menu === w ? null : w; syRerender(); return; }
  if ((b = t.closest('[data-sy-edit]'))) { SY.sheet = b.getAttribute('data-sy-edit'); SY.menu = null; syRerender(); return; }
  if (t.closest('[data-sy-folder]')) { SY.menu = null; toast('Opens the folder in your file manager'); syRerender(); return; }
  if ((b = t.closest('[data-sy-remove]'))) {
    var n = b.getAttribute('data-sy-remove');
    if (!b.classList.contains('armed')) { b.classList.add('armed'); b.textContent = 'Remove ' + n + '? Games and saves stay'; return; }
    SY.installed = SY.installed.filter(function (x) { return x !== n; });
    SY.order.forEach(function (k) { if (SY.pick[k] === n) { SY.pick[k] = ''; EMU[k] = 'none'; } });
    SY.menu = null; toast(n + ' removed'); syRerender(); return;
  }
  if ((b = t.closest('[data-sy-update]'))) { var u = b.getAttribute('data-sy-update'); toast('Updating ' + u + '…'); setTimeout(function () { SY.ver[u] = SY.upd[u]; delete SY.upd[u]; toast(u + ' updated'); syRerender(); }, 1200); return; }
  if ((b = t.closest('[data-sy-fw]'))) { var f = b.getAttribute('data-sy-fw'); SY.fw[f] = 'requested'; toast('Installing ' + SY.name[f] + ' firmware…'); syRerender(); setTimeout(function () { SY.fw[f] = 'installed'; toast(SY.name[f] + ' firmware installed'); syRerender(); }, 1300); return; }
  if (t.closest('[data-sy-umu]')) { e.stopPropagation(); SY.umu = 'busy'; syRerender(); setTimeout(function () { SY.umu = 'managed'; toast('umu-run installed'); syRerender(); }, 1200); return; }
  if (t.closest('[data-sy-manual]')) { SY.sheet = 'new'; syRerender(); return; }
  if (t.closest('[data-sy-close]')) { SY.sheet = null; syRerender(); return; }
  if (t.closest('[data-sy-save]')) { if (SY.sheet === 'new' && !syHas('Mednafen')) SY.installed.push('Mednafen'); SY.sheet = null; toast('Emulator saved'); syRerender(); return; }
  if (t.closest('[data-sy-adv]')) { SY.adv = !SY.adv; return; }
  if (SY.menu !== null && !t.closest('.row-menu')) { SY.menu = null; syRerender(); }
}

function syChange(e) {
  var t = e.target, k = t.getAttribute('data-sy-pick'), v = t.value;
  if (k) {
    if (v.indexOf('install:') === 0) { syInstall(v.slice(8), k); return; }
    SY.pick[k] = v; EMU[k] = v || 'none'; SY.open[k] = !!v; syRerender(); return;
  }
  if (t.hasAttribute('data-sy-proton')) {
    if (v.indexOf('install:') === 0) { syInstall(v.slice(8), 'PC'); return; }
    SY.proton = v; toast('Windows games now use ' + SY.proton); syRerender();
  }
}
Object.keys(SY.pick).forEach(function (k) { EMU[k] = SY.pick[k] || 'none'; });
