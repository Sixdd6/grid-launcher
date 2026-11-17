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
    let mut attributes = tauri_build::Attributes::new().capabilities_path_pattern(pattern);

    // Windows (MSVC): the Common Controls v6 manifest goes in through the
    // linker, so the test binaries get it too, not only the app. Without it
    // a test binary fails to start (STATUS_ENTRYPOINT_NOT_FOUND): Windows
    // loads Common Controls v5, which lacks entry points tauri links to.
    // tauri-build's own copy of the manifest only reaches the app binary.
    // Tauri embeds it for its own tests the same way (tauri's build.rs).
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
