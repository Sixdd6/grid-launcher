"""Writes the TV mode prototype card: python3 -I docs/design/prototype/tvmode.py docs/design/system/components"""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gen  # noqa: E402  (writes the small previews too; reuse its icon helper)

CSS = open(os.path.join(HERE, 'tvmode.css')).read()
JS = open(os.path.join(HERE, 'tvmode.js')).read()
HTML = f"""<!-- @dsCard group="Prototype" height=800 width=1280 subtitle="TV mode: the fullscreen, controller-first GRID. Arrows, Enter = A, Esc = B, X, Y, Q/E = LB/RB, Z = LT, M = Menu, V = View; a real controller works too" -->
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>TV mode</title>
<style>
{CSS}
</style></head>
<body>
<div class="tv">
  <div class="tv-art" id="art"></div>
  <header class="top" id="top"></header>
  <main class="stage" id="stage"></main>
  <footer class="hints" id="hints"></footer>
  <div id="layers"></div>
  <div class="toasts" id="toasts" aria-live="polite"></div>
  <div class="desk" id="desk" hidden></div>
</div>
{gen.ICON_JS}
<script>
{JS}
</script>
</body>
</html>
"""
d = os.path.join(gen.OUT, 'TVMode')
os.makedirs(d, exist_ok=True)
open(os.path.join(d, 'preview.html'), 'w').write(HTML)
