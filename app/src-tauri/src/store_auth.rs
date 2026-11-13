//! The store login window (GameStores card "Login window", TVStores
//! "Signing in"): a separate, unprivileged window that shows the store's own
//! login page and catches its final redirect in Rust.
//!
//! - The window has the label [`LOGIN_WINDOW_LABEL`] and NO capability
//!   (`capabilities/*.json` name `main` only), so the store's page can never
//!   call a GRID command.
//! - Its webview is incognito (an ephemeral WebKit context on Linux) with a
//!   data folder of its own under `<data_dir>/stores/login-webview/`, so the
//!   store's cookies never mix with GRID's window and never outlive the
//!   window; Disconnect deletes that folder too ([`wipe_login_data`]).
//! - The redirect to `embed.gog.com/on_login_success?code=…` is caught by
//!   `on_navigation` (WebKitGTK asks the navigation policy for a server
//!   redirect too) and, as a fallback, by `on_page_load`. The navigation is
//!   refused, so the store's success page never loads; the code goes
//!   straight to [`crate::stores_service::StoresService::finish_login`] as a
//!   [`SecretString`]. It never crosses IPC, a log line or an error.
//! - Chrome: a plain decorated window. Its title is "Sign in to GOG — GRID"
//!   followed by the page's address (a lock when it is https), so the user
//!   can check the store's domain; GRID draws nothing inside the store's
//!   page. A network error replaces the page with GRID's own error page
//!   (Linux) whose button reloads the login page.
//! - TV mode types into the page with [`type_script`] (`store_login_type`).
//!
//! Secrets: nothing in this file logs a URL (it can carry the code), and the
//! text TV mode types (a password, possibly) is never logged either.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use secrecy::SecretString;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};

/// The login window's label. No capability names it.
pub const LOGIN_WINDOW_LABEL: &str = "store-login";

/// The path GOG redirects to when the login finishes.
const LOGIN_SUCCESS_PATH: &str = "on_login_success";

/// What a URL the login window is about to load means.
#[derive(Debug)]
pub enum Redirect {
    /// Any other page: let it load.
    Other,
    /// The finish redirect with its login code.
    Code(SecretString),
    /// The finish redirect without a code (GOG reported an error).
    Failed,
}

/// Whether `url` is the login's finish redirect: the embed host (scheme,
/// host and port of `embed`) and the `on_login_success` path under it.
/// Real form `https://embed.gog.com/on_login_success?origin=client&code=…`;
/// e2e form `http://127.0.0.1:<port>/on_login_success?code=…`.
pub fn is_login_redirect(url: &Url, embed: &Url) -> bool {
    if url.scheme() != embed.scheme()
        || url.host_str() != embed.host_str()
        || url.port_or_known_default() != embed.port_or_known_default()
    {
        return false;
    }
    let Ok(expected) = embed.join(LOGIN_SUCCESS_PATH) else {
        return false;
    };
    url.path().trim_end_matches('/') == expected.path().trim_end_matches('/')
}

/// The `code` query value, `None` when absent or blank.
fn code_param(url: &Url) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Classifies a URL the login window is about to load (see [`Redirect`]).
pub fn classify(url: &Url, embed: &Url) -> Redirect {
    if !is_login_redirect(url, embed) {
        return Redirect::Other;
    }
    match code_param(url) {
        Some(code) => Redirect::Code(SecretString::from(code)),
        None => Redirect::Failed,
    }
}

/// The window title: "Sign in to GOG — GRID", then the page's address with
/// a lock when it is https. Only the scheme, host and port: never a path or
/// a query (a query can carry the code).
pub fn window_title(store_name: &str, url: Option<&Url>) -> String {
    let base = format!("Sign in to {store_name} — GRID");
    let Some(url) = url else {
        return base;
    };
    let Some(host) = url.host_str() else {
        return base;
    };
    let address = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    if url.scheme() == "https" {
        format!("{base} · \u{1F512} {address}")
    } else {
        format!("{base} · {}://{address}", url.scheme())
    }
}

/// Key words `store_login_type` understands besides plain text: TV mode's
/// d-pad and on-screen keyboard keys.
pub const KEY_TAB: &str = "\t";
/// Shift-Tab (the terminal back-tab sequence).
pub const KEY_BACK_TAB: &str = "\u{1b}[Z";
/// Enter: presses the focused button, or submits the focused field's form.
pub const KEY_ENTER: &str = "\n";
/// Backspace: deletes the character before the caret.
pub const KEY_BACKSPACE: &str = "\u{8}";

/// The focus helpers every script below shares: the page's focusable,
/// visible, enabled elements in document order.
const FOCUS_JS: &str = r#"var sel='input:not([type=hidden]),select,textarea,button,a[href],[tabindex]:not([tabindex="-1"]),[contenteditable="true"]';
var all=Array.prototype.slice.call(document.querySelectorAll(sel)).filter(function(e){if(e.disabled)return false;var r=e.getBoundingClientRect();if(r.width===0&&r.height===0)return false;var s=getComputedStyle(e);return s.visibility!=='hidden'&&s.display!=='none';});"#;

/// The script that types `text` into the login page's focused field, or
/// moves focus for a key word ([`KEY_TAB`], [`KEY_BACK_TAB`],
/// [`KEY_ENTER`], [`KEY_BACKSPACE`]). Text goes in at the caret with an
/// `input` event (and `change`), through the element's own value setter so
/// a framework-managed field sees it. `text` is embedded as a JSON string,
/// so nothing in it can break out of the literal.
pub fn type_script(text: &str) -> String {
    let literal = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    let body = match text {
        KEY_TAB | KEY_BACK_TAB => {
            let step = if text == KEY_TAB { 1 } else { -1 };
            format!(
                "{FOCUS_JS}if(!all.length)return;var i=all.indexOf(document.activeElement);\
                 var n=i<0?(({step})>0?0:all.length-1):(i+({step})+all.length)%all.length;all[n].focus();"
            )
        }
        KEY_ENTER => "var e=document.activeElement;if(!e)return;\
             if(e.tagName==='BUTTON'||e.tagName==='A'||(e.tagName==='INPUT'&&(e.type==='submit'||e.type==='button'||e.type==='checkbox'||e.type==='radio'))){e.click();return;}\
             if(e.form){if(e.form.requestSubmit)e.form.requestSubmit();else e.form.submit();}"
            .to_string(),
        KEY_BACKSPACE => "var e=document.activeElement;if(!e||!('value' in e))return;\
             var s=e.selectionStart==null?e.value.length:e.selectionStart,en=e.selectionEnd==null?s:e.selectionEnd;\
             if(s===en&&s>0)s-=1;var v=e.value.slice(0,s)+e.value.slice(en);\
             var d=Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e),'value');if(d&&d.set)d.set.call(e,v);else e.value=v;\
             try{e.setSelectionRange(s,s);}catch(_){}\
             e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));"
            .to_string(),
        _ => format!(
            "var t={literal};var e=document.activeElement;if(!e)return;\
             if(e.isContentEditable){{document.execCommand('insertText',false,t);return;}}\
             if(!('value' in e))return;\
             var s=e.selectionStart==null?e.value.length:e.selectionStart,en=e.selectionEnd==null?s:e.selectionEnd;\
             var v=e.value.slice(0,s)+t+e.value.slice(en);\
             var d=Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e),'value');if(d&&d.set)d.set.call(e,v);else e.value=v;\
             try{{e.setSelectionRange(s+t.length,s+t.length);}}catch(_){{}}\
             e.dispatchEvent(new Event('input',{{bubbles:true}}));e.dispatchEvent(new Event('change',{{bubbles:true}}));"
        ),
    };
    format!("(function(){{{body}}})();")
}

/// The script run when a login page finishes loading: focus lands on the
/// page's first text field (TVStores "Focus lands on the page's first
/// field"), unless the page already focused one.
pub fn focus_first_field_script() -> String {
    format!(
        "(function(){{{FOCUS_JS}var a=document.activeElement;if(a&&a!==document.body)return;\
         for(var i=0;i<all.length;i++){{var e=all[i];if(e.tagName==='INPUT'||e.tagName==='TEXTAREA'){{e.focus();return;}}}}\
         if(all.length)all[0].focus();}})();"
    )
}

/// GRID's page in place of a login page that could not be reached
/// (GameStores "Network error": a warning strip inside the window). The
/// button reloads the login page.
#[cfg(any(target_os = "linux", test))]
pub fn network_error_html(host: &str, login_url: &Url) -> String {
    let host = html_escape(host);
    let target = serde_json::to_string(login_url.as_str()).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Sign in</title>\
<style>body{{font:15px/1.45 system-ui,sans-serif;margin:0;background:#15171c;color:#e8e9ec}}\
.strip{{background:#4a3410;color:#ffd99a;padding:14px 18px;border-bottom:1px solid #6b4b16}}\
main{{padding:24px 18px}}button{{font:inherit;padding:8px 16px;border-radius:6px;border:1px solid #555;background:#2a2d35;color:inherit;cursor:pointer}}\
button:focus{{outline:2px solid #7aa7ff;outline-offset:2px}}</style></head><body>\
<div class=\"strip\" role=\"alert\">Couldn\u{2019}t reach {host}. Check your connection; the page reloads when you try again.</div>\
<main><button id=\"retry\" autofocus onclick='location.href={target}'>Try again</button></main></body></html>"
    )
}

#[cfg(any(target_os = "linux", test))]
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `<data_dir>/stores/login-webview/<source>`: the login window's own
/// webview data folder.
pub fn login_data_dir(data_dir: &Path, source: &str) -> PathBuf {
    data_dir
        .join(grid_core::stores::paths::STORES_DIR)
        .join("login-webview")
        .join(source)
}

/// Deletes the login window's webview data (its cookies) for `source`.
/// A missing folder is fine.
pub fn wipe_login_data(data_dir: &Path, source: &str) -> std::io::Result<()> {
    match std::fs::remove_dir_all(login_data_dir(data_dir, source)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// What the login window reports back, once.
pub trait LoginSink: Send + Sync + 'static {
    /// The finish redirect carried a code.
    fn code(&self, code: SecretString);
    /// The finish redirect carried no code.
    fn failed(&self);
    /// The window closed before the redirect (✕, Cancel).
    fn cancelled(&self);
}

/// Everything [`open_login_window`] needs.
pub struct LoginWindowSpec {
    /// "GOG".
    pub store_name: String,
    /// `gog`, for the data folder.
    pub source: String,
    pub login_url: Url,
    /// The embed host whose `on_login_success` ends the login.
    pub embed: Url,
    pub data_dir: PathBuf,
    /// TV mode: keep the window above the fullscreen main window.
    pub tv: bool,
}

/// Fires the sink once, whichever of the redirect hooks or the close sees
/// the end first.
struct Once<S: LoginSink> {
    done: AtomicBool,
    sink: S,
}

impl<S: LoginSink> Once<S> {
    /// `true` the first time only.
    fn take(&self) -> bool {
        !self.done.swap(true, Ordering::SeqCst)
    }

    /// Handles a URL the window is about to load or has loaded. Returns
    /// whether the window may load it.
    fn on_url(&self, url: &Url, embed: &Url) -> bool {
        match classify(url, embed) {
            Redirect::Other => true,
            Redirect::Code(code) => {
                if self.take() {
                    self.sink.code(code);
                }
                false
            }
            Redirect::Failed => {
                if self.take() {
                    self.sink.failed();
                }
                false
            }
        }
    }
}

/// Opens the login window, or focuses it when it is already open. The sink
/// hears exactly one outcome; the window closes itself after a redirect.
pub fn open_login_window<S: LoginSink>(
    app: &AppHandle,
    spec: LoginWindowSpec,
    sink: S,
) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let once = Arc::new(Once {
        done: AtomicBool::new(false),
        sink,
    });
    let data = login_data_dir(&spec.data_dir, &spec.source);
    let _ = std::fs::create_dir_all(&data);

    let nav_once = once.clone();
    let nav_embed = spec.embed.clone();
    let nav_app = app.clone();
    let load_once = once.clone();
    let load_embed = spec.embed.clone();
    let load_app = app.clone();
    let store_name = spec.store_name.clone();
    let mut builder = WebviewWindowBuilder::new(
        app,
        LOGIN_WINDOW_LABEL,
        WebviewUrl::External(spec.login_url.clone()),
    )
    .title(window_title(&spec.store_name, Some(&spec.login_url)))
    .inner_size(520.0, 760.0)
    .center()
    .focused(true)
    .incognito(true)
    .data_directory(data)
    .on_navigation(move |url| {
        let allow = nav_once.on_url(url, &nav_embed);
        if !allow {
            tracing::info!("store login: finish redirect caught (navigation)");
            close_soon(&nav_app);
        }
        allow
    })
    .on_page_load(move |window, payload| {
        if !load_once.on_url(payload.url(), &load_embed) {
            tracing::info!("store login: finish redirect caught (page load)");
            close_soon(&load_app);
            return;
        }
        if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
            let _ = window.set_title(&window_title(&store_name, Some(payload.url())));
            let _ = window.eval(focus_first_field_script());
        }
    });
    // Transient for the main window, so it stays above it (TV mode's
    // fullscreen window too).
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main)?;
    }
    if spec.tv {
        builder = builder.always_on_top(true);
    }
    let window = builder.build()?;

    let close_once = once.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) && close_once.take() {
            close_once.sink.cancelled();
        }
    });

    #[cfg(target_os = "linux")]
    install_network_error_page(&window, spec.login_url.clone());
    Ok(())
}

/// Closes the login window from outside the WebKit signal that caught the
/// redirect.
fn close_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
            let _ = window.close();
        }
    });
}

/// Closes the login window if it is open (Cancel). Its close reports
/// `cancelled` unless a redirect came first.
pub fn close_login_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        let _ = window.close();
    }
}

/// Runs [`type_script`] in the login window. `false` when it is not open.
pub fn type_into_login_window(app: &AppHandle, text: &str) -> bool {
    match app.get_webview_window(LOGIN_WINDOW_LABEL) {
        Some(window) => window.eval(type_script(text)).is_ok(),
        None => false,
    }
}

/// Whether the login window is open.
pub fn login_window_open(app: &AppHandle) -> bool {
    app.get_webview_window(LOGIN_WINDOW_LABEL).is_some()
}

/// Linux: a network failure shows GRID's error page instead of WebKit's.
/// A navigation GRID refused (the finish redirect) or one that was
/// cancelled is not a network error.
#[cfg(target_os = "linux")]
fn install_network_error_page(window: &tauri::WebviewWindow, login_url: Url) {
    let _ = window.with_webview(move |webview| {
        use webkit2gtk::{NetworkError, PolicyError, WebViewExt};
        let login_url = login_url.clone();
        webview
            .inner()
            .connect_load_failed(move |view, _event, uri, error| {
                // A refused navigation (the finish redirect) and a
                // cancelled load are not network errors; anything else
                // (WebKit's network domain, or the HTTP stack's own error
                // for a dropped or refused connection) is.
                if error.kind::<PolicyError>().is_some()
                    || matches!(error.kind::<NetworkError>(), Some(NetworkError::Cancelled))
                {
                    return false;
                }
                let host = Url::parse(uri)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_string))
                    .unwrap_or_default();
                tracing::info!("store login: the login page could not be reached");
                view.load_alternate_html(&network_error_html(&host, &login_url), uri, None);
                true
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn the_real_redirect_matches() {
        let embed = url("https://embed.gog.com/");
        assert!(is_login_redirect(
            &url("https://embed.gog.com/on_login_success?origin=client&code=abc"),
            &embed
        ));
        assert!(is_login_redirect(
            &url("https://embed.gog.com/on_login_success/?code=abc"),
            &embed
        ));
        for other in [
            "https://login.gog.com/on_login_success?code=abc",
            "http://embed.gog.com/on_login_success?code=abc",
            "https://embed.gog.com:8443/on_login_success?code=abc",
            "https://embed.gog.com/on_login_successful?code=abc",
            "https://embed.gog.com/account",
            "https://embed.gog.com.evil.example/on_login_success?code=abc",
            "https://auth.gog.com/auth?client_id=1&redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success",
        ] {
            assert!(!is_login_redirect(&url(other), &embed), "{other} matched");
        }
    }

    #[test]
    fn the_e2e_redirect_matches_with_its_port() {
        let embed = url("http://127.0.0.1:41235/");
        assert!(is_login_redirect(
            &url("http://127.0.0.1:41235/on_login_success?code=e2e-code"),
            &embed
        ));
        assert!(!is_login_redirect(
            &url("http://127.0.0.1:41236/on_login_success?code=e2e-code"),
            &embed
        ));
        assert!(!is_login_redirect(
            &url("http://127.0.0.1:41235/auth?code=e2e-code"),
            &embed
        ));
    }

    #[test]
    fn an_embed_base_with_a_path_prefix_is_honoured() {
        let embed = url("http://127.0.0.1:9000/gog/");
        assert!(is_login_redirect(
            &url("http://127.0.0.1:9000/gog/on_login_success?code=x"),
            &embed
        ));
        assert!(!is_login_redirect(
            &url("http://127.0.0.1:9000/on_login_success?code=x"),
            &embed
        ));
    }

    #[test]
    fn the_code_is_read_from_the_query() {
        assert_eq!(
            code_param(&url(
                "https://embed.gog.com/on_login_success?origin=client&code=abc123"
            )),
            Some("abc123".to_string())
        );
        assert_eq!(
            code_param(&url("https://embed.gog.com/on_login_success?code=")),
            None
        );
        assert_eq!(
            code_param(&url(
                "https://embed.gog.com/on_login_success?error=access_denied"
            )),
            None
        );
        let embed = url("https://embed.gog.com/");
        assert!(matches!(
            classify(
                &url("https://embed.gog.com/on_login_success?code=abc"),
                &embed
            ),
            Redirect::Code(_)
        ));
        assert!(matches!(
            classify(
                &url("https://embed.gog.com/on_login_success?error=x"),
                &embed
            ),
            Redirect::Failed
        ));
        assert!(matches!(
            classify(&url("https://login.gog.com/"), &embed),
            Redirect::Other
        ));
    }

    #[test]
    fn a_code_never_shows_in_the_debug_form() {
        let embed = url("https://embed.gog.com/");
        let out = classify(
            &url("https://embed.gog.com/on_login_success?code=FAKE-STORE-TOKEN-not-real"),
            &embed,
        );
        assert!(!format!("{out:?}").contains("FAKE-STORE-TOKEN-not-real"));
    }

    #[test]
    fn titles_show_the_host_and_a_lock_for_https_only() {
        assert_eq!(window_title("GOG", None), "Sign in to GOG — GRID");
        assert_eq!(
            window_title("GOG", Some(&url("https://login.gog.com/auth?code=secret"))),
            "Sign in to GOG — GRID · \u{1F512} login.gog.com"
        );
        let e2e = window_title("GOG", Some(&url("http://127.0.0.1:4100/auth?x=1")));
        assert_eq!(e2e, "Sign in to GOG — GRID · http://127.0.0.1:4100");
        assert!(!e2e.contains("x=1"));
    }

    #[test]
    fn typed_text_is_a_json_literal() {
        let script = type_script("a\"b'</script>\\c");
        assert!(script.contains(r#"var t="a\"b'</script>\\c";"#), "{script}");
        assert!(script.contains("new Event('input'"));
        assert!(script.starts_with("(function(){") && script.ends_with("})();"));
    }

    #[test]
    fn key_words_move_focus_or_submit() {
        assert!(type_script(KEY_TAB).contains("all[n].focus()"));
        assert!(type_script(KEY_TAB).contains("i+(1)"));
        assert!(type_script(KEY_BACK_TAB).contains("i+(-1)"));
        assert!(type_script(KEY_ENTER).contains("requestSubmit"));
        assert!(type_script(KEY_BACKSPACE).contains("s-=1"));
        assert!(!type_script(KEY_TAB).contains("var t="));
    }

    #[test]
    fn the_error_page_escapes_the_host() {
        let page = network_error_html("<b>x</b>", &url("https://auth.gog.com/auth?a=1"));
        assert!(page.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(page.contains("Check your connection; the page reloads when you try again."));
        assert!(page.contains(r#"location.href="https://auth.gog.com/auth?a=1""#));
    }

    #[test]
    fn the_login_data_folder_is_per_store_and_wipes() {
        let dir = tempfile::tempdir().unwrap();
        let data = login_data_dir(dir.path(), "gog");
        assert_eq!(
            data,
            dir.path().join("stores").join("login-webview").join("gog")
        );
        std::fs::create_dir_all(data.join("cookies")).unwrap();
        wipe_login_data(dir.path(), "gog").unwrap();
        assert!(!data.exists());
        wipe_login_data(dir.path(), "gog").unwrap();
    }

    struct Recorder(std::sync::Mutex<Vec<&'static str>>);
    impl LoginSink for Arc<Recorder> {
        fn code(&self, _code: SecretString) {
            self.0.lock().unwrap().push("code");
        }
        fn failed(&self) {
            self.0.lock().unwrap().push("failed");
        }
        fn cancelled(&self) {
            self.0.lock().unwrap().push("cancelled");
        }
    }

    #[test]
    fn the_sink_hears_one_outcome() {
        let rec = Arc::new(Recorder(Default::default()));
        let once = Once {
            done: AtomicBool::new(false),
            sink: rec.clone(),
        };
        let embed = url("http://127.0.0.1:1/");
        assert!(once.on_url(&url("http://127.0.0.1:1/auth"), &embed));
        assert!(!once.on_url(&url("http://127.0.0.1:1/on_login_success?code=c"), &embed));
        // The page-load fallback sees the same URL again; the close after it.
        assert!(!once.on_url(&url("http://127.0.0.1:1/on_login_success?code=c"), &embed));
        assert!(!once.take());
        assert_eq!(*rec.0.lock().unwrap(), vec!["code"]);
    }
}
