//! The GOG update check. A Windows (content-system) install compares the
//! default build's id with `meta.build_id`; a Linux installer build
//! compares api.gog.com's Linux installer version with `meta.version`
//! (`docs/stores-phase1-plan.md` "Update identity"). An offline or drive
//! install with no build id never shows an update.

use super::api::{self, Build};
use super::meta::{GogMeta, GogOrigin, GogPlatform};
use super::{GogService, GogUpdate};
use crate::library::registry::InstalledGame;
use crate::stores::{GameSource, StoreError, StoreKind};

/// The update a Windows build has: the default build, when its id differs
/// from the installed one. `None` without an installed build id.
pub fn windows_update(store_id: &str, meta: &GogMeta, builds: &[Build]) -> Option<GogUpdate> {
    let installed = meta.build_id.trim();
    if installed.is_empty() {
        return None;
    }
    let latest = api::default_build(builds)?;
    (latest.build_id != installed).then(|| GogUpdate {
        store_id: store_id.trim().to_string(),
        version: latest.version_name.clone(),
        build_id: latest.build_id.clone(),
    })
}

/// The update a Linux installer build has: GOG's Linux installer version,
/// when it differs from the installed one. An offline or drive install
/// with no build id never has one.
pub fn linux_update(store_id: &str, meta: &GogMeta, latest: Option<&str>) -> Option<GogUpdate> {
    if meta.origin != GogOrigin::Gogdl && meta.build_id.trim().is_empty() {
        return None;
    }
    let installed = meta.version.trim();
    let latest = latest.map(str::trim).filter(|v| !v.is_empty())?;
    (!installed.is_empty() && latest != installed).then(|| GogUpdate {
        store_id: store_id.trim().to_string(),
        version: latest.to_string(),
        build_id: String::new(),
    })
}

/// Newer builds for the GOG rows among `rows`. A row whose check fails is
/// skipped; a failed sign-in fails the whole check.
pub async fn check_updates(
    svc: &GogService,
    rows: &[InstalledGame],
) -> Result<Vec<GogUpdate>, StoreError> {
    let rows: Vec<&InstalledGame> = rows
        .iter()
        .filter(|r| r.source == GameSource::Store(StoreKind::Gog) && !r.store_id.trim().is_empty())
        .collect();
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let token = match svc.bearer().await {
        Ok(token) => Some(token),
        Err(StoreError::NotSignedIn) => None,
        Err(error) => return Err(error),
    };
    let bearer = token.as_ref().map(|t| &t.access);
    let mut updates = Vec::new();
    for row in rows {
        let meta = GogMeta::from_json(&row.store_meta);
        let found = match meta.platform {
            GogPlatform::Windows => {
                if meta.build_id.trim().is_empty() {
                    continue;
                }
                match api::builds(
                    svc.http(),
                    svc.endpoints(),
                    bearer,
                    &row.store_id,
                    "windows",
                )
                .await
                {
                    Ok(builds) => windows_update(&row.store_id, &meta, &builds),
                    Err(StoreError::LoginExpired) => return Err(StoreError::LoginExpired),
                    Err(_) => None,
                }
            }
            GogPlatform::Linux => {
                match api::product_json(
                    svc.http(),
                    svc.endpoints(),
                    bearer,
                    &row.store_id,
                    "downloads",
                )
                .await
                {
                    Ok(Some(value)) => linux_update(
                        &row.store_id,
                        &meta,
                        api::parse_linux_installer_version(&value).as_deref(),
                    ),
                    Ok(None) => None,
                    Err(StoreError::LoginExpired) => return Err(StoreError::LoginExpired),
                    Err(_) => None,
                }
            }
        };
        updates.extend(found);
    }
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(id: &str, branch: Option<&str>, version: &str) -> Build {
        Build {
            build_id: id.into(),
            branch: branch.map(str::to_string),
            version_name: version.into(),
            generation: 2,
            link: String::new(),
        }
    }

    #[test]
    fn windows_compares_the_default_build() {
        let meta = GogMeta {
            build_id: "2".into(),
            ..Default::default()
        };
        let builds = [build("9", Some("beta"), "3b"), build("3", None, "1.6")];
        let update = windows_update("1", &meta, &builds).unwrap();
        assert_eq!(update.build_id, "3");
        assert_eq!(update.version, "1.6");
        let current = [build("2", None, "1.5")];
        assert!(windows_update("1", &meta, &current).is_none());
        assert!(windows_update("1", &GogMeta::default(), &builds).is_none());
    }

    #[test]
    fn linux_compares_the_installer_version() {
        let meta = GogMeta {
            platform: GogPlatform::Linux,
            version: "2.0".into(),
            ..Default::default()
        };
        assert_eq!(
            linux_update("1", &meta, Some("2.1")).unwrap().version,
            "2.1"
        );
        assert!(linux_update("1", &meta, Some("2.0")).is_none());
        assert!(linux_update("1", &meta, None).is_none());
        let offline = GogMeta {
            origin: GogOrigin::Offline,
            ..meta.clone()
        };
        assert!(linux_update("1", &offline, Some("2.1")).is_none());
        let offline_with_build = GogMeta {
            build_id: "5".into(),
            ..offline
        };
        assert!(linux_update("1", &offline_with_build, Some("2.1")).is_some());
    }
}
