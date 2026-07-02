from __future__ import annotations

import json
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import urlopen, Request

_RA_API_BASE = "https://retroachievements.org/API"
_RA_DOREQUEST_URL = "https://retroachievements.org/dorequest.php"


class RetroAchievementsError(Exception):
    pass


def _validated_ra_game_id(ra_game_id: int) -> int:
    game_id = int(ra_game_id)
    if game_id <= 0:
        raise ValueError("ra_game_id must be a positive integer")
    return game_id


def _fetch_json(url: str) -> dict:
    try:
        req = Request(url, headers={"User-Agent": "grid-launcher/1.0 (retroachievements-client)"})
        with urlopen(req, timeout=10) as response:
            raw = response.read().decode("utf-8")
        payload = json.loads(raw)
    except HTTPError as exc:
        try:
            body = exc.read().decode("utf-8", errors="replace")[:300]
        except Exception:
            body = ""
        detail = f"{body}" if body else str(exc)
        raise RetroAchievementsError(f"RetroAchievements HTTP {exc.code}: {detail}") from exc
    except (URLError, json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
        raise RetroAchievementsError(f"RetroAchievements request failed: {exc}") from exc

    if not isinstance(payload, dict):
        raise RetroAchievementsError("RetroAchievements response must be a JSON object")

    if ("Success" in payload and not payload.get("Success")) or payload.get("Error"):
        message = str(payload.get("Error") or payload.get("Message") or "RetroAchievements returned an error")
        raise RetroAchievementsError(message)

    return payload


def _normalize_achievement(achievement_id: str, achievement: dict, include_progress: bool) -> dict:
    normalized_id = achievement.get("ID") or achievement.get("AchievementID") or achievement_id
    date_earned = ""
    if include_progress:
        earned_raw = achievement.get("DateEarned")
        hardcore_raw = achievement.get("DateEarnedHardcore")
        earned_value = earned_raw if earned_raw and str(earned_raw).strip() not in ("", "0", "null", "None") else None
        if earned_value is None:
            earned_value = hardcore_raw if hardcore_raw and str(hardcore_raw).strip() not in ("", "0", "null", "None") else None
        date_earned = str(earned_value) if earned_value is not None else ""
    return {
        "id": int(normalized_id),
        "title": str(achievement.get("Title") or ""),
        "description": str(achievement.get("Description") or ""),
        "points": int(achievement.get("Points") or 0),
        "badge_name": str(achievement.get("BadgeName") or ""),
        "date_earned": date_earned,
    }


def ra_login(username: str, password: str) -> dict:
    if not isinstance(username, str) or not username.strip():
        raise ValueError("username must be a non-empty string")
    if not isinstance(password, str) or not password.strip():
        raise ValueError("password must be a non-empty string")

    url = f"{_RA_DOREQUEST_URL}?{urlencode({'r': 'login', 'u': username, 'p': password})}"
    data = _fetch_json(url)

    if data.get("Success") is True:
        user = data.get("User")
        token = data.get("Token")
        if not isinstance(user, str) or not user:
            raise RetroAchievementsError("RetroAchievements login response missing User")
        if not isinstance(token, str) or not token:
            raise RetroAchievementsError("RetroAchievements login response missing Token")
        return {
            "username": user,
            "token": token,
        }

    message = str(data.get("Error") or data.get("Message") or "Invalid credentials")
    raise RetroAchievementsError(message)


def fetch_game_achievements(ra_game_id: int, username: str, api_key: str) -> list[dict]:
    game_id = _validated_ra_game_id(ra_game_id)

    has_credentials = bool(username) and bool(api_key)
    if has_credentials:
        endpoint = "API_GetGameInfoAndUserProgress.php"
        params = {
            "u": username,
            "y": api_key,
            "g": game_id,
        }
    else:
        endpoint = "API_GetGameExtended.php"
        params = {
            "g": game_id,
        }

    url = f"{_RA_API_BASE}/{endpoint}?{urlencode(params)}"
    payload = _fetch_json(url)

    achievements_raw = payload.get("Achievements")
    if not isinstance(achievements_raw, dict) or not achievements_raw:
        return []

    achievements: list[dict] = []
    for achievement_id, achievement_data in achievements_raw.items():
        if not isinstance(achievement_data, dict):
            continue
        achievements.append(_normalize_achievement(str(achievement_id), achievement_data, has_credentials))
    return achievements


def search_ra_game(title: str, api_key: str) -> list[dict]:
    raise NotImplementedError("RA game search not implemented")


def resolve_ra_game_id(game: dict, username: str, api_key: str) -> int | None:
    value = game.get("ra_id")
    if value is None:
        return None
    text = str(value).strip()
    if not text:
        return None
    if text.isdigit():
        return int(text)
    return None
