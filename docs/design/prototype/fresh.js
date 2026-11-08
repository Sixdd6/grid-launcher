/* ==== Fresh startup: Connect → Choose your systems → the app, with the
   setup card following the installs. Disconnect starts over. ==== */
SY.name.JAG = 'Atari Jaguar'; SY.games.JAG = 3; SY.order.push('JAG'); SY.pick.JAG = '';
var FR = { phase: null, form: { url: '', user: '', secret: '', lib: '~/Games/GRID', token: false }, busy: false, ticked: {}, choice: {}, proton: false, protonChoice: '', run: null };

function freshReset() {
  SY.installed = []; SY.order.forEach(function (k) { SY.pick[k] = ''; EMU[k] = 'none'; });
  SY.fw = {}; SY.upd = {}; SY.winSet = false; SY.open = {}; SY.busy = {}; SY.menu = null; SY.sheet = null;
  SY.protons = ['Wine 10.0 (system)', 'Proton 9.0 (Steam)', 'Proton Experimental (Steam)']; SY.protonGet = ['GE-Proton10-17', 'Proton-CachyOS 10.0-20251031']; SY.proton = 'Wine 10.0 (system)';
  G.forEach(function (g) { g.installed = false; g.update = false; g.missing = false; g.cloud = false; });
  S.view = 'server'; S.platform = 'DISCOVER'; S.setPane = 'systems'; S.libFilter = 'all'; S.detail = null; S.dl = null;
  FR.form = { url: '', user: '', secret: '', lib: '~/Games/GRID', token: false }; FR.busy = false; FR.ticked = {}; FR.choice = {}; FR.proton = false; FR.protonChoice = ''; FR.run = null;
  $('setup-card').hidden = true; $('scrim').hidden = true;
  DONE.length = 0; DLQ.length = 0;
}

function frOptions(k) {
  return Object.keys(SY.can).filter(function (n) { return SY.can[n].indexOf(k) >= 0; })
    .sort(function (a, b) { return (b === SY.rec[k]) - (a === SY.rec[k]) || a.localeCompare(b); });
}

function freshShow(phase) {
  FR.phase = phase;
  var el = $('fresh'), h;
  el.hidden = false;
  if (phase === 'connect') {
    var f = FR.form, ready = f.url.trim() && f.secret.trim() && f.lib.trim() && (f.token || f.user.trim());
    h = '<form class="fr-card" onsubmit="return false"><header class="fr-head"><span class="g-logo" data-icon="grid" data-size="28"></span><h1>Connect to RomM</h1><p class="lede">GRID plays the games on your RomM server.</p></header>' +
      '<label>Server URL<input class="field" data-fr="url" placeholder="https://romm.example" value="' + syEsc(f.url) + '"></label>' +
      (f.token ? '' : '<label>Username<input class="field" data-fr="user" autocomplete="off" value="' + syEsc(f.user) + '"></label>') +
      '<label>' + (f.token ? 'API token' : 'Password') + '<input class="field" type="password" data-fr="secret" autocomplete="off" value="' + syEsc(f.secret) + '"></label>' +
      '<label class="check"><input type="checkbox" data-fr-token' + (f.token ? ' checked' : '') + '> Use API token</label>' +
      '<label>Library folder<span class="inline"><input class="field" data-fr="lib" value="' + syEsc(f.lib) + '"><button class="g-btn secondary" type="button">Browse…</button></span><span class="help">Games, emulators and saves go here.</span></label>' +
      '<button class="g-btn" data-fr-connect' + (ready && !FR.busy ? '' : ' disabled') + '>' + (FR.busy ? 'Connecting…' : 'Connect') + '</button>' +
      '<p class="help center">Prototype: any values connect.</p></form>';
  } else {
    var count = {}, rows = SY.order.slice().sort(function (a, b) { return SY.name[a].localeCompare(SY.name[b]); }).map(function (k) {
      var opts = frOptions(k), t = !!FR.ticked[k];
      if (t) count[FR.choice[k]] = 1;
      var right = !opts.length ? '<span class="muted-s">No emulator available</span>' :
        t ? '<select class="field sys-pick" data-fr-pick="' + k + '">' + opts.map(function (n) { return '<option value="' + n + '"' + (n === FR.choice[k] ? ' selected' : '') + '>' + n + (n === SY.rec[k] ? ' (recommended)' : '') + '</option>'; }).join('') + '</select>' : '';
      return '<li class="sysrow fr-row' + (t ? ' ticked' : '') + (opts.length ? '' : ' off') + '"><label class="fr-pick"><input type="checkbox" data-fr-tick="' + k + '"' + (t ? ' checked' : '') + (opts.length ? '' : ' disabled') + '><b>' + SY.name[k] + '</b><span class="muted-s">' + SY.games[k] + ' games</span></label>' + right + '</li>';
    });
    rows.push('<li class="sysrow fr-row' + (FR.proton ? ' ticked' : '') + '"><label class="fr-pick"><input type="checkbox" data-fr-proton' + (FR.proton ? ' checked' : '') + '><b>Windows</b><span class="muted-s">' + SY.games.PC + ' games · runs through Proton</span></label>' +
      (FR.proton ? '<select class="field sys-pick" data-fr-proton-pick aria-label="Proton version for Windows games">' + syProtonOptions(FR.protonChoice) + '</select>' : '') + '</li>');
    var n = Object.keys(count).length, pDl = FR.proton && FR.protonChoice.indexOf('install:') === 0 ? FR.protonChoice.slice(8) : '';
    /* One count: a Proton download is one of the installs. */
    var all = n + (pDl ? 1 : 0);
    var label = all ? 'Install ' + all + ' emulator' + (all === 1 ? '' : 's') : FR.proton ? 'Continue' : 'Install emulators';
    h = '<section class="fr-card wide"><header class="fr-head"><span class="g-logo" data-icon="grid" data-size="28"></span><h1>Choose your systems</h1><p class="lede">Tick the systems you play. GRID installs an emulator for each and sets it up.</p></header>' +
      '<ul class="sys-list">' + rows.join('') + '</ul>' +
      '<footer class="fr-foot"><div class="form-actions"><button class="g-btn secondary" data-fr-skip>Skip for now</button><button class="g-btn" data-fr-install' + (n || FR.proton ? '' : ' disabled') + '>' + label + '</button></div>' +
      '<p class="help">You can change these later in Settings › Systems.</p></footer></section>';
  }
  el.innerHTML = h;
  gIcons(el);
}
function freshHide() { $('fresh').hidden = true; FR.phase = null; renderAll(); }

function frStartRun() {
  var jobs = {};
  Object.keys(FR.ticked).forEach(function (k) { if (FR.ticked[k]) { var n = FR.choice[k]; (jobs[n] = jobs[n] || []).push(k); } });
  var pDl = FR.proton && FR.protonChoice.indexOf('install:') === 0 ? FR.protonChoice.slice(8) : '';
  if (FR.proton) SY.winSet = true;
  if (FR.proton && !pDl) SY.proton = FR.protonChoice;
  if (pDl) jobs[pDl] = ['PC'];
  var names = Object.keys(jobs);
  if (!names.length) { if (FR.proton) toast('Windows games will use ' + SY.proton); renderAll(); return; }
  /* One item per install, Proton included: the card's count and its bar
     segments both come from this list. */
  FR.run = { items: names.map(function (n) { return { name: n, state: 'queued' }; }), systems: 0 };
  names.forEach(function (n) { jobs[n].forEach(function (k) { SY.busy[k] = true; }); });
  frCard();
  names.forEach(function (n, i) {
    setTimeout(function () { if (FR.run) { FR.run.items[i].state = 'running'; frCard(); } }, i * 1300);
    setTimeout(function () {
      jobs[n].forEach(function (k) { delete SY.busy[k]; });
      if (jobs[n][0] === 'PC') { SY.protons.unshift(n); SY.protonGet = SY.protonGet.filter(function (p) { return p !== n; }); SY.proton = n; if (FR.run) FR.run.systems += 1; }
      else {
        if (!syHas(n)) SY.installed.push(n);
        jobs[n].forEach(function (k) { SY.pick[k] = n; EMU[k] = n; if (FR.run) FR.run.systems += 1; });
        if (jobs[n].indexOf('PS') >= 0) SY.fw.PS = 'install';
        if (jobs[n].indexOf('SAT') >= 0) SY.fw.SAT = 'install';
      }
      addDone(n, jobs[n][0] === 'PC' ? 'Compat tool' : 'Emulator', 'Finished');
      if (FR.run) { FR.run.items[i].state = 'done'; frCard(); }
      renderSettings(); renderServer();
    }, 1600 + i * 1300);
  });
}
var FR_STATE = { queued: 'Waiting', running: 'Installing', done: 'Installed', failed: 'Didn’t install' };
function frCard() {
  var r = FR.run, c = $('setup-card');
  if (!r) { c.hidden = true; return; }
  var total = r.items.length, done = r.items.filter(function (it) { return it.state === 'done'; }).length, finished = done >= total;
  var noun = total + ' install' + (total === 1 ? '' : 's');
  var segs = r.items.map(function (it) { return '<span class="setup-seg ' + it.state + '" title="' + syEsc(it.name) + ' · ' + FR_STATE[it.state] + '"><b></b></span>'; }).join('');
  c.innerHTML = '<p class="title">' + (finished ? noun + ' done' : 'Setting up ' + noun + ' · ' + done + ' done') + '</p>' +
    '<div class="segbar" role="progressbar" aria-valuemin="0" aria-valuemax="' + total + '" aria-valuenow="' + done + '" aria-label="' + done + ' of ' + total + ' installs done">' + segs + '</div>' +
    (finished ? '<p class="line">' + r.systems + ' system' + (r.systems === 1 ? ' is' : 's are') + ' ready to play.</p>' : '') +
    '<div class="form-actions"><button class="small-btn" data-go="downloads">Open Downloads</button>' + (finished ? '<button class="small-btn note-dismiss" data-note-dismiss>Dismiss</button>' : '') + '</div>';
  c.hidden = false;
  if (finished) noteCountdown(c);
}

document.addEventListener('input', function (e) { var k = e.target.getAttribute && e.target.getAttribute('data-fr'); if (k) { FR.form[k] = e.target.value; var b = document.querySelector('[data-fr-connect]'); var f = FR.form; if (b) b.disabled = !(f.url.trim() && f.secret.trim() && f.lib.trim() && (f.token || f.user.trim())); } });
document.addEventListener('change', function (e) {
  var t = e.target;
  if (t.hasAttribute && t.hasAttribute('data-fr-token')) { FR.form.token = t.checked; freshShow('connect'); return; }
  var k = t.getAttribute && t.getAttribute('data-fr-tick');
  if (k) { FR.ticked[k] = t.checked; if (t.checked && !FR.choice[k]) FR.choice[k] = frOptions(k)[0]; freshShow('setup'); return; }
  if (t.hasAttribute && t.hasAttribute('data-fr-proton')) { FR.proton = t.checked; if (t.checked && !FR.protonChoice) FR.protonChoice = 'install:' + SY.protonRec; freshShow('setup'); return; }
  if (t.hasAttribute && t.hasAttribute('data-fr-proton-pick')) { FR.protonChoice = t.value; freshShow('setup'); return; }
  k = t.getAttribute && t.getAttribute('data-fr-pick');
  if (k) { FR.choice[k] = t.value; freshShow('setup'); }
});
document.addEventListener('click', function (e) {
  var t = e.target;
  if (t.closest('[data-fr-connect]')) { FR.busy = true; freshShow('connect'); setTimeout(function () { FR.busy = false; freshShow('setup'); }, 700); return; }
  if (t.closest('[data-fr-skip]')) { freshHide(); return; }
  if (t.closest('[data-fr-install]')) { freshHide(); frStartRun(); return; }
  var m = t.closest('#menu button');
  if (m && /Disconnect/.test(m.textContent)) { $('menu').hidden = true; freshReset(); renderAll(); freshShow('connect'); }
});

/* ==== One notification style: cards stacked at the bottom right. Each closes
   itself 10 s after it appears (the bar inside Dismiss runs down); pointing
   at a card pauses it. This toast() replaces the centred one. ==== */
var NOTE_MS = 10000, NOTE_MAX = 4;
function noteCountdown(card) {
  var btn = card.querySelector('[data-note-dismiss]');
  if (!btn || btn.querySelector('.cd')) return;
  var bar = document.createElement('span'); bar.className = 'cd'; bar.style.animationDuration = NOTE_MS + 'ms';
  bar.addEventListener('animationend', function () { noteClose(card); });
  btn.appendChild(bar);
}
function noteClose(card) {
  if (card.id === 'setup-card') { FR.run = null; card.hidden = true; card.innerHTML = ''; return; }
  card.classList.add('leaving'); setTimeout(function () { card.remove(); }, 180);
}
function toast(msg, kind) {
  var stack = $('notes');
  var cards = stack.querySelectorAll('.note-card:not(#setup-card)');
  if (cards.length >= NOTE_MAX) cards[0].remove();
  var c = document.createElement('aside');
  c.className = 'note-card' + (kind === 'error' ? ' error' : ''); c.setAttribute('role', kind === 'error' ? 'alert' : 'status');
  c.innerHTML = '<p class="title"></p><div class="form-actions"><button class="small-btn note-dismiss" data-note-dismiss>Dismiss</button></div>';
  c.querySelector('.title').textContent = msg;
  stack.appendChild(c);
  noteCountdown(c);
}
document.addEventListener('click', function (e) { var b = e.target.closest('[data-note-dismiss]'); if (b) noteClose(b.closest('.note-card')); });
