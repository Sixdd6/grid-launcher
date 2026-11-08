/* ---- emulators ---- */
var CATALOG = [
  { n: 'DuckStation', p: 'GitHub · stenzek/duckstation', tag: 'v0.1-9226', inst: true },
  { n: 'Dolphin', p: 'dolphin-emu.org', tag: '2509', inst: true },
  { n: 'Ymir (Sega Saturn)', p: 'GitHub · StrikerX3/Ymir', tag: 'v0.4.1', inst: true },
  { n: 'PCSX2', p: 'GitHub · PCSX2/pcsx2', tag: 'v2.4.0', inst: false },
  { n: 'RetroArch', p: 'buildbot.libretro.com', tag: '1.21.0', inst: false },
  { n: 'RPCS3', p: 'GitHub · RPCS3/rpcs3-binaries-linux', tag: 'build-0.0.37', inst: false },
  { n: 'Eden', p: 'git.eden-emu.dev', tag: 'v0.0.3', inst: false },
  { n: 'xemu', p: 'GitHub · xemu-project/xemu', tag: 'v0.8.92', inst: false },
  { n: 'PPSSPP', p: 'GitHub · hrydgard/ppsspp', tag: 'v1.19.3', inst: false },
  { n: 'Cemu', p: 'GitHub · cemu-project/Cemu', tag: 'v2.6', inst: false }
];
var COMPAT = [
  { n: 'GE-Proton10-17', p: 'GitHub · GloriousEggroll/proton-ge-custom', tag: 'GE-Proton10-17', inst: true },
  { n: 'Wine-GE 8-26', p: 'GitHub · GloriousEggroll/wine-ge-custom', tag: 'GE-Proton8-26', inst: false },
  { n: 'Proton Experimental (umu)', p: 'GitHub · Open-Wine-Components/umu-proton', tag: 'UMU-Proton-9.0-4e', inst: false }
];
var DEFAULTS = { PS: 'DuckStation', N64: 'RetroArch', SAT: 'Ymir (Sega Saturn)', GC: 'Dolphin', SNES: 'RetroArch', PS2: '(none)' };
var CORES = { N64: ['mupen64plus_next', 'parallel_n64'], SNES: ['snes9x', 'bsnes'] };
S.addTab = 'catalog'; S.catQ = ''; S.compatDefault = 'GE-Proton10-17'; S.umu = 'system';

function catRow(e, kind) {
  var busy = S.installing && S.installing[e.n];
  var btn = e.inst ? '<button class="small-btn" disabled>Installed</button>'
    : '<button class="small-btn primary" data-cat="' + kind + ':' + esc(e.n) + '"' + (busy ? ' disabled' : '') + '>' + (busy ? 'Installing…' : 'Install') + '</button>';
  return '<li class="cat-row"><div><span class="t">' + esc(e.n) + '</span><span class="s">' + esc(e.p) + ' • ' + esc(e.tag) + '</span></div>' + btn + '</li>';
}

function renderEmulators(){
  var panes = [['installed','Installed'],['catalog','Add from catalog'],['defaults','Platform defaults'],['compat','Compat tools']];
  rail('emu-rail', panes.map(function(p){ return { key: p[0], label: p[1], sel: S.emuPane === p[0] }; }), function(k){ S.emuPane = k; renderEmulators(); });
  var el = $('emu-pane'), h;
  if (S.emuPane === 'installed') {
    var emus = [['DuckStation','PS','~/Games/GRID/emulators/duckstation/DuckStation.AppImage','-fullscreen "%rom%"'],['Ymir (Sega Saturn)','SAT','~/Games/GRID/emulators/ymir/ymir-sdl3','-f "%rom%"'],['Dolphin','GC','~/Games/GRID/emulators/dolphin/dolphin-emu','-b -e "%rom%"']];
    h = '<h2>Installed</h2>' + emus.map(function(e){
      var st = S.fw[e[1]], act;
      if (st === 'installed') act = '<span class="small-btn ok"><span data-icon="check" data-size="14"></span>Installed</span>';
      else if (st === 'requested') act = '<button class="small-btn" disabled>Requested…</button>';
      else if (st === 'unavailable') act = '<button class="small-btn" data-fw="'+e[1]+'">Not needed</button>';
      else act = '<button class="small-btn primary" data-fw="'+e[1]+'">Install firmware</button>';
      return '<div class="emu"><div class="emu-top"><b>'+e[0]+'</b><code>'+esc(e[3])+'</code><div class="acts"><button class="small-btn" data-edit="'+esc(e[0])+'">Edit</button><button class="small-btn">Delete</button></div></div><div class="emu-top"><code>'+esc(e[2])+'</code></div>'+
        '<div class="fw">'+PLAT[e[1]]+' firmware · '+(e[1]==='GC'?'1 file':'3 files')+'<span class="acts">'+act+'</span></div></div>';
    }).join('');
  } else if (S.emuPane === 'catalog') {
    h = '<h2>Add from catalog</h2><div class="g-tabs" role="tablist"><button data-addtab="catalog" class="' + (S.addTab === 'catalog' ? 'active' : '') + '">Catalog</button><button data-addtab="manual" class="' + (S.addTab === 'manual' ? 'active' : '') + '">Manual</button></div>';
    if (S.addTab === 'catalog') {
      var q = S.catQ.trim().toLowerCase(), list = CATALOG.filter(function(e){ return !q || e.n.toLowerCase().indexOf(q) >= 0; });
      h += '<input class="g-search" style="flex:none" type="search" placeholder="Search emulators…" aria-label="Search emulators" id="cat-q" value="' + esc(S.catQ) + '">' +
        (list.length ? '<ul class="cat-list">' + list.map(function(e){ return catRow(e, 'emu'); }).join('') + '</ul>' : '<p class="note">No emulators found.</p>');
    } else {
      h += '<form class="form" onsubmit="return false"><label>Profile<select class="field"><option>Choose a profile to fill in the fields…</option><option>DuckStation</option><option>PCSX2</option></select></label>' +
        '<label>Name<input class="field" placeholder="PCSX2"></label><label>Executable path<span class="inline"><input class="field" placeholder="/path/to/emulator"><button class="g-btn secondary" type="button">Browse…</button></span></label>' +
        '<label>Arguments<input class="field" placeholder="-fullscreen &quot;%rom%&quot;"></label><label>Save strategy<select class="field"><option>auto</option><option>single_file</option><option>folder</option></select></label>' +
        '<label>Save directories<input class="field" placeholder="memcards;saves (semicolon-separated)"></label><div class="form-actions"><button class="g-btn" type="button" data-toast="Emulator saved">Save</button><button class="g-btn secondary" type="button" data-addtab="catalog">Cancel</button></div></form>';
    }
  } else if (S.emuPane === 'defaults') {
    var plats = Object.keys(PLAT).sort(function(a,b){ return PLAT[a].localeCompare(PLAT[b]); });
    h = '<div class="pane-head"><h2>Platform defaults</h2><button class="g-btn secondary" data-toast="Opens the first-run platform picker">Set up platforms…</button></div><ul class="defaults">' + plats.map(function(p){
      var opts = ['(none)', 'DuckStation', 'Dolphin', 'Ymir (Sega Saturn)', 'RetroArch'];
      var sel = '<select class="field" data-default="' + p + '">' + opts.map(function(o){ return '<option' + (DEFAULTS[p] === o ? ' selected' : '') + '>' + o + '</option>'; }).join('') + '</select>';
      var core = DEFAULTS[p] === 'RetroArch' ? '<div class="dfield"><span>Core</span><select class="field">' + (CORES[p] || []).map(function(c){ return '<option>' + c + '</option>'; }).join('') + (CORES[p] ? '' : '<option>No installed core</option>') + '</select></div>' : '';
      return '<li class="dcard"><b>' + PLAT[p] + '</b><div class="dfield"><span>Emulator</span>' + sel + '</div>' + core + '</li>';
    }).join('') + '</ul>';
  } else {
    var umuLabel = S.umu === 'system' ? 'umu-run: system · /usr/bin/umu-run' : S.umu === 'busy' ? 'umu-run: installing…' : 'umu-run: GRID-managed · 1.2.9';
    h = '<h2>Compat tools</h2><div class="sub"><h3>Compatibility tools</h3>' +
      '<div class="umu"><div class="umu-row"><span class="t">' + umuLabel + '</span><button class="small-btn" data-umu' + (S.umu === 'busy' ? ' disabled' : '') + '>' + (S.umu === 'busy' ? 'Installing…' : S.umu === 'system' ? 'Install' : 'Reinstall') + '</button></div>' +
      '<p class="note">Proton games run through umu-run. The first Proton launch downloads the Steam Linux Runtime (about 200 MB), so it takes longer than later ones.</p></div>' +
      '<h4>Default</h4><ul class="radios">' + ['GE-Proton10-17', 'Wine 10.0 (system)'].map(function(t){ return '<li><label><input type="radio" name="cd" data-compat="' + esc(t) + '"' + (S.compatDefault === t ? ' checked' : '') + '> ' + esc(t) + '</label></li>'; }).join('') + '</ul>' +
      '<h4>Install</h4><ul class="cat-list">' + COMPAT.map(function(e){ return catRow(e, 'compat'); }).join('') + '</ul></div>';
  }
  el.innerHTML = h;
  gIcons(el);
  var cq = $('cat-q'); if (cq) { cq.oninput = function(){ S.catQ = this.value; var pos = this.selectionStart; renderEmulators(); var n = $('cat-q'); n.focus(); n.setSelectionRange(pos, pos); }; }
}

function installCatalog(kind, name) {
  var list = kind === 'emu' ? CATALOG : COMPAT, e = list.filter(function(x){ return x.n === name; })[0];
  S.installing = S.installing || {}; S.installing[name] = true; toast('Installing ' + name + '…'); renderEmulators();
  setTimeout(function(){ e.inst = true; delete S.installing[name]; toast(name + ' installed'); renderEmulators(); }, 1800);
}

/* ---- settings ---- */
S.cloud = { restore: true, upload: true, skip: true, delay: 5, keep: 10 };
S.ra = { user: '', set: false };
S.connEdit = false; S.updateNotice = true;

function renderSettings(){
  var panes = [['connection','Connection'],['library','Library'],['cloud','Cloud saves'],['ra','RetroAchievements'],['updates','Updates'],['appearance','Appearance']];
  rail('set-rail', panes.map(function(p){ return { key: p[0], label: p[1], sel: S.setPane === p[0] }; }), function(k){ S.setPane = k; S.connEdit = false; renderSettings(); });
  var el = $('set-pane'), h;
  if (S.setPane === 'connection') {
    h = '<h2>Connection</h2><dl class="kv"><div><dt>Server</dt><dd>https://romm.home.lan</dd></div><div><dt>User</dt><dd>player</dd></div><div><dt>Credential</dt><dd>Stored in the OS keyring · session verified</dd></div><div><dt>Status</dt><dd><span class="g-status-dot online" style="display:inline-block;margin-right:6px"></span>Connected</dd></div></dl>';
    h += S.connEdit
      ? '<form class="form" onsubmit="return false"><label>Server URL<input class="field" value="https://romm.home.lan"></label><label>Username<input class="field" value="player"></label><label>Password<input class="field" type="password" placeholder="Not shown"></label><label class="check"><input type="checkbox"> Use API token</label><div class="form-actions"><button class="g-btn" type="button" data-conn="save">Connect</button><button class="g-btn secondary" type="button" data-conn="cancel">Cancel</button></div></form>'
      : '<div class="form-actions"><button class="g-btn" data-conn="edit">Change server…</button><button class="g-btn secondary" data-toast="Reconnected to romm.home.lan">Reconnect</button><button class="g-btn secondary">Disconnect</button></div>';
  } else if (S.setPane === 'library') {
    h = '<h2>Library</h2><div class="sub"><h3>Library folder</h3><p class="note">Games, emulators and saves live in this folder. New installs go here.</p><code class="path">/home/player/Games/GRID</code><div class="form-actions"><button class="g-btn" data-libchange>Change…</button></div></div>';
  } else if (S.setPane === 'cloud') {
    var c = S.cloud;
    h = '<h2>Cloud saves</h2><form class="form" onsubmit="return false">' +
      '<label class="check"><input type="checkbox" data-cloud="restore"' + (c.restore ? ' checked' : '') + '> Restore cloud saves before launch</label>' +
      '<label class="check"><input type="checkbox" data-cloud="upload"' + (c.upload ? ' checked' : '') + '> Upload cloud saves after exit</label>' +
      '<label class="check"><input type="checkbox" data-cloud="skip"' + (c.skip ? ' checked' : '') + '> Skip download when the local save is newer</label>' +
      '<label>Upload delay (seconds)<input class="field narrow" type="number" min="0" max="60" value="' + c.delay + '"></label>' +
      '<label>Save retention limit (0 = unlimited)<input class="field narrow" type="number" min="0" value="' + c.keep + '"></label>' +
      '<p class="note">Auto-sync uses the latest server save record only.</p><div class="form-actions"><button class="g-btn" type="button" data-toast="Cloud save settings saved">Save</button></div></form>';
  } else if (S.setPane === 'ra') {
    h = '<h2>RetroAchievements</h2><p class="note">' + (S.ra.set ? 'Set for ' + esc(S.ra.user) : 'Not set') + '</p><form class="form" onsubmit="return false">' +
      '<label>Username<input class="field" id="ra-user" value="' + esc(S.ra.user) + '" autocomplete="off"></label><label>Password<input class="field" type="password" autocomplete="off"></label><label>Token<input class="field" type="password" autocomplete="off"></label>' +
      '<div class="form-actions"><button class="g-btn secondary" type="button" data-ra="login">Log In</button><button class="g-btn" type="button" data-ra="save">Save</button><button class="g-btn secondary" type="button" data-ra="clear">Clear</button></div></form>';
  } else if (S.setPane === 'updates') {
    h = '<h2>Updates</h2><p class="line">GRID Launcher 0.9.10</p>' +
      (S.updateNotice ? '<p class="line update-line">GRID Launcher v0.9.11 is available <button class="g-btn" data-toast="Opens the release page in your browser">Open release</button><button class="g-btn secondary" data-dismiss-update>Dismiss</button></p>' : '<p class="line">Checked 2 hours ago — you have the latest release</p>') +
      '<p class="note">GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.</p>';
  } else {
    var theme = document.documentElement.getAttribute('data-theme') || 'light';
    h = '<h2>Appearance</h2>'+
      '<label class="setting"><span>Theme</span><span class="g-control"><select id="set-theme"><option value="light"'+(theme==='light'?' selected':'')+'>Light</option><option value="dark"'+(theme==='dark'?' selected':'')+'>Dark</option></select></span></label>'+
      '<label class="setting"><span>Background fade<small>0 turns the art off</small></span><span style="display:flex;align-items:center;gap:10px"><input type="range" min="0" max="60" step="10" value="'+S.fade+'" id="set-fade"><span class="g-counts">'+S.fade+'%</span></span></label>'+
      '<label class="setting"><span>Game labels</span><span class="g-switch" id="set-labels"><button data-l="always" class="'+(S.labels==='always'?'active':'')+'">Always visible</button><button data-l="hover" class="'+(S.labels==='hover'?'active':'')+'">Visible on hover</button></span></label>'+
      '<label class="setting"><span>Card size</span><span class="g-control"><select id="set-size"><option value="120"'+(S.cardW===120?' selected':'')+'>Small</option><option value="160"'+(S.cardW===160?' selected':'')+'>Medium</option><option value="200"'+(S.cardW===200?' selected':'')+'>Large</option></select></span></label>'+
      '<label class="setting"><span>Hardware acceleration<small>Applies after a restart</small></span><span class="check"><input type="checkbox" checked> Render on the graphics card</span></label>';
  }
  el.innerHTML = h;
  if (S.setPane === 'appearance') {
    $('set-theme').onchange = function(){ document.documentElement.setAttribute('data-theme', this.value); };
    $('set-fade').oninput = function(){ S.fade = +this.value; paintArt(G[S.lastViewed || 0]); renderSettings(); };
    $('set-labels').onclick = function(e){ var b = e.target.closest('[data-l]'); if (b) { e.preventDefault(); S.labels = b.getAttribute('data-l'); renderAll(); } };
    $('set-size').onchange = function(){ S.cardW = +this.value; syncSize(); renderAll(); };
  }
}

/* ---- library change dialog ---- */
function openLibChange(step) {
  S.libStep = step || 'pick';
  var d = $('libdlg'), h;
  if (S.libStep === 'pick') h = '<h3>Change library folder</h3><p class="note">Pick or type the new folder. It must be a full path, not inside the current folder.</p><label class="form"><span class="inline"><input class="field" value="/mnt/games/GRID"><button class="g-btn secondary" type="button">Browse…</button></span></label>' +
    '<div class="choice"><button class="g-btn" data-lib="move">Move existing files</button><button class="g-btn secondary" data-lib="fresh">Start fresh</button><button class="g-btn secondary" data-lib="close">Cancel</button></div>';
  else if (S.libStep === 'move') h = '<h3>Move existing files</h3><ul class="moves"><li><code>games/</code><span>Copied and checked</span></li><li><code>emulators/</code><span>Copied and checked</span></li><li><code>saves/</code><span>Copied and checked</span></li></ul><p class="line">Copies 48.2 GB; 1.3 TB free</p><p class="note">Installs, launches and cloud transfers wait while the move runs. Follow it in Downloads.</p><div class="choice"><button class="g-btn" data-lib="start">Start move</button><button class="g-btn secondary" data-lib="pick">Back</button></div>';
  else h = '<h3>Start fresh</h3><p class="line">Delete the old game files, or leave them on disk?</p><p class="note">Either way, games in the old folder leave the library. Emulators and saves are not touched.</p><div class="choice"><button class="g-btn secondary confirm" data-lib="delete">Delete…</button><button class="g-btn" data-lib="leave">Leave them</button><button class="g-btn secondary" data-lib="pick">Back</button></div>';
  d.innerHTML = h; $('libscrim').hidden = false;
}
function closeLibChange() { $('libscrim').hidden = true; }
