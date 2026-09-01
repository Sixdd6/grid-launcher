use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use grid_core::library::download::{download_targets, FileTarget};
use grid_core::library::LibraryError;
use grid_core::romm::{RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

#[tokio::test]
async fn single_target_downloads_and_reports_final_progress() {
    let server = MockServer::start().await;
    let body = b"hello world".to_vec();
    Mock::given(method("GET"))
        .and(path("/api/roms/1/content/game.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("game.zip");
    let targets = vec![FileTarget {
        url_path: "/api/roms/1/content/game.zip".to_string(),
        query: vec![],
        dest: dest.clone(),
        expected_size: body.len() as i64,
    }];
    let cancel = AtomicBool::new(false);
    let mut last: Option<(u64, u64, f64)> = None;
    let mut calls = 0u32;
    let mut on_progress = |d: u64, t: u64, s: f64| {
        calls += 1;
        last = Some((d, t, s));
    };

    download_targets(&client, &targets, &cancel, &mut on_progress)
        .await
        .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), body);
    let (downloaded, total, _speed) = last.expect("at least one progress emission");
    assert_eq!(downloaded, body.len() as u64);
    assert_eq!(total, body.len() as u64);
    assert!(calls >= 1);
}

#[tokio::test]
async fn multi_target_accumulates_progress_and_both_land() {
    let server = MockServer::start().await;
    let body1 = b"AAAA".to_vec();
    let body2 = b"BBBBBB".to_vec();
    Mock::given(method("GET"))
        .and(path("/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body1.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/f2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body2.clone()))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest1 = dir.path().join("f1.bin");
    let dest2 = dir.path().join("f2.bin");
    let targets = vec![
        FileTarget {
            url_path: "/f1".to_string(),
            query: vec![],
            dest: dest1.clone(),
            expected_size: body1.len() as i64,
        },
        FileTarget {
            url_path: "/f2".to_string(),
            query: vec![],
            dest: dest2.clone(),
            expected_size: body2.len() as i64,
        },
    ];
    let cancel = AtomicBool::new(false);
    let mut last = (0u64, 0u64, 0.0f64);
    let mut on_progress = |d: u64, t: u64, s: f64| {
        last = (d, t, s);
    };

    download_targets(&client, &targets, &cancel, &mut on_progress)
        .await
        .unwrap();

    assert_eq!(fs::read(&dest1).unwrap(), body1);
    assert_eq!(fs::read(&dest2).unwrap(), body2);
    let expected_total = (body1.len() + body2.len()) as u64;
    assert_eq!(last.0, expected_total);
    assert_eq!(last.1, expected_total);
}

/// Cancellation is triggered from inside the progress callback the instant
/// the first target's full byte count has been reported — this lands
/// deterministically between target 1 finishing and target 2 starting
/// (small wiremock bodies arrive as a single `bytes_stream` chunk, so the
/// very first progress emission already reports the whole of target 1). The
/// in-flight target 2 must be deleted; target 1 must survive.
#[tokio::test]
async fn cancellation_deletes_in_flight_partial_and_keeps_completed() {
    let server = MockServer::start().await;
    let body1 = b"COMPLETE".to_vec();
    let body2 = b"SHOULD-NOT-FULLY-LAND".to_vec();
    Mock::given(method("GET"))
        .and(path("/f1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body1.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/f2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body2.clone()))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest1 = dir.path().join("f1.bin");
    let dest2 = dir.path().join("f2.bin");
    let targets = vec![
        FileTarget {
            url_path: "/f1".to_string(),
            query: vec![],
            dest: dest1.clone(),
            expected_size: body1.len() as i64,
        },
        FileTarget {
            url_path: "/f2".to_string(),
            query: vec![],
            dest: dest2.clone(),
            expected_size: body2.len() as i64,
        },
    ];
    let cancel = AtomicBool::new(false);
    let body1_len = body1.len() as u64;
    let mut on_progress = |downloaded: u64, _total: u64, _speed: f64| {
        if downloaded >= body1_len {
            cancel.store(true, Ordering::Relaxed);
        }
    };

    let err = download_targets(&client, &targets, &cancel, &mut on_progress)
        .await
        .unwrap_err();

    assert!(matches!(err, LibraryError::Cancelled), "got {err:?}");
    assert_eq!(fs::read(&dest1).unwrap(), body1);
    assert!(
        !dest2.exists(),
        "in-flight target's partial must be deleted"
    );
}

#[tokio::test]
async fn http_error_maps_to_romm_http_and_leaves_no_partial() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server exploded"))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("boom.bin");
    let targets = vec![FileTarget {
        url_path: "/boom".to_string(),
        query: vec![],
        dest: dest.clone(),
        expected_size: 10,
    }];
    let cancel = AtomicBool::new(false);
    let mut on_progress = |_: u64, _: u64, _: f64| {};

    let err = download_targets(&client, &targets, &cancel, &mut on_progress)
        .await
        .unwrap_err();

    match err {
        LibraryError::Romm(RommError::Http { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected Romm(Http), got {other:?}"),
    }
    assert!(!dest.exists());
}

#[tokio::test]
async fn pre_existing_dest_with_matching_size_is_not_re_requested() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/skip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"should not be fetched".to_vec()))
        .expect(0)
        .mount_as_scoped(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("existing.bin");
    let content = b"already-here".to_vec();
    fs::write(&dest, &content).unwrap();
    let targets = vec![FileTarget {
        url_path: "/skip".to_string(),
        query: vec![],
        dest: dest.clone(),
        expected_size: content.len() as i64,
    }];
    let cancel = AtomicBool::new(false);
    let mut last = (0u64, 0u64, 0.0f64);
    let mut on_progress = |d: u64, t: u64, s: f64| {
        last = (d, t, s);
    };

    download_targets(&client, &targets, &cancel, &mut on_progress)
        .await
        .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), content);
    assert_eq!(last.0, content.len() as u64);
    drop(mock); // expect(0) verified on drop: no request was ever made
}
