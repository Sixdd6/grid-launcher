//! RetroAchievements data as RomM serves it, and the Details "Achievements"
//! view model built from it.
//!
//! Sources (openapi `DetailedRomSchema`, `UserSchema`):
//! - the ROM detail's `ra_id` and `merged_ra_metadata.achievements[]`
//!   (`RAGameRomAchievement`, every field nullable);
//! - `GET /api/users/me`'s `ra_username` (`""` when unset) and
//!   `ra_progression` (`{}` when the account has none) with
//!   `results[] (RAUserGameProgression)` and their `earned_achievements[]`.
//!
//! The join: the ROM's `ra_id` equals a progression's `rom_ra_id`; an
//! achievement's `ra_id` equals an earned entry's `id` (a string on the wire).
//!
//! Also the RomM side of the RA username (user ruling U6): GRID sets
//! `ra_username` on the RomM account when the user saves it in Settings. Only
//! the username crosses to RomM, never the RA token.

use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Deserializer, Serialize};

use super::error::excerpt;
use super::{RommClient, RommError};

/// Decodes a nullable list leniently: `null` or a missing field is empty, and
/// an element that does not fit `T` is dropped instead of failing the page.
fn lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let raw = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(raw
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// One `RAGameRomAchievement`. Every field is nullable on the wire.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct RaAchievement {
    #[serde(default)]
    pub ra_id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub points: Option<i64>,
    #[serde(default)]
    pub num_awarded: Option<i64>,
    #[serde(default)]
    pub num_awarded_hardcore: Option<i64>,
    #[serde(default)]
    pub badge_id: Option<String>,
    /// External (media.retroachievements.org). The image host filter drops it.
    #[serde(default)]
    pub badge_url: Option<String>,
    #[serde(default)]
    pub badge_url_lock: Option<String>,
    /// Served by RomM itself (public static mount).
    #[serde(default)]
    pub badge_path: Option<String>,
    #[serde(default)]
    pub badge_path_lock: Option<String>,
    #[serde(default)]
    pub display_order: Option<i64>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

/// `RomRAMetadata` — only the achievement list is read.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RaRomMetadata {
    #[serde(default, deserialize_with = "lenient_vec")]
    pub achievements: Vec<RaAchievement>,
}

/// The RA part of `DetailedRomSchema`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RomRa {
    #[serde(default)]
    pub ra_id: Option<i64>,
    #[serde(default)]
    pub merged_ra_metadata: Option<RaRomMetadata>,
}

impl RomRa {
    pub fn achievements(&self) -> &[RaAchievement] {
        self.merged_ra_metadata
            .as_ref()
            .map(|m| m.achievements.as_slice())
            .unwrap_or_default()
    }
}

/// `EarnedAchievement`: `id` is the achievement's RA id as a string.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RaEarned {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub date_hardcore: Option<String>,
}

/// `RAUserGameProgression`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RaGameProgression {
    #[serde(default)]
    pub rom_ra_id: Option<i64>,
    #[serde(default)]
    pub max_possible: Option<i64>,
    #[serde(default)]
    pub num_awarded: Option<i64>,
    #[serde(default)]
    pub num_awarded_hardcore: Option<i64>,
    #[serde(default, deserialize_with = "lenient_vec")]
    pub earned_achievements: Vec<RaEarned>,
}

/// `RAProgression`. RomM sends `{}` for an account with no RA link.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RaProgression {
    #[serde(default)]
    pub total: Option<i64>,
    #[serde(default, deserialize_with = "lenient_vec")]
    pub results: Vec<RaGameProgression>,
}

/// The RA part of `UserSchema` (`GET /api/users/me`).
#[derive(Debug, Clone, Deserialize)]
pub struct RaUser {
    pub id: i64,
    #[serde(default)]
    pub ra_username: Option<String>,
    #[serde(default)]
    pub ra_progression: Option<RaProgression>,
}

impl RaUser {
    /// The RomM account's RA username; `""` (RomM's "unset") reads as `None`.
    pub fn ra_username(&self) -> Option<&str> {
        self.ra_username
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// One Achievements tab row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AchievementRow {
    pub ra_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub points: i64,
    /// Resolved, host-filtered badge URL for the row's state (the locked
    /// badge while not earned). `""` when no RomM-served badge exists.
    pub badge_url: String,
    pub earned: bool,
    /// The unlock date as RomM states it; `None` while locked.
    pub unlocked_at: Option<String>,
    /// Earned in hardcore mode.
    pub hardcore: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AchievementsSummary {
    pub total: usize,
    pub earned: usize,
    pub hardcore: usize,
    pub points_total: i64,
    pub points_earned: i64,
}

/// What `get_achievements` returns. No secret is in it: the RA username on
/// RomM is reduced to a flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AchievementsView {
    /// The RomM account has an RA username, so unlock state is known.
    pub progress_known: bool,
    pub summary: AchievementsSummary,
    pub rows: Vec<AchievementRow>,
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// Builds the Achievements view. `resolver` turns a badge path or URL into a
/// fetchable, host-filtered URL (`images::urls::server_resolver`); an empty
/// result means "no badge".
///
/// Order: earned first, most recent unlock first; then the rest by
/// `display_order` (missing last), then by RA id.
pub fn build_view(
    rom: &RomRa,
    user: &RaUser,
    resolver: &dyn Fn(&str) -> String,
) -> AchievementsView {
    let progress_known = user.ra_username().is_some();
    let progression = match (progress_known, rom.ra_id, user.ra_progression.as_ref()) {
        (true, Some(rom_ra_id), Some(p)) => {
            p.results.iter().find(|g| g.rom_ra_id == Some(rom_ra_id))
        }
        _ => None,
    };
    let earned_for = |ra_id: Option<i64>| -> Option<&RaEarned> {
        let id = ra_id?;
        progression?
            .earned_achievements
            .iter()
            .find(|e| e.id.as_deref().and_then(|s| s.trim().parse::<i64>().ok()) == Some(id))
    };
    let badge = |first: Option<&str>, second: Option<&str>| -> String {
        [first, second]
            .into_iter()
            .filter_map(non_empty)
            .map(resolver)
            .find(|u| !u.is_empty())
            .unwrap_or_default()
    };

    let mut rows: Vec<(Option<i64>, AchievementRow)> = rom
        .achievements()
        .iter()
        .map(|a| {
            let earned = earned_for(a.ra_id);
            let unlocked_at = earned.and_then(|e| {
                non_empty(e.date.as_deref())
                    .or(non_empty(e.date_hardcore.as_deref()))
                    .map(str::to_string)
            });
            let is_earned = earned.is_some();
            let hardcore = earned.is_some_and(|e| non_empty(e.date_hardcore.as_deref()).is_some());
            // A locked row shows the lock badge; without one (or without
            // progress) the normal badge stands in.
            let unlocked_badge = badge(a.badge_path.as_deref(), a.badge_url.as_deref());
            let badge_url = if is_earned || !progress_known {
                unlocked_badge
            } else {
                let locked = badge(a.badge_path_lock.as_deref(), a.badge_url_lock.as_deref());
                if locked.is_empty() {
                    unlocked_badge
                } else {
                    locked
                }
            };
            (
                a.display_order,
                AchievementRow {
                    ra_id: a.ra_id,
                    title: a.title.clone().unwrap_or_default(),
                    description: a.description.clone().unwrap_or_default(),
                    points: a.points.unwrap_or(0).max(0),
                    badge_url,
                    earned: is_earned,
                    unlocked_at,
                    hardcore,
                },
            )
        })
        .collect();

    rows.sort_by(|(order_a, a), (order_b, b)| {
        b.earned
            .cmp(&a.earned)
            .then_with(|| {
                if a.earned {
                    b.unlocked_at.cmp(&a.unlocked_at)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| match (order_a, order_b) {
                (Some(x), Some(y)) => x.cmp(y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.ra_id.cmp(&b.ra_id))
    });
    let rows: Vec<AchievementRow> = rows.into_iter().map(|(_, r)| r).collect();

    let summary = AchievementsSummary {
        total: rows.len(),
        earned: rows.iter().filter(|r| r.earned).count(),
        hardcore: rows.iter().filter(|r| r.hardcore).count(),
        points_total: rows.iter().map(|r| r.points).sum(),
        points_earned: rows.iter().filter(|r| r.earned).map(|r| r.points).sum(),
    };
    AchievementsView {
        progress_known,
        summary,
        rows,
    }
}

/// The RA username to send to RomM, or `None` when nothing must be sent:
/// the local name is blank (RomM ignores an empty value, and GRID never tries
/// to clear it) or RomM already holds exactly this name.
pub fn ra_username_to_push<'a>(local: &'a str, on_server: Option<&str>) -> Option<&'a str> {
    let local = local.trim();
    if local.is_empty() {
        return None;
    }
    if on_server.map(str::trim) == Some(local) {
        return None;
    }
    Some(local)
}

/// The `application/x-www-form-urlencoded` body of `PUT /api/users/{id}`
/// that changes only `ra_username`.
pub fn ra_username_form_body(name: &str) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("ra_username", name)
        .finish()
}

/// What happened to the RomM side of an RA username save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RaUsernameSync {
    /// Nothing to send: no local username.
    Skipped,
    /// RomM already has this username.
    Unchanged,
    /// RomM now has the username. `refreshed` is false when the follow-up
    /// progress refresh failed (expected for a name RA does not know).
    Updated { refreshed: bool },
    /// The RomM update failed; the local save stands.
    Failed { error: String },
    /// No live RomM session, so nothing was sent. Set by the caller, which
    /// owns the session; `sync_ra_username` itself never returns it.
    NotConnected,
}

impl RommClient {
    /// `GET /api/users/me`, decoded for its RA fields.
    pub async fn me_ra(&self) -> Result<RaUser, RommError> {
        self.get_json("/api/users/me", &[]).await
    }

    /// `GET /api/roms/{id}`, decoded for its RA fields.
    pub async fn rom_ra(&self, rom_id: i64) -> Result<RomRa, RommError> {
        self.get_json(&format!("/api/roms/{rom_id}"), &[]).await
    }

    async fn checked(&self, request: reqwest::RequestBuilder) -> Result<(), RommError> {
        let resp = request
            .header(AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if let Some(e) = self.auth_error(status) {
            return Err(e);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RommError::Http {
                status: status.as_u16(),
                excerpt: excerpt(&body),
            });
        }
        Ok(())
    }

    /// `PUT /api/users/{id}` with only `ra_username` in the form body.
    pub async fn set_ra_username(&self, user_id: i64, name: &str) -> Result<(), RommError> {
        let request = self
            .http
            .put(self.endpoint(&format!("/api/users/{user_id}"))?)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(ra_username_form_body(name));
        self.checked(request).await
    }

    /// `POST /api/users/{id}/ra/refresh {"incremental": true}`.
    pub async fn refresh_ra_progress(&self, user_id: i64) -> Result<(), RommError> {
        let request = self
            .http
            .post(self.endpoint(&format!("/api/users/{user_id}/ra/refresh"))?)
            .json(&serde_json::json!({ "incremental": true }));
        self.checked(request).await
    }

    /// The whole U6 step: read the RomM account, set `ra_username` when it
    /// differs, then ask RomM to refresh the progress (best effort). Never
    /// fails: the outcome says what happened. Logs carry no secret — only the
    /// error text, which never holds the request or its headers.
    pub async fn sync_ra_username(&self, local: &str) -> RaUsernameSync {
        if local.trim().is_empty() {
            return RaUsernameSync::Skipped;
        }
        let user = match self.me_ra().await {
            Ok(user) => user,
            Err(e) => {
                return RaUsernameSync::Failed {
                    error: e.to_string(),
                }
            }
        };
        let Some(name) = ra_username_to_push(local, user.ra_username()) else {
            return RaUsernameSync::Unchanged;
        };
        if let Err(e) = self.set_ra_username(user.id, name).await {
            tracing::warn!("setting the RetroAchievements username on RomM failed: {e}");
            return RaUsernameSync::Failed {
                error: e.to_string(),
            };
        }
        let refreshed = match self.refresh_ra_progress(user.id).await {
            Ok(()) => true,
            Err(e) => {
                tracing::info!("RomM could not refresh RetroAchievements progress: {e}");
                false
            }
        };
        RaUsernameSync::Updated { refreshed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolver(value: &str) -> String {
        crate::images::urls::server_resolver("https://romm.example")(value)
    }

    /// Shaped like the live ROM 194 probe: `ra_id` set, three achievements,
    /// badge paths served by RomM, badge URLs on RA's own host.
    fn rom_with_achievements() -> RomRa {
        serde_json::from_value(json!({
            "id": 194,
            "ra_id": 5001,
            "merged_ra_metadata": {
                "first_release_date": null,
                "genres": [],
                "companies": [],
                "achievements": [
                    {
                        "ra_id": 11, "title": "First Steps", "description": "Finish 1-1",
                        "points": 5, "num_awarded": 100, "num_awarded_hardcore": 40,
                        "badge_id": "1001",
                        "badge_url": "https://media.retroachievements.org/Badge/1001.png",
                        "badge_url_lock": "https://media.retroachievements.org/Badge/1001_lock.png",
                        "badge_path": "roms/9/194/badges/1001.png",
                        "badge_path_lock": "roms/9/194/badges/1001_lock.png",
                        "display_order": 2, "type": "progression"
                    },
                    {
                        "ra_id": 12, "title": "Boss", "description": "Beat the boss",
                        "points": 25, "num_awarded": null, "num_awarded_hardcore": null,
                        "badge_id": "1002",
                        "badge_url": "https://media.retroachievements.org/Badge/1002.png",
                        "badge_url_lock": null,
                        "badge_path": "roms/9/194/badges/1002.png",
                        "badge_path_lock": null,
                        "display_order": 1, "type": "win_condition"
                    },
                    {
                        "ra_id": 13, "title": "Collector", "description": null,
                        "points": 10, "num_awarded": 3, "num_awarded_hardcore": 1,
                        "badge_id": "1003",
                        "badge_url": "https://media.retroachievements.org/Badge/1003.png",
                        "badge_url_lock": "https://media.retroachievements.org/Badge/1003_lock.png",
                        "badge_path": null,
                        "badge_path_lock": null,
                        "display_order": null, "type": null
                    },
                    {
                        "ra_id": 14, "title": "Speedrun", "description": "Fast",
                        "points": 50, "num_awarded": 1, "num_awarded_hardcore": 0,
                        "badge_id": "1004",
                        "badge_url": null, "badge_url_lock": null,
                        "badge_path": "roms/9/194/badges/1004.png",
                        "badge_path_lock": "roms/9/194/badges/1004_lock.png",
                        "display_order": 3, "type": null
                    }
                ]
            }
        }))
        .expect("rom decodes")
    }

    fn user_unset() -> RaUser {
        serde_json::from_value(json!({
            "id": 7, "username": "tester", "ra_username": "", "ra_progression": {}
        }))
        .expect("user decodes")
    }

    fn user_with_progress() -> RaUser {
        serde_json::from_value(json!({
            "id": 7,
            "username": "tester",
            "ra_username": "fake-ra-player",
            "ra_progression": {
                "total": 2,
                "results": [
                    {
                        "rom_ra_id": 999, "max_possible": 1, "num_awarded": 1,
                        "num_awarded_hardcore": 0,
                        "earned_achievements": [{ "id": "11", "date": "2020-01-01 00:00:00" }]
                    },
                    {
                        "rom_ra_id": 5001, "max_possible": 4, "num_awarded": 2,
                        "num_awarded_hardcore": 1,
                        "most_recent_awarded_date": "2024-03-02 10:00:00",
                        "highest_award_kind": null,
                        "earned_achievements": [
                            { "id": "11", "date": "2024-01-05 09:00:00" },
                            { "id": "14", "date": "2024-03-02 10:00:00",
                              "date_hardcore": "2024-03-02 10:00:00" }
                        ]
                    }
                ]
            }
        }))
        .expect("user decodes")
    }

    #[test]
    fn an_openapi_shaped_rom_decodes_with_all_nullable_fields() {
        let rom = rom_with_achievements();
        assert_eq!(rom.ra_id, Some(5001));
        assert_eq!(rom.achievements().len(), 4);
        let collector = &rom.achievements()[2];
        assert_eq!(collector.description, None);
        assert_eq!(collector.display_order, None);
        assert_eq!(collector.kind, None);
        assert_eq!(rom.achievements()[1].kind.as_deref(), Some("win_condition"));
    }

    #[test]
    fn a_rom_with_an_ra_id_but_no_achievements_has_an_empty_list() {
        for payload in [
            json!({ "id": 1, "ra_id": 77, "merged_ra_metadata": { "achievements": [] } }),
            json!({ "id": 1, "ra_id": 77, "merged_ra_metadata": null }),
            json!({ "id": 1, "ra_id": null }),
            json!({ "id": 1, "ra_id": 77, "merged_ra_metadata": { "achievements": null } }),
        ] {
            let rom: RomRa = serde_json::from_value(payload).expect("rom decodes");
            assert!(rom.achievements().is_empty());
        }
    }

    #[test]
    fn a_malformed_achievement_is_dropped_not_fatal() {
        let rom: RomRa = serde_json::from_value(json!({
            "ra_id": 1,
            "merged_ra_metadata": { "achievements": [null, "x", { "ra_id": 2, "title": "Ok" }] }
        }))
        .expect("rom decodes");
        assert_eq!(rom.achievements().len(), 1);
        assert_eq!(rom.achievements()[0].title.as_deref(), Some("Ok"));
    }

    #[test]
    fn an_unset_user_reads_empty_string_as_none_and_empty_object_as_no_results() {
        let user = user_unset();
        assert_eq!(user.ra_username(), None);
        assert!(user.ra_progression.expect("an object").results.is_empty());
        let null_user: RaUser = serde_json::from_value(json!({
            "id": 7, "ra_username": null, "ra_progression": null
        }))
        .expect("decodes");
        assert_eq!(null_user.ra_username(), None);
        assert!(null_user.ra_progression.is_none());
        let bare: RaUser = serde_json::from_value(json!({ "id": 7, "username": "x" })).unwrap();
        assert_eq!(bare.ra_username(), None);
    }

    #[test]
    fn a_user_with_progress_decodes_earned_ids_as_strings() {
        let user = user_with_progress();
        assert_eq!(user.ra_username(), Some("fake-ra-player"));
        let game = &user.ra_progression.as_ref().unwrap().results[1];
        assert_eq!(game.rom_ra_id, Some(5001));
        assert_eq!(game.earned_achievements[1].id.as_deref(), Some("14"));
        assert_eq!(
            game.earned_achievements[1].date_hardcore.as_deref(),
            Some("2024-03-02 10:00:00")
        );
    }

    #[test]
    fn the_join_puts_earned_first_newest_first_then_locked_by_display_order() {
        let view = build_view(&rom_with_achievements(), &user_with_progress(), &resolver);
        let order: Vec<Option<i64>> = view.rows.iter().map(|r| r.ra_id).collect();
        // 14 (2024-03) and 11 (2024-01) earned; 12 (order 1) then 13 (no order).
        assert_eq!(order, vec![Some(14), Some(11), Some(12), Some(13)]);
        assert!(view.progress_known);
        let speedrun = &view.rows[0];
        assert!(speedrun.earned && speedrun.hardcore);
        assert_eq!(speedrun.unlocked_at.as_deref(), Some("2024-03-02 10:00:00"));
        let first = &view.rows[1];
        assert!(first.earned && !first.hardcore);
        assert_eq!(first.unlocked_at.as_deref(), Some("2024-01-05 09:00:00"));
        assert!(!view.rows[2].earned);
        assert_eq!(view.rows[2].unlocked_at, None);
    }

    #[test]
    fn the_join_ignores_another_games_progression() {
        // rom_ra_id 999 also lists "11" as earned; it must not leak in.
        let mut user = user_with_progress();
        user.ra_progression.as_mut().unwrap().results.remove(1);
        let view = build_view(&rom_with_achievements(), &user, &resolver);
        assert!(view.progress_known);
        assert_eq!(view.summary.earned, 0);
        assert!(view.rows.iter().all(|r| !r.earned));
    }

    #[test]
    fn the_summary_counts_earned_points_and_hardcore() {
        let view = build_view(&rom_with_achievements(), &user_with_progress(), &resolver);
        assert_eq!(
            view.summary,
            AchievementsSummary {
                total: 4,
                earned: 2,
                hardcore: 1,
                points_total: 90,
                points_earned: 55,
            }
        );
    }

    #[test]
    fn without_an_ra_username_the_list_has_no_progress() {
        let mut user = user_with_progress();
        user.ra_username = Some("   ".into());
        for user in [user_unset(), user] {
            let view = build_view(&rom_with_achievements(), &user, &resolver);
            assert!(!view.progress_known);
            assert_eq!(view.summary.earned, 0);
            assert_eq!(view.summary.points_total, 90);
            // Locked order only: display_order 1, 2, 3, then none.
            let order: Vec<Option<i64>> = view.rows.iter().map(|r| r.ra_id).collect();
            assert_eq!(order, vec![Some(12), Some(11), Some(14), Some(13)]);
            // Neutral badges: the unlocked art, not the lock art.
            assert_eq!(
                view.rows[1].badge_url,
                "https://romm.example/assets/romm/resources/roms/9/194/badges/1001.png"
            );
        }
    }

    #[test]
    fn badges_come_from_romm_and_external_urls_are_dropped() {
        let view = build_view(&rom_with_achievements(), &user_with_progress(), &resolver);
        let by_id = |id: i64| view.rows.iter().find(|r| r.ra_id == Some(id)).unwrap();
        // Earned: the unlocked badge from RomM.
        assert_eq!(
            by_id(11).badge_url,
            "https://romm.example/assets/romm/resources/roms/9/194/badges/1001.png"
        );
        // Locked with no lock badge anywhere: the unlocked RomM badge stands in.
        assert_eq!(
            by_id(12).badge_url,
            "https://romm.example/assets/romm/resources/roms/9/194/badges/1002.png"
        );
        // Only RA-hosted URLs: the host filter drops them, so no badge.
        assert_eq!(by_id(13).badge_url, "");
    }

    #[test]
    fn a_locked_row_shows_the_lock_badge() {
        let mut user = user_with_progress();
        user.ra_progression.as_mut().unwrap().results[1]
            .earned_achievements
            .retain(|e| e.id.as_deref() != Some("14"));
        let view = build_view(&rom_with_achievements(), &user, &resolver);
        let speedrun = view.rows.iter().find(|r| r.ra_id == Some(14)).unwrap();
        assert!(!speedrun.earned);
        assert_eq!(
            speedrun.badge_url,
            "https://romm.example/assets/romm/resources/roms/9/194/badges/1004_lock.png"
        );
    }

    #[test]
    fn the_view_serializes_without_any_username() {
        let view = build_view(&rom_with_achievements(), &user_with_progress(), &resolver);
        let text = serde_json::to_string(&view).unwrap();
        assert!(!text.contains("fake-ra-player"));
    }

    #[test]
    fn the_push_decision_skips_blank_and_unchanged_names() {
        assert_eq!(ra_username_to_push("", None), None);
        assert_eq!(ra_username_to_push("   ", Some("someone")), None);
        assert_eq!(ra_username_to_push("player", Some("player")), None);
        assert_eq!(ra_username_to_push(" player ", Some("player")), None);
        assert_eq!(ra_username_to_push("player", None), Some("player"));
        assert_eq!(ra_username_to_push("player", Some("other")), Some("player"));
        assert_eq!(
            ra_username_to_push(" player ", Some("Player")),
            Some("player")
        );
    }

    #[test]
    fn the_form_body_carries_only_the_encoded_username() {
        assert_eq!(ra_username_form_body("player"), "ra_username=player");
        assert_eq!(ra_username_form_body("a b&c=d"), "ra_username=a+b%26c%3Dd");
    }
}
