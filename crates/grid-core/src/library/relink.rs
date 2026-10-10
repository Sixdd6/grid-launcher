//! Q4: installed rows with no ROM id. A row imported from the Python app (or
//! installed before rom ids were recorded) carries a title and a platform but
//! no server id, so Play, cloud saves and update checks cannot reach it.
//!
//! After a connect, the relink pass matches each such row against the
//! server's ROMs for the row's platform by normalized title. It links a row
//! ONLY when exactly one server ROM matches, and never to a rom id another
//! installed row already holds. Every other row stays unlinked; the Details
//! view offers "Link to server game" for it.
//!
//! The matcher ([`match_unlinked`]) is pure. [`relink_pass`] does the I/O:
//! one platform list, one ROM list per platform that has an unlinked row,
//! and one registry write per link. Logs carry the title and the rom id
//! only — never a URL, a header or a token.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use super::registry::{InstalledGame, Registry, SetRomIdOutcome};
use super::LibraryError;
use crate::autoconfig::cores::normalize_platform_key;
use crate::romm::{GameSummary, Platform, RommClient};

/// One link: the registry identity of an unlinked row and the rom id it gets.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RelinkMatch {
    pub title: String,
    pub platform: String,
    pub rom_id: i64,
}

/// The form two titles are compared in: lowercased, every run of characters
/// that are not `a-z`/`0-9` collapsed to one space, trimmed. The same
/// normalizer the RetroArch platform lookup uses (`normalize_platform_key`),
/// so "Chrono Trigger", "chrono-trigger" and "CHRONO TRIGGER!" compare equal.
pub fn normalize_title(title: &str) -> String {
    normalize_platform_key(title)
}

/// Whether `row_platform` (the platform text a registry row stores) names
/// `platform`. The row text is compared, normalized, against every name the
/// server gives the platform: its display name, its custom name, its raw
/// name and its slug. A blank row platform names nothing.
pub fn server_platform_matches(row_platform: &str, platform: &Platform) -> bool {
    let wanted = normalize_platform_key(row_platform);
    if wanted.is_empty() {
        return false;
    }
    let custom = platform.custom_name.as_deref().unwrap_or("");
    [
        platform.display_name.as_str(),
        custom,
        platform.name.as_str(),
        platform.slug.as_str(),
    ]
    .iter()
    .any(|alias| !alias.trim().is_empty() && normalize_platform_key(alias) == wanted)
}

/// The ids of every server platform `row_platform` names, in server order.
pub fn platform_ids_for(row_platform: &str, platforms: &[Platform]) -> Vec<i64> {
    platforms
        .iter()
        .filter(|p| server_platform_matches(row_platform, p))
        .map(|p| p.id)
        .collect()
}

/// The server platforms whose ROM lists the pass needs: the ones some
/// unlinked row names. Sorted and de-duplicated.
pub fn platforms_to_fetch(rows: &[InstalledGame], platforms: &[Platform]) -> Vec<i64> {
    let ids: BTreeSet<i64> = rows
        .iter()
        .filter(|row| row.rom_id.is_none())
        .flat_map(|row| platform_ids_for(&row.platform, platforms))
        .collect();
    ids.into_iter().collect()
}

/// The links to make. For each row with no rom id: the server ROMs on the
/// platforms the row names whose normalized name equals the row's
/// normalized title. A row is linked only when
///
/// - exactly one distinct ROM matches,
/// - no installed row already holds that rom id, and
/// - no other unlinked row matched the same ROM in this pass (two rows
///   that both claim one ROM are ambiguous, so neither is linked).
///
/// Rows that already have a rom id are never touched. Output follows the
/// order of `rows`.
pub fn match_unlinked(
    rows: &[InstalledGame],
    platforms: &[Platform],
    roms_by_platform: &HashMap<i64, Vec<GameSummary>>,
) -> Vec<RelinkMatch> {
    let held: BTreeSet<i64> = rows.iter().filter_map(|row| row.rom_id).collect();

    let mut proposed: Vec<(usize, i64)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row.rom_id.is_some() {
            continue;
        }
        let title = normalize_title(&row.title);
        if title.is_empty() {
            continue;
        }
        let mut candidates: BTreeSet<i64> = BTreeSet::new();
        for platform_id in platform_ids_for(&row.platform, platforms) {
            let Some(roms) = roms_by_platform.get(&platform_id) else {
                continue;
            };
            candidates.extend(
                roms.iter()
                    .filter(|rom| normalize_title(&rom.name) == title)
                    .map(|rom| rom.id),
            );
        }
        if candidates.len() != 1 {
            continue;
        }
        let rom_id = *candidates.iter().next().expect("one candidate");
        if held.contains(&rom_id) {
            continue;
        }
        proposed.push((index, rom_id));
    }

    let mut claims: BTreeMap<i64, usize> = BTreeMap::new();
    for (_, rom_id) in &proposed {
        *claims.entry(*rom_id).or_default() += 1;
    }
    proposed
        .into_iter()
        .filter(|(_, rom_id)| claims.get(rom_id) == Some(&1))
        .map(|(index, rom_id)| RelinkMatch {
            title: rows[index].title.clone(),
            platform: rows[index].platform.clone(),
            rom_id,
        })
        .collect()
}

/// Every server ROM on the platforms `row_platform` names, for the Details
/// "Link to server game" picker. Empty when no server platform matches.
pub async fn server_roms_for_platform(
    client: &RommClient,
    row_platform: &str,
) -> Result<Vec<GameSummary>, LibraryError> {
    let platforms = client.platforms().await?;
    let mut out = Vec::new();
    for platform_id in platform_ids_for(row_platform, &platforms) {
        out.extend(client.games(platform_id).await?);
    }
    Ok(out)
}

/// The relink pass. Reads the registry; when no row lacks a rom id it makes
/// no request at all. Otherwise it reads the platform list and the ROM list
/// of each platform an unlinked row names, runs [`match_unlinked`], and
/// writes each link with [`Registry::set_rom_id_by_key`]. Returns the links
/// it wrote. A platform whose ROM list fails is skipped (its rows stay
/// unlinked); a failed platform list fails the pass.
pub async fn relink_pass(
    registry: Arc<Registry>,
    client: &RommClient,
) -> Result<Vec<RelinkMatch>, LibraryError> {
    let reg = registry.clone();
    let rows = tokio::task::spawn_blocking(move || reg.all())
        .await
        .map_err(join_err)??;
    if rows.iter().all(|row| row.rom_id.is_some()) {
        return Ok(Vec::new());
    }

    let platforms = client.platforms().await?;
    let mut roms_by_platform: HashMap<i64, Vec<GameSummary>> = HashMap::new();
    for platform_id in platforms_to_fetch(&rows, &platforms) {
        match client.games(platform_id).await {
            Ok(roms) => {
                roms_by_platform.insert(platform_id, roms);
            }
            Err(e) => tracing::warn!(platform_id, "relink: ROM list failed: {e}"),
        }
    }

    let matches = match_unlinked(&rows, &platforms, &roms_by_platform);
    if matches.is_empty() {
        return Ok(Vec::new());
    }
    let reg = registry.clone();
    let linked = tokio::task::spawn_blocking(move || -> Result<Vec<RelinkMatch>, LibraryError> {
        let mut linked = Vec::new();
        for link in matches {
            match reg.set_rom_id_by_key(&link.title, &link.platform, link.rom_id)? {
                SetRomIdOutcome::Linked => linked.push(link),
                // The registry changed under the pass (an install, a manual
                // link): leave the row for the next pass or the user.
                SetRomIdOutcome::RomIdHeld | SetRomIdOutcome::NoSuchRow => {}
            }
        }
        Ok(linked)
    })
    .await
    .map_err(join_err)??;
    for link in &linked {
        tracing::info!(
            title = %link.title,
            rom_id = link.rom_id,
            "relink: linked an installed game to its server ROM"
        );
    }
    Ok(linked)
}

fn join_err(e: tokio::task::JoinError) -> LibraryError {
    LibraryError::Registry(format!("the relink task did not finish: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(id: i64, name: &str, slug: &str, custom: Option<&str>, display: &str) -> Platform {
        Platform {
            id,
            name: name.to_string(),
            slug: slug.to_string(),
            rom_count: 1,
            custom_name: custom.map(str::to_string),
            display_name: display.to_string(),
        }
    }

    fn snes() -> Platform {
        platform(
            1,
            "Super Nintendo Entertainment System",
            "snes",
            None,
            "Super Nintendo Entertainment System",
        )
    }

    fn windows() -> Platform {
        platform(3, "Windows", "win", Some("Windows 9x"), "Windows 9x")
    }

    fn rom(id: i64, name: &str, platform_id: i64) -> GameSummary {
        GameSummary {
            id,
            name: name.to_string(),
            platform_id,
            cover_path: None,
            cover_large_path: None,
            screenshot_urls: Vec::new(),
            fanart_urls: Vec::new(),
            platform_display_name: String::new(),
            genres: Vec::new(),
        }
    }

    fn row(title: &str, platform: &str, rom_id: Option<i64>) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: platform.to_string(),
            rom_id,
            ..Default::default()
        }
    }

    fn roms(entries: Vec<(i64, Vec<GameSummary>)>) -> HashMap<i64, Vec<GameSummary>> {
        entries.into_iter().collect()
    }

    fn link(title: &str, platform: &str, rom_id: i64) -> RelinkMatch {
        RelinkMatch {
            title: title.to_string(),
            platform: platform.to_string(),
            rom_id,
        }
    }

    #[test]
    fn a_unique_match_links() {
        let rows = vec![row(
            "Super Mario World",
            "Super Nintendo Entertainment System",
            None,
        )];
        let server = roms(vec![(
            1,
            vec![
                rom(101, "Super Mario World", 1),
                rom(103, "Secret of Mana", 1),
            ],
        )]);
        assert_eq!(
            match_unlinked(&rows, &[snes()], &server),
            vec![link(
                "Super Mario World",
                "Super Nintendo Entertainment System",
                101
            )]
        );
    }

    #[test]
    fn two_candidates_link_nothing() {
        let rows = vec![row("Tetris", "snes", None)];
        let server = roms(vec![(1, vec![rom(1, "Tetris", 1), rom(2, "TETRIS", 1)])]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn no_candidate_links_nothing() {
        let rows = vec![row("Mystery Game", "snes", None)];
        let server = roms(vec![(1, vec![rom(101, "Super Mario World", 1)])]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn the_row_platform_matches_the_slug_the_name_the_custom_and_the_display_name() {
        let server = roms(vec![(3, vec![rom(302, "Win9x Game", 3)])]);
        for alias in ["win", "Windows", "Windows 9x", " windows-9x "] {
            let rows = vec![row("Win9x Game", alias, None)];
            assert_eq!(
                match_unlinked(&rows, &[snes(), windows()], &server),
                vec![link("Win9x Game", alias, 302)],
                "row platform {alias:?}"
            );
        }
    }

    #[test]
    fn a_rom_on_another_platform_does_not_match() {
        let rows = vec![row("Win9x Game", "snes", None)];
        let server = roms(vec![(3, vec![rom(302, "Win9x Game", 3)])]);
        assert!(match_unlinked(&rows, &[snes(), windows()], &server).is_empty());
    }

    #[test]
    fn titles_compare_casefolded_without_punctuation() {
        let rows = vec![
            row("chrono-trigger", "SNES", None),
            row("  SECRET OF MANA! ", "SNES", None),
        ];
        let server = roms(vec![(
            1,
            vec![rom(102, "Chrono Trigger", 1), rom(103, "Secret of Mana", 1)],
        )]);
        assert_eq!(
            match_unlinked(&rows, &[snes()], &server),
            vec![
                link("chrono-trigger", "SNES", 102),
                link("  SECRET OF MANA! ", "SNES", 103)
            ]
        );
    }

    #[test]
    fn an_id_another_row_holds_links_nothing() {
        let rows = vec![
            row("Super Mario World", "SNES", Some(101)),
            row("Super Mario World!", "SNES", None),
        ];
        let server = roms(vec![(1, vec![rom(101, "Super Mario World", 1)])]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn two_unlinked_rows_claiming_one_rom_link_neither() {
        let rows = vec![
            row("Super Mario World", "SNES", None),
            row(
                "super mario world",
                "Super Nintendo Entertainment System",
                None,
            ),
        ];
        let server = roms(vec![(1, vec![rom(101, "Super Mario World", 1)])]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn already_linked_rows_are_ignored() {
        // The linked row's title matches a DIFFERENT rom: it must not move.
        let rows = vec![row("Super Mario World", "SNES", Some(999))];
        let server = roms(vec![(1, vec![rom(101, "Super Mario World", 1)])]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn a_blank_title_or_platform_links_nothing() {
        let rows = vec![row("", "SNES", None), row("Super Mario World", "  ", None)];
        let server = roms(vec![(
            1,
            vec![rom(101, "Super Mario World", 1), rom(5, "", 1)],
        )]);
        assert!(match_unlinked(&rows, &[snes()], &server).is_empty());
    }

    #[test]
    fn platforms_to_fetch_lists_only_platforms_with_unlinked_rows() {
        let rows = vec![
            row("A", "SNES", Some(1)),
            row("B", "Windows 9x", None),
            row("C", "win", None),
            row("D", "Nintendo 64", None),
        ];
        assert_eq!(platforms_to_fetch(&rows, &[snes(), windows()]), vec![3]);
        assert!(platforms_to_fetch(&rows[..1], &[snes(), windows()]).is_empty());
    }
}
