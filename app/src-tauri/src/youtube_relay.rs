//! The YouTube relay page: what lets a trailer play inside GRID.
//!
//! An `<iframe>` to YouTube straight from the app page never plays on Linux
//! or macOS: the page origin is `tauri://localhost`, a "local scheme" under
//! the W3C referrer policy, so no `Referer` is ever sent and YouTube answers
//! error 153 for every embed (tauri-apps/tauri#14422). A page served from the
//! loopback media server (`http://127.0.0.1:<port>`) has a real http origin,
//! and YouTube accepts it (probed 2026-11-10: a 127.0.0.1 page with
//! `YT.Player` reached READY and played). So the app frames this page, and
//! this page frames YouTube's player.
//!
//! The page is pure output of [`relay_page`]; nothing a request carries
//! reaches it except a video id that passed [`video_id`] and a volume parsed
//! as a number. It gets no IPC (the media server is a plain HTTP origin, not
//! a Tauri one, and no capability names it) and sends its own strict CSP
//! ([`relay_csp`]), with a fresh script nonce per response.
//!
//! The protocol with the app (`app/src/lib/details/youtubeRelay.ts`) is
//! postMessage only, and both ends check the other's origin and window:
//!
//! * relay → app: `{source:'grid-yt', type:'boot'}` until the app says hello,
//!   then `linked`, `ready`, `state` (`data` = the YT player state),
//!   `volume` (`value` 0–100, `muted`) and `error` (`code`: the YT error
//!   number, or `"load"` when the player API script did not load);
//! * app → relay: `{source:'grid-yt', cmd}` with `hello`, `play`, `pause`,
//!   `toggle`, `seek` (`by` seconds) and `volume` (`value` 0–100).
//!
//! The relay loads YouTube's player only after the app's hello. If messages
//! cannot pass in one direction, the player never loads and the app falls
//! back to the poster and "Watch on YouTube" on its ready timeout, rather
//! than showing a player the controller cannot drive.

/// The app page origins the relay talks to: Linux and macOS
/// (`tauri://localhost`), Windows (`http://tauri.localhost`, or `https://`
/// with `useHttpsScheme`), and the Vite dev server in a debug build.
#[cfg(not(debug_assertions))]
pub const PARENT_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];
#[cfg(debug_assertions)]
pub const PARENT_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "http://localhost:5173",
];

/// YouTube's player API script, which the relay loads after the app's hello.
pub const PLAYER_API: &str = "https://www.youtube.com/iframe_api";

/// The player API script the relay loads: [`PLAYER_API`], except in an `e2e`
/// build with `GRID_LAUNCHER_E2E_YOUTUBE_API` set to a loopback URL
/// (`http://127.0.0.1:<port>/<path>`), which the end-to-end suite points at a
/// dead port so no run ever reaches YouTube and the fallback is exercised
/// the same way on every machine. Anything else in the variable is ignored.
pub fn player_api() -> String {
    #[cfg(feature = "e2e")]
    if let Ok(url) = std::env::var("GRID_LAUNCHER_E2E_YOUTUBE_API") {
        if loopback_script(&url).is_some() {
            return url;
        }
    }
    PLAYER_API.to_string()
}

/// `http://127.0.0.1:<port>` for a plain loopback script URL, `None` for
/// anything else (or any character that would need escaping in the page or
/// the CSP).
fn loopback_script(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("http://127.0.0.1:")?;
    let port_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if port_len == 0 || !rest[port_len..].starts_with('/') {
        return None;
    }
    if !url
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"/:._-".contains(&b))
    {
        return None;
    }
    Some(&url[.."http://127.0.0.1:".len() + port_len])
}

/// The path segment the media server answers with the relay page, after its
/// nonce: `/<nonce>/yt.html?v=<id>&vol=<0-100>`.
pub const RELAY_NAME: &str = "yt.html";

/// A YouTube video id: exactly 11 `[A-Za-z0-9_-]` characters, YouTube's fixed
/// id format, after trimming surrounding whitespace. Anything else — a path,
/// an id with a query string tacked on, a full URL — is refused, so nothing
/// but a bare id is ever interpolated into a URL or into the relay page.
pub fn video_id(id: &str) -> Option<&str> {
    let trimmed = id.trim();
    let valid = trimmed.len() == 11
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    valid.then_some(trimmed)
}

/// The relay request's query, `v=<id>&vol=<0-100>`, in either order: the id
/// must pass [`video_id`] (no percent-decoding: a valid id has nothing to
/// decode), the volume must be a whole number and is clamped to 100; a
/// missing volume is 50. Any other key, a repeated key or a malformed pair
/// refuses the whole request.
pub fn parse_relay_query(query: &str) -> Option<(String, u8)> {
    let mut id: Option<String> = None;
    let mut volume: Option<u8> = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "v" if id.is_none() => {
                // `video_id` trims; the query must already be exact.
                if value.trim() != value {
                    return None;
                }
                id = Some(video_id(value)?.to_string());
            }
            "vol" if volume.is_none() => {
                if value.is_empty() || value.len() > 3 || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return None;
                }
                volume = Some(value.parse::<u16>().ok()?.min(100) as u8);
            }
            _ => return None,
        }
    }
    Some((id?, volume.unwrap_or(50)))
}

/// The relay page's `Content-Security-Policy`: nothing by default; scripts
/// only from this response's nonce and YouTube's player hosts; frames only
/// YouTube's player; images only YouTube's thumbnail host; inline styles
/// (the page's own and the ones YouTube's API sets on its iframe). No
/// `connect-src`, no forms, no `<base>`.
///
/// `api` is the player API script ([`player_api`]); an e2e build's loopback
/// override adds its own origin to `script-src`, and nothing else does.
pub fn relay_csp(script_nonce: &str, api: &str) -> String {
    let extra = match loopback_script(api) {
        Some(origin) => format!(" {origin}"),
        None => String::new(),
    };
    format!(
        "default-src 'none'; \
         script-src 'nonce-{script_nonce}' https://www.youtube.com https://s.ytimg.com{extra}; \
         frame-src https://www.youtube-nocookie.com https://www.youtube.com; \
         img-src https://i.ytimg.com; \
         style-src 'unsafe-inline'; \
         base-uri 'none'; \
         form-action 'none'"
    )
}

/// The relay page for one trailer. `id` must have passed [`video_id`] (it is
/// checked again here, and an invalid one builds no page); `volume` is the
/// level the player opens at, clamped to 100; `script_nonce` is this
/// response's CSP nonce (hex, so it needs no escaping); `api` is the player
/// API script, [`PLAYER_API`] or a plain loopback URL ([`player_api`]).
pub fn relay_page(id: &str, volume: u8, script_nonce: &str, api: &str) -> Option<String> {
    let id = video_id(id)?;
    if script_nonce.is_empty() || !script_nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if api != PLAYER_API && loopback_script(api).is_none() {
        return None;
    }
    let volume = volume.min(100);
    let parents = PARENT_ORIGINS
        .iter()
        .map(|o| format!("'{o}'"))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="referrer" content="strict-origin-when-cross-origin">
<title>Trailer</title>
<style>html,body{{margin:0;height:100%;background:#000;overflow:hidden}}#p{{width:100%;height:100%;border:0}}</style>
</head>
<body>
<div id="p"></div>
<script nonce="{script_nonce}">
(function () {{
  'use strict';
  var VIDEO = '{id}';
  var VOLUME = {volume};
  var PARENTS = [{parents}];
  var SOURCE = 'grid-yt';
  var started = false, ready = false, player = null, state = -1;
  var lastVolume = -1, lastMuted = null, poll = null, boots = 0, booting = null;

  function post(msg) {{
    if (window.parent === window) return;
    msg.source = SOURCE;
    // Only the app page's own origin receives it: a target origin that
    // does not match the parent drops the message.
    for (var i = 0; i < PARENTS.length; i++) {{
      try {{ window.parent.postMessage(msg, PARENTS[i]); }} catch (e) {{}}
    }}
  }}

  function reportVolume() {{
    if (!ready) return;
    var v = player.getVolume(), m = player.isMuted();
    if (v === lastVolume && m === lastMuted) return;
    lastVolume = v; lastMuted = m;
    post({{ type: 'volume', value: v, muted: m }});
  }}

  function start() {{
    started = true;
    if (booting !== null) {{ clearInterval(booting); booting = null; }}
    window.onYouTubeIframeAPIReady = function () {{
      player = new YT.Player('p', {{
        host: 'https://www.youtube-nocookie.com',
        videoId: VIDEO,
        width: '100%',
        height: '100%',
        playerVars: {{ autoplay: 1, playsinline: 1, rel: 0, enablejsapi: 1, fs: 1, origin: location.origin }},
        events: {{
          onReady: function () {{
            ready = true;
            player.setVolume(VOLUME);
            if (VOLUME > 0) player.unMute();
            lastVolume = VOLUME; lastMuted = player.isMuted();
            post({{ type: 'ready' }});
            player.playVideo();
          }},
          onStateChange: function (e) {{
            state = e.data;
            post({{ type: 'state', data: e.data }});
            reportVolume();
            if (state === 1 && poll === null) poll = setInterval(reportVolume, 1000);
            if (state !== 1 && poll !== null) {{ clearInterval(poll); poll = null; }}
          }},
          onError: function (e) {{ post({{ type: 'error', code: e.data }}); }}
        }}
      }});
    }};
    var s = document.createElement('script');
    s.nonce = '{script_nonce}';
    s.src = '{api}';
    s.onerror = function () {{ post({{ type: 'error', code: 'load' }}); }};
    document.head.appendChild(s);
  }}

  function onCommand(d) {{
    if (d.cmd === 'hello') {{
      if (!started) start();
      post({{ type: 'linked' }});
      return;
    }}
    if (!ready) return;
    if (d.cmd === 'play') player.playVideo();
    else if (d.cmd === 'pause') player.pauseVideo();
    else if (d.cmd === 'toggle') {{
      if (state === 1 || state === 3) player.pauseVideo(); else player.playVideo();
    }} else if (d.cmd === 'seek') {{
      var by = Number(d.by);
      if (!isFinite(by)) return;
      var end = player.getDuration();
      var t = Math.max(0, player.getCurrentTime() + by);
      if (end > 0) t = Math.min(end, t);
      player.seekTo(t, true);
    }} else if (d.cmd === 'volume') {{
      var v = Number(d.value);
      if (!isFinite(v)) return;
      v = Math.max(0, Math.min(100, Math.round(v)));
      player.setVolume(v);
      if (v > 0 && player.isMuted()) player.unMute();
      lastVolume = v; lastMuted = player.isMuted();
    }}
  }}

  window.addEventListener('message', function (e) {{
    if (e.source !== window.parent || PARENTS.indexOf(e.origin) < 0) return;
    var d = e.data;
    if (!d || typeof d !== 'object' || d.source !== SOURCE || typeof d.cmd !== 'string') return;
    onCommand(d);
  }});

  // Until the app answers, say the relay is here (its listener may attach
  // a moment after this page runs). The player loads only after hello.
  post({{ type: 'boot' }});
  booting = setInterval(function () {{
    if (started || ++boots > 40) {{ clearInterval(booting); booting = null; return; }}
    post({{ type: 'boot' }});
  }}, 250);
}})();
</script>
</body>
</html>
"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_eleven_character_id_passes() {
        assert_eq!(video_id("dQw4w9WgXcQ"), Some("dQw4w9WgXcQ"));
        assert_eq!(video_id("a-b_c-D_E9z"), Some("a-b_c-D_E9z"));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(video_id("  dQw4w9WgXcQ\n"), Some("dQw4w9WgXcQ"));
    }

    #[test]
    fn anything_but_a_bare_id_is_refused() {
        for bad in [
            "",
            "dQw4w9WgXc",
            "dQw4w9WgXcQQ",
            "../evil/xxx",
            "dQw4w9WgXcQ&list=x",
            "dQw4w9WgX'Q",
            "dQw4w9WgX<Q",
            "dQw4w9WgX\"Q",
            "dQw4w9 gXcQ",
            "dQw4w9WgXcé",
            "https://youtu.be/dQw4w9WgXcQ",
        ] {
            assert_eq!(video_id(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_query_takes_an_id_and_a_volume_in_either_order() {
        assert_eq!(
            parse_relay_query("v=dQw4w9WgXcQ&vol=80"),
            Some(("dQw4w9WgXcQ".to_string(), 80))
        );
        assert_eq!(
            parse_relay_query("vol=0&v=dQw4w9WgXcQ"),
            Some(("dQw4w9WgXcQ".to_string(), 0))
        );
        assert_eq!(
            parse_relay_query("v=dQw4w9WgXcQ"),
            Some(("dQw4w9WgXcQ".to_string(), 50))
        );
        assert_eq!(
            parse_relay_query("v=dQw4w9WgXcQ&vol=250"),
            Some(("dQw4w9WgXcQ".to_string(), 100))
        );
    }

    #[test]
    fn a_bad_query_is_refused_whole() {
        for bad in [
            "",
            "vol=50",
            "v=bad",
            "v=dQw4w9WgXcQ&v=dQw4w9WgXcQ",
            "v=dQw4w9WgXcQ&vol=50&vol=60",
            "v=dQw4w9WgXcQ&vol=-1",
            "v=dQw4w9WgXcQ&vol=5000",
            "v=dQw4w9WgXcQ&vol=",
            "v=dQw4w9WgXcQ&x=1",
            "v=dQw4w9WgXcQ&vol",
            "v=%20dQw4w9WgXcQ",
            "v= dQw4w9WgXcQ",
            "v=dQw4w9WgXcQ%27",
        ] {
            assert_eq!(parse_relay_query(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_csp_allows_only_the_nonce_and_youtubes_player_hosts() {
        let csp = relay_csp("abc123", PLAYER_API);
        assert!(csp.starts_with("default-src 'none';"), "{csp}");
        assert!(
            csp.contains("script-src 'nonce-abc123' https://www.youtube.com https://s.ytimg.com;")
        );
        assert!(csp.contains("frame-src https://www.youtube-nocookie.com https://www.youtube.com;"));
        assert!(csp.contains("img-src https://i.ytimg.com;"));
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("form-action 'none'"));
        assert!(!csp.contains("unsafe-eval"));
        assert!(!csp.contains("connect-src"));
        assert!(!csp.contains('*'));
        // Inline scripts run only with the nonce: no 'unsafe-inline' for
        // scripts (styles alone may be inline).
        let script = csp
            .split(';')
            .find(|d| d.trim().starts_with("script-src"))
            .unwrap();
        assert!(!script.contains("unsafe-inline"));
    }

    #[test]
    fn the_page_carries_the_id_volume_nonce_and_parent_origins() {
        let page = relay_page("dQw4w9WgXcQ", 70, "0123abcd", PLAYER_API).unwrap();
        assert!(page.contains("var VIDEO = 'dQw4w9WgXcQ';"));
        assert!(page.contains("var VOLUME = 70;"));
        assert!(page.contains(r#"<script nonce="0123abcd">"#));
        assert!(page.contains("s.nonce = '0123abcd';"));
        assert!(page.contains("'tauri://localhost'"));
        assert!(page.contains("'http://tauri.localhost'"));
        assert!(
            page.contains(r#"<meta name="referrer" content="strict-origin-when-cross-origin">"#)
        );
        assert!(page.contains("host: 'https://www.youtube-nocookie.com'"));
        assert!(page.contains("s.src = 'https://www.youtube.com/iframe_api';"));
        // Every inline script carries the nonce, and no handler is inline.
        assert_eq!(page.matches("<script").count(), 1);
        assert!(!page.contains(" onload="));
        assert!(!page.contains(" onerror="));
        // Messages go only to the listed parent origins, never to '*'.
        assert!(!page.contains("'*'"));
        assert!(page.contains("PARENTS.indexOf(e.origin) < 0"));
        assert!(page.contains("e.source !== window.parent"));
    }

    #[test]
    fn the_page_clamps_the_volume_and_refuses_a_bad_id_or_nonce() {
        assert!(relay_page("dQw4w9WgXcQ", 200, "ab", PLAYER_API)
            .unwrap()
            .contains("var VOLUME = 100;"));
        assert_eq!(relay_page("dQw4w9WgX'Q", 50, "ab", PLAYER_API), None);
        assert_eq!(relay_page("dQw4w9WgXcQ", 50, "ab'c", PLAYER_API), None);
    }

    #[test]
    fn only_a_plain_loopback_url_may_replace_the_player_api() {
        let dead = "http://127.0.0.1:9/iframe_api";
        let page = relay_page("dQw4w9WgXcQ", 50, "ab", dead).unwrap();
        assert!(page.contains("s.src = 'http://127.0.0.1:9/iframe_api';"));
        let csp = relay_csp("ab", dead);
        assert!(
            csp.contains("https://s.ytimg.com http://127.0.0.1:9;"),
            "{csp}"
        );
        for bad in [
            "https://evil.example/x.js",
            "http://127.0.0.1/x.js",
            "http://127.0.0.1:9",
            "http://127.0.0.1:9/x';alert(1)//",
            "http://127.0.0.1:9/x x",
            "http://localhost:9/x",
        ] {
            assert_eq!(relay_page("dQw4w9WgXcQ", 50, "ab", bad), None, "{bad}");
            assert!(
                !relay_csp("ab", bad).contains("127.0.0.1"),
                "{bad}: {}",
                relay_csp("ab", bad)
            );
        }
    }
}
