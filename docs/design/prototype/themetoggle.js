/* ==== Light / dark toggle: top-right on the startup screens, left of the
   connection dot and username in the top bar. Shows the current mode (sun or
   moon); a click switches, and the palette eases across instead of snapping.
   Settings › Appearance › Theme follows it. ==== */
function currentTheme() { return document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'light'; }
function paintToggles(spin) {
  var t = currentTheme(), next = t === 'dark' ? 'light' : 'dark';
  document.querySelectorAll('[data-theme-toggle]').forEach(function (b) {
    b.setAttribute('aria-label', 'Switch to ' + next + ' mode'); b.title = 'Switch to ' + next + ' mode';
    b.innerHTML = '<span class="tt-icon' + (spin ? ' spin' : '') + '">' + gIcon(t === 'dark' ? 'moon' : 'sun', 18) + '</span>';
  });
}
function setTheme(t) {
  var root = document.documentElement;
  if (!window.matchMedia || !matchMedia('(prefers-reduced-motion: reduce)').matches) {
    root.classList.add('theming'); clearTimeout(setTheme.t); setTheme.t = setTimeout(function () { root.classList.remove('theming'); }, 320);
  }
  root.setAttribute('data-theme', t);
  paintToggles(true);
  if (S.view === 'settings' && S.setPane === 'appearance') renderSettings();
}
document.addEventListener('click', function (e) { if (e.target.closest('[data-theme-toggle]')) setTheme(currentTheme() === 'dark' ? 'light' : 'dark'); });
/* The Appearance select changes it too: keep the icons in step. */
document.addEventListener('change', function (e) { if (e.target.id === 'set-theme') { setTheme(e.target.value); } });
/* Only the startup screens show the corner toggle. */
var ttShow = freshShow; freshShow = function (p) { ttShow(p); $('theme-start').hidden = false; };
var ttHide = freshHide; freshHide = function () { ttHide(); $('theme-start').hidden = true; };
setTimeout(function () { paintToggles(false); }, 0);
