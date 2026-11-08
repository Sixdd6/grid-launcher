/* ==== Grid headers (Server, Library): each folds into its toolbar row once
   its grid scrolls, and unfolds near the top. The header floats over the grid
   (the scroller reserves its unfolded height as top padding), so folding never
   moves a card. One behaviour, two instances. ==== */
var FOLD_AT = 32, UNFOLD_AT = 8;
function makeFold(o) {
  var f = { folded: false, key: null };
  var head = function () { return $(o.head); }, scroll = function () { return $(o.scroll); };
  f.measure = function () {
    var sc = scroll(), hd = head();
    if (!sc || !hd || f.folded || !hd.offsetHeight) return;
    sc.style.setProperty('--head-h', hd.offsetHeight + 'px');
    $(o.body).style.setProperty('--sb', (sc.offsetWidth - sc.clientWidth) + 'px');
  };
  f.set = function (folded) {
    if (f.folded === folded) return;
    f.folded = folded;
    head().classList.toggle('folded', folded);
    head().querySelector('.g-search').placeholder = folded ? o.foldedPlaceholder() : o.placeholder;
  };
  f.onScroll = function () {
    var y = scroll().scrollTop;
    if (!f.folded && y > FOLD_AT) f.set(true);
    else if (f.folded && y < UNFOLD_AT) f.set(false);
  };
  /* After each render: a new system or filter starts at the top, unfolded. */
  f.sync = function () {
    var k = o.key();
    if (f.key !== k) { f.key = k; scroll().scrollTop = 0; f.set(false); }
    else if (f.folded) head().querySelector('.g-search').placeholder = o.foldedPlaceholder();
    requestAnimationFrame(f.measure);
  };
  setTimeout(function () { scroll().addEventListener('scroll', f.onScroll, { passive: true }); f.measure(); }, 0);
  window.addEventListener('resize', function () { requestAnimationFrame(f.measure); });
  return f;
}

var SRV_FOLD = makeFold({ body: 'srv-body', head: 'srv-head', scroll: 'srv-scroll',
  placeholder: 'Search title, platform or genre',
  foldedPlaceholder: function () { return 'Search ' + PLAT[S.platform] + '…'; },
  key: function () { return S.platform; } });
var LIB_FOLD = makeFold({ body: 'lib-body', head: 'lib-head', scroll: 'lib-scroll',
  placeholder: 'Search by title',
  foldedPlaceholder: function () { var f = S.libFilter; return f === 'all' ? 'Search your library…' : f === 'recent' ? 'Search recent games…' : f === 'updates' ? 'Search games with updates…' : 'Search ' + PLAT[f] + '…'; },
  key: function () { return S.libFilter; } });

var foldRenderServer = renderServer;
renderServer = function () { foldRenderServer(); SRV_FOLD.sync(); };
var foldRenderLibrary = renderLibrary;
renderLibrary = function () { foldRenderLibrary(); LIB_FOLD.sync(); };
