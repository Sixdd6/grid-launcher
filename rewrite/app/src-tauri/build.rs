fn main() {
    // `tauri-build` validates every file matched by the capabilities glob
    // against the permissions of currently-linked plugins, *unconditionally*
    // — this happens regardless of tauri.conf.json's `security.capabilities`
    // list, which only controls what gets embedded into the runtime ACL.
    // `capabilities/e2e.json` grants `wdio:default` / `wdio-webdriver:default`,
    // permissions that only exist once the `e2e` feature links the
    // corresponding plugins. Without this, a plain `cargo build -p app`
    // (default features) fails at compile time with "Permission wdio:default
    // not found", even though the plugins and their commands are never
    // reachable in that build. So the capabilities glob itself must exclude
    // e2e.json unless the `e2e` feature is enabled.
    println!("cargo:rerun-if-changed=capabilities");
    let e2e = std::env::var_os("CARGO_FEATURE_E2E").is_some();
    let pattern = if e2e {
        "./capabilities/**/*"
    } else {
        "./capabilities/default.json"
    };
    let attributes = tauri_build::Attributes::new().capabilities_path_pattern(pattern);
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
