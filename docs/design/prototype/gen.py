import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1]  # .../ds/project/components
icons = json.load(open(os.path.join(HERE, 'icons.json')))

ICON_JS = """<script>
(function(){
var I=%s, F=['star','play','more'];
window.gIcon=function(n,s){s=s||14;var f=F.indexOf(n)>=0;
return '<svg class="g-icon" viewBox="0 0 24 24" width="'+s+'" height="'+s+'" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="'+I[n]+'" fill="'+(f?'currentColor':'none')+'" stroke="'+(f?'none':'currentColor')+'"/></svg>';};
window.gIcons=function(root){(root||document).querySelectorAll('[data-icon]').forEach(function(el){el.innerHTML=gIcon(el.getAttribute('data-icon'),+el.getAttribute('data-size')||14);});};
gIcons();
})();
</script>""" % json.dumps(icons)


def page(marker, title, body, style='', script=''):
    return f"""{marker}
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>{title}</title>
<style>
  body {{ padding: 16px; background: var(--bg); }}
  .row {{ display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }}
  .col {{ display: flex; flex-direction: column; gap: 10px; }}
  .cap {{ font-size: 11px; color: var(--text-muted); margin: 0 0 6px; }}
{style}
</style></head>
<body>
{body}
{ICON_JS}
{script}
</body>
</html>
"""


def write(name, text):
    d = os.path.join(OUT, name)
    os.makedirs(d, exist_ok=True)
    open(os.path.join(d, 'preview.html'), 'w').write(text)


# Placeholder cover art: invented titles on flat fills (no real game art).
GAMES = [
    ("Starfall Circuit", "hsl(258 45% 38%)", "PS"), ("Lantern Vale", "hsl(32 70% 42%)", "N64"),
    ("Ironclad Drift", "hsl(205 55% 34%)", "SAT"), ("Moss & Marrow", "hsl(140 35% 30%)", "GC"),
    ("Pixel Harbor", "hsl(350 55% 42%)", "SNES"), ("Cinder Peaks", "hsl(15 60% 40%)", "PS"),
    ("Echo Garden", "hsl(175 45% 30%)", "N64"), ("Velvet Static", "hsl(290 35% 36%)", "PS2"),
    ("Tidewalker", "hsl(195 60% 32%)", "GC"), ("Brass Comet", "hsl(45 65% 36%)", "SAT"),
    ("Hollow Signal", "hsl(220 30% 28%)", "PS"), ("Kite Season", "hsl(95 40% 34%)", "SNES"),
    ("Neon Orchard", "hsl(320 50% 40%)", "PS2"), ("Glass Monsoon", "hsl(185 35% 38%)", "N64"),
    ("Quiet Engine", "hsl(0 0% 30%)", "SAT"), ("Ember Choir", "hsl(8 65% 38%)", "GC"),
]

COVER_CSS = """
  .g-art::before { content:''; position:absolute; right:-18%; top:12%; width:70%; aspect-ratio:1; border-radius:50%; background:rgba(255,255,255,.12); }
  .g-art::after { content:''; position:absolute; left:10%; top:8%; width:34%; height:6px; border-radius:3px; background:rgba(255,255,255,.35); }
  .g-art { padding-bottom: 34px; }
  .g-art span { position:relative; text-shadow:0 1px 2px rgba(0,0,0,.35); }
"""

# ---------------- Button
write('Button', page('<!-- @dsCard group="Actions" height=170 -->', 'Button', """
<div class="col">
  <p class="cap">Primary · Secondary · Destructive · Disabled</p>
  <div class="row">
    <button class="g-btn">Play</button>
    <button class="g-btn secondary">Game Settings</button>
    <button class="g-btn secondary destructive">Remove from library (keeps files)</button>
    <button class="g-btn" disabled>Installing…</button>
  </div>
  <p class="cap">Two-click confirm states</p>
  <div class="row">
    <button class="g-btn secondary confirm">Confirm uninstall</button>
    <button class="g-btn confirm">Confirm</button>
    <button class="g-btn update-confirm">Saves and configuration will be preserved — confirm update</button>
  </div>
</div>"""))

# ---------------- IconButton
write('IconButton', page('<!-- @dsCard group="Actions" height=96 -->', 'Icon button', """
<div class="row">
  <button class="g-icon-btn" aria-label="Close"><span data-icon="close" data-size="20"></span></button>
  <button class="g-icon-btn" aria-label="Back"><span data-icon="arrowLeft" data-size="20"></span></button>
  <button class="g-icon-btn" aria-label="Previous"><span data-icon="chevronLeft" data-size="20"></span></button>
  <button class="g-icon-btn" aria-label="Next"><span data-icon="chevronRight" data-size="20"></span></button>
  <button class="g-icon-btn" aria-label="Dismiss"><span data-icon="close" data-size="14"></span></button>
  <span class="cap" style="margin:0">28×28 target · icon 20 or 14 · hover tints to primary</span>
</div>"""))

# ---------------- TopBar
write('TopBar', page('<!-- @dsCard group="Navigation" height=110 width=1100 -->', 'Top bar', """
<header class="g-topbar" style="border:1px solid var(--line)">
  <div class="g-brand"><span class="g-logo" data-icon="grid" data-size="20"></span><span class="g-wordmark">GRID</span></div>
  <nav class="g-pills" aria-label="Views">
    <button class="g-pill">Library</button><button class="g-pill active" aria-current="page">Server</button>
    <button class="g-pill">Downloads</button><button class="g-pill">Emulators</button><button class="g-pill">Settings</button>
  </nav>
  <div class="g-session">
    <button class="g-update-badge">Update</button>
    <span class="g-status-dot online" aria-hidden="true"></span>
    <button class="g-session-chip">player@romm.home.lan</button>
  </div>
</header>""", style="body{padding:16px 0}"))

# ---------------- Menu
write('Menu', page('<!-- @dsCard group="Overlays" height=200 -->', 'Server menu', """
<div class="g-menu" role="menu" style="width:220px">
  <button role="menuitem">Reconnect</button>
  <button role="menuitem">Disconnect</button>
  <button role="menuitem">Open RomM in browser</button>
  <span class="g-menu-host" role="none">romm.home.lan</span>
</div>"""))

# ---------------- RailPane
write('RailPane', page('<!-- @dsCard group="Navigation" height=380 -->', 'Rail', """
<nav class="g-rail" aria-label="Library filters" style="height:340px">
  <button class="g-rail-item active" aria-current="page"><span class="g-rail-label">All games</span><span class="g-rail-count">42</span></button>
  <button class="g-rail-item"><span class="g-rail-label">Recent</span><span class="g-rail-count">7</span></button>
  <button class="g-rail-item"><span class="g-rail-label">Updates</span><span class="g-rail-count">2</span></button>
  <span class="g-rail-heading">PLATFORMS</span>
  <button class="g-rail-item"><span class="g-rail-label">GameCube</span><span class="g-rail-count">6</span></button>
  <button class="g-rail-item"><span class="g-rail-label">Nintendo 64</span><span class="g-rail-count">11</span></button>
  <button class="g-rail-item"><span class="g-rail-label">PlayStation</span><span class="g-rail-count">14</span></button>
  <button class="g-rail-item"><span class="g-rail-label">Sega Saturn</span><span class="g-rail-count">5</span></button>
</nav>"""))

# ---------------- Tabs
write('Tabs', page('<!-- @dsCard group="Navigation" height=90 -->', 'Tabs', """
<div class="g-tabs" role="tablist" style="max-width:560px">
  <button role="tab" class="active" aria-selected="true">Overview</button>
  <button role="tab">Media</button><button role="tab">Saves</button><button role="tab">Achievements</button>
</div>"""))

# ---------------- SegmentedSwitch
write('SegmentedSwitch', page('<!-- @dsCard group="Navigation" height=80 -->', 'Segmented switch', """
<div class="g-switch" role="tablist" aria-label="Cloud saves or states">
  <button role="tab" class="active" aria-selected="true">Saves</button>
  <button role="tab">States</button>
</div>""", script="""<script>
document.querySelectorAll('.g-switch button').forEach(function(b){b.onclick=function(){
document.querySelectorAll('.g-switch button').forEach(function(x){x.classList.toggle('active',x===b);x.setAttribute('aria-selected',x===b);});};});
</script>"""))

# ---------------- Toolbar
write('Toolbar', page('<!-- @dsCard group="Forms" height=170 width=900 -->', 'View header and toolbar', """
<div class="g-panel g-head">
  <header class="g-platform-header">
    <h2>PlayStation</h2>
    <p class="g-counts">48 games · 6 installed</p>
    <div class="g-chips">
      <button class="g-chip-outline">Firmware: 3 files · installed</button>
      <button class="g-chip-outline">Emulator: DuckStation</button>
    </div>
  </header>
  <div class="g-toolbar">
    <input class="g-search" type="search" placeholder="Search title, platform or genre" aria-label="Search this platform">
    <label class="g-control"><span>Size</span><select><option>Small</option><option selected>Medium</option><option>Large</option></select></label>
  </div>
</div>"""))

# ---------------- Chip
write('Chip', page('<!-- @dsCard group="Content" height=120 -->', 'Chips', """
<div class="col">
  <p class="cap">Status chips (Details header)</p>
  <div class="row">
    <span class="g-chip playing">Playing</span><span class="g-chip">Identified</span>
    <span class="g-chip">Verified · Hack</span><span class="g-chip">v1.1</span>
    <span class="g-chip status">Not linked to a server game</span><span class="g-chip status missing">Files not found</span>
  </div>
  <p class="cap">Outline chips (view header links)</p>
  <div class="row"><button class="g-chip-outline">Firmware: 3 files</button><button class="g-chip-outline">No default emulator</button></div>
</div>"""))

# ---------------- GameCard
cards = []
states = [
    (0, dict(installed=True, cloud=True)),
    (1, dict(update=True, installed=True)),
    (2, dict(missing=True, installed=True, platform=True)),
    (3, dict()),
]
def card_html(i, st, focused=False, label=True):
    t, c, p = GAMES[i]
    tags = ''
    if st.get('missing') or st.get('update'):
        tags = '<div class="g-tag-stack">' + ('<span class="g-tag missing">MISSING</span>' if st.get('missing') else '') + ('<span class="g-tag update">UPDATE</span>' if st.get('update') else '') + '</div>'
    dot = '<span class="g-dot" role="img" aria-label="Installed"></span>' if st.get('installed') else ''
    plat = f'<span class="g-tag platform g-corner">{p}</span>' if st.get('platform') else ''
    cloud = '<span class="g-cloud-badge g-corner" role="img" aria-label="Cloud saves enabled" data-icon="cloud" data-size="14"></span>' if st.get('cloud') else ''
    prim = 'Details' if st.get('missing') else ('Play' if st.get('installed') else 'Install')
    lab = f'<span class="g-label" title="{t}">{t}</span>' if label else ''
    return f'''<div class="g-card{' focused' if focused else ''}">
  <div class="g-cover"><div class="g-art" style="background:{c}"><span>{t}</span></div>{tags}{dot}{plat}{cloud}
    <div class="g-shade" aria-hidden="true"></div>
    <div class="g-bar"><button class="g-card-primary">{prim}</button><button class="g-round" aria-label="Cloud sync" data-icon="cloud" data-size="15"></button><button class="g-round" aria-label="Details" data-icon="more" data-size="15"></button></div>
  </div>{lab}
</div>'''
body = '<p class="cap">Hover a card: it lifts, the action bar shows, and the full title wraps. Covers are placeholders.</p><div class="cards">' + ''.join(card_html(i, st, focused=(i == 3)) for i, st in states) + '</div>'
write('GameCard', page('<!-- @dsCard group="Content" height=340 -->', 'Game card', body, style=COVER_CSS + """
  .cards { display:grid; grid-template-columns:repeat(4,160px); gap:16px 14px; padding-bottom:24px; }"""))

# ---------------- FactsPanel
write('FactsPanel', page('<!-- @dsCard group="Content" height=330 width=760 -->', 'Overview', """
<div style="display:flex;gap:20px 28px;align-items:flex-start;flex-wrap:wrap">
  <div class="g-description" style="flex:1 1 340px">
    <p>A courier pilot races a failing orbital ring, one district at a time, before the lights go out for good.</p>
    <p>Long descriptions break into paragraphs and cap at about 70 characters a line, so the server's single block of text stays readable.</p>
  </div>
  <dl class="g-facts" style="flex:0 1 220px">
    <div><dt>Released</dt><dd>14 March 1998</dd></div>
    <div><dt>Genres</dt><dd>Racing, Action</dd></div>
    <div><dt>Companies</dt><dd>Northwind Interactive</dd></div>
    <div><dt>Size</dt><dd>612 MB</dd></div>
    <div><dt>Version</dt><dd>v1.1 installed · v1.2 on server</dd></div>
  </dl>
</div>"""))

# ---------------- Toast
write('Toast', page('<!-- @dsCard group="Feedback" height=140 -->', 'Toast', """
<div class="col" style="align-items:center">
  <p class="g-toast">Starfall Circuit — Cloud save uploaded</p>
  <p class="g-toast error">Lantern Vale — Upload failed: the server did not answer</p>
</div>"""))

# ---------------- DownloadStrip
write('DownloadStrip', page('<!-- @dsCard group="Feedback" height=80 width=1000 -->', 'Download strip', """
<footer class="g-strip" role="button" tabindex="0" style="border:1px solid var(--border)">
  <span class="g-strip-lead" data-icon="download" data-size="14"></span>
  <span class="g-strip-line">Starfall Circuit · 64% · 38.2 MB/s</span>
  <svg width="120" height="18" viewBox="0 0 120 18" aria-label="Transfer rate"><polyline fill="none" stroke="var(--primary)" stroke-width="1.5" points="0,14 10,12 20,9 30,10 40,6 50,7 60,5 70,8 80,4 90,6 100,3 110,5 120,4"/><polyline fill="none" stroke="var(--graph-disk)" stroke-width="1.5" points="0,16 10,15 20,13 30,14 40,11 50,12 60,10 70,12 80,9 90,11 100,8 110,10 120,9"/></svg>
  <span class="g-strip-open">Open Downloads</span>
</footer>""", style="body{padding:16px 0}"))

# ---------------- DetailsDialog
write('DetailsDialog', page('<!-- @dsCard group="Overlays" height=560 width=1040 -->', 'Details', f"""
<div class="g-dialog" role="dialog" aria-label="Starfall Circuit" style="height:520px">
  <button class="g-icon-btn g-dialog-close" aria-label="Close"><span data-icon="close" data-size="20"></span></button>
  <div class="g-details">
    <aside class="g-details-left">
      <div class="g-cover" style="box-shadow:var(--shadow-lift)"><div class="g-art" style="background:{GAMES[0][1]};font-size:20px"><span>Starfall Circuit</span></div></div>
      <button class="g-btn">Play</button>
      <button class="g-btn secondary">Uninstall</button>
      <p class="g-meta-line">Last played 2 days ago</p>
      <p class="g-meta-line">DuckStation</p>
    </aside>
    <section class="g-details-right">
      <header class="g-details-head">
        <h2>Starfall Circuit</h2>
        <p class="g-header-line">PlayStation · 1998 · Racing · <span class="g-rating"><span data-icon="star" data-size="14"></span>8.4</span></p>
        <div class="row" style="gap:6px"><span class="g-chip">Identified</span><span class="g-chip">v1.1</span></div>
      </header>
      <div class="g-tabs" role="tablist"><button class="active">Overview</button><button>Media</button><button>Saves</button><button>Achievements</button></div>
      <div class="g-description"><p>A courier pilot races a failing orbital ring, one district at a time, before the lights go out for good.</p></div>
    </section>
  </div>
</div>""", style=COVER_CSS))

print('previews written')
