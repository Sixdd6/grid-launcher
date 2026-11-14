//! Epic updates: the owned list's `build_version` against each installed
//! row's `meta.version`. Pure; the only network is the owned refresh that
//! produced the list.

use super::meta::EpicMeta;
use super::{EpicGame, EpicUpdate};
use crate::library::registry::InstalledGame;

/// The installed games the owned list has a different build for. A row
/// with no version, or a game with no build version (a third-party
/// title), never shows an update.
pub fn updates(rows: &[InstalledGame], owned: &[EpicGame]) -> Vec<EpicUpdate> {
    rows.iter()
        .filter_map(|row| {
            let meta = EpicMeta::from_json(&row.store_meta);
            let installed = meta.version.trim();
            let game = owned
                .iter()
                .find(|game| game.app_name == row.store_id.trim())?;
            let latest = game.build_version.trim();
            (!installed.is_empty() && !latest.is_empty() && installed != latest).then(|| {
                EpicUpdate {
                    app_name: row.store_id.trim().to_string(),
                    version: latest.to_string(),
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{GameSource, StoreKind};

    fn row(id: &str, version: &str) -> InstalledGame {
        InstalledGame {
            store_id: id.into(),
            source: GameSource::Store(StoreKind::Epic),
            store_meta: EpicMeta {
                version: version.into(),
                ..Default::default()
            }
            .to_json(),
            ..Default::default()
        }
    }

    fn game(id: &str, build: &str) -> EpicGame {
        EpicGame {
            app_name: id.into(),
            build_version: build.into(),
            ..Default::default()
        }
    }

    #[test]
    fn updates_compare_build_versions() {
        let rows = [
            row("a", "1"),
            row("b", "2"),
            row("c", ""),
            row("d", "1"),
            row("f", " 4 "),
        ];
        let owned = [
            game("a", "1"),
            game("b", "3"),
            game("c", "9"),
            game("e", "1"),
            game("f", "4"),
        ];
        assert_eq!(
            updates(&rows, &owned),
            vec![EpicUpdate {
                app_name: "b".into(),
                version: "3".into()
            }]
        );
    }

    #[test]
    fn a_third_party_title_or_a_blank_meta_never_updates() {
        let rows = [
            row("a", "1"),
            InstalledGame {
                store_id: "b".into(),
                store_meta: String::new(),
                ..Default::default()
            },
        ];
        let owned = [game("a", ""), game("b", "2")];
        assert!(updates(&rows, &owned).is_empty());
    }
}
