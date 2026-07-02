"""Discover tab caching and API fetching logic."""

from __future__ import annotations

import time
from typing import Any

from ..core.api import api_get_json


class DiscoverCache:
    """In-memory cache for discover sections with TTL support."""

    def __init__(self, ttl: int = 3600) -> None:
        """Initialize cache.
        
        Args:
            ttl: Time to live for cache entries in seconds (default 1 hour)
        """
        self.ttl = ttl
        self.cache: dict[str, dict[str, Any]] = {}
        self.installed_game_keys: set[str] = set()
        self.installed_platform_names: set[str] = set()

    def set_installed_games(self, games: list[dict[str, Any]]) -> None:
        """Update set of installed game keys for filtering.
        
        Args:
            games: List of installed game records from config
        """
        self.installed_game_keys = {
            game.get("name", "").lower() for game in games
            if isinstance(game, dict) and game.get("name")
        }

    def set_installed_platform_names(self, library_games: list[dict[str, Any]]) -> None:
        """Update set of installed platform names for filtering unexplored platforms."""
        self.installed_platform_names = {
            game.get("platform", "").strip().lower()
            for game in library_games
            if isinstance(game, dict) and game.get("platform")
        }

    def get_section(self, section_id: str, force_refresh: bool = False) -> dict[str, Any] | None:
        """Get cached section data.
        
        Args:
            section_id: Section identifier (e.g., "trending", "new", "by_genre:action")
            force_refresh: If True, ignore cache
            
        Returns:
            Cached section data or None if not in cache or stale
        """
        if force_refresh:
            return None

        entry = self.cache.get(section_id)
        if entry is None:
            return None

        if time.time() - entry.get("timestamp", 0) > self.ttl:
            return None

        return entry.get("data")

    def set_section(self, section_id: str, data: dict[str, Any]) -> None:
        """Store section data in cache.
        
        Args:
            section_id: Section identifier
            data: Section data (games list, etc.)
        """
        self.cache[section_id] = {
            "data": data,
            "timestamp": time.time(),
        }

    def invalidate_section(self, section_id: str) -> None:
        """Remove a section from cache.
        
        Args:
            section_id: Section identifier to invalidate
        """
        self.cache.pop(section_id, None)

    def is_stale(self, section_id: str) -> bool:
        """Check if a section is stale or missing.
        
        Args:
            section_id: Section identifier
            
        Returns:
            True if section is stale or not in cache
        """
        entry = self.cache.get(section_id)
        if entry is None:
            return True
        return time.time() - entry.get("timestamp", 0) > self.ttl

    def clear(self) -> None:
        """Clear all cached sections."""
        self.cache.clear()

    def save_to_disk(self, path: "Path") -> None:
        """Persist cache to disk as JSON. Silently ignores all errors."""
        import json as _json
        from pathlib import Path as _Path
        try:
            p = _Path(path)
            p.parent.mkdir(parents=True, exist_ok=True)
            tmp = p.with_suffix(".tmp")
            tmp.write_text(_json.dumps(self.cache, default=str), encoding="utf-8")
            tmp.replace(p)
        except Exception:
            pass

    def load_from_disk(self, path: "Path") -> None:
        """Load cache from disk JSON. Silently ignores missing file or corrupt data."""
        import json as _json
        from pathlib import Path as _Path
        try:
            p = _Path(path)
            if not p.exists():
                return
            raw = _json.loads(p.read_text(encoding="utf-8"))
            if not isinstance(raw, dict):
                return
            for key, entry in raw.items():
                if (
                    isinstance(entry, dict)
                    and "data" in entry
                    and "timestamp" in entry
                    and isinstance(entry["timestamp"], (int, float))
                ):
                    self.cache.setdefault(key, entry)
        except Exception:
            pass


def _fetch_roms(
    base_url: str,
    api_token: str,
    params: dict[str, Any],
) -> list[dict[str, Any]]:
    """Call GET /api/roms and return raw items list. Returns [] on any error."""
    # Always disable extra response metadata we don't need
    params.setdefault("with_char_index", "false")
    params.setdefault("with_filter_values", "false")
    try:
        response = api_get_json(base_url, api_token, "/api/roms", params)
    except Exception:
        return []
    if not isinstance(response, dict):
        return []
    items = response.get("items") or []
    return [item for item in items if isinstance(item, dict)]


def _extract_genres_from_response(response: Any) -> list[str]:
    """Extract genre names from a /api/roms response with filter_values."""
    if not isinstance(response, dict):
        return []
    fv = response.get("filter_values") or {}
    if not isinstance(fv, dict):
        return []
    raw = fv.get("genres") or []
    result: list[str] = []
    for g in raw:
        if isinstance(g, str) and g:
            result.append(g)
        elif isinstance(g, dict):
            name = g.get("name") or ""
            if isinstance(name, str) and name:
                result.append(name)
    return sorted(result)[:15]


def normalize_discover_item(item: dict[str, Any]) -> dict[str, Any]:
    """Map a raw /api/roms item to the game dict format expected by make_game_card."""
    title = (item.get("name") or item.get("fs_name_no_ext") or "").strip()

    platform_raw = item.get("platform_display_name") or item.get("platform_slug") or ""
    platform = platform_raw.strip() if isinstance(platform_raw, str) else ""

    # Cover URL: try known field names in order of preference
    cover_url = ""
    for key in ("path_cover_large", "path_cover_small", "url_cover", "cover_url"):
        val = item.get(key)
        if val and isinstance(val, str):
            cover_url = val
            break

    genres_raw = item.get("genres") or []
    if isinstance(genres_raw, list):
        genres = ", ".join(
            (g.get("name", "") if isinstance(g, dict) else str(g))
            for g in genres_raw if g
        )
    elif isinstance(genres_raw, str):
        genres = genres_raw
    else:
        genres = ""

    rating_raw = item.get("rating") or item.get("average_rating") or 0
    rating = str(rating_raw) if rating_raw else ""

    return {
        "title": title,
        "platform": platform,
        "cover_url": cover_url,
        "screenshot_urls": "",
        "rom_id": str(item.get("id", "")),
        "genres": genres,
        "rating": rating,
        "description": str(item.get("summary") or item.get("description") or ""),
        "regions": "",
        "languages": "",
        "companies": "",
        "release_year": str(item.get("first_release_date") or ""),
        "filesize_bytes": str(item.get("total_filesize") or ""),
        "revision": "",
        "tags": "",
        "fanart_url": "",
        "first_release_date": str(item.get("first_release_date") or ""),
        "server_updated_at": "",
        "rom_file_name": "",
        "rom_nested_file_name": "",
        "rom_base_file_id": "",
        "ra_id": str(item.get("ra_id") or ""),
        "ps4_has_update": "false",
        "ps4_has_dlc": "false",
        "ps4_file_ids_by_category": "{}",
        "xbox360_has_update": "false",
        "xbox360_has_dlc": "false",
        "xbox360_file_ids_by_category": "{}",
        "update_available": "false",
    }


def fetch_all_games(
    base_url: str,
    api_token: str,
    limit: int = 20,
) -> tuple[list[dict[str, Any]], list[str]]:
    """Fetch games and available genres in one request."""
    params: dict[str, Any] = {
        "limit": limit,
        "with_char_index": "false",
        "with_filter_values": "true",
    }
    try:
        response = api_get_json(base_url, api_token, "/api/roms", params)
    except Exception:
        return [], []

    if not isinstance(response, dict):
        return [], []

    items = [item for item in (response.get("items") or []) if isinstance(item, dict)]
    genres = _extract_genres_from_response(response)
    return [normalize_discover_item(i) for i in items], genres


def fetch_games_by_genre(
    base_url: str,
    api_token: str,
    genre: str,
    limit: int = 15,
) -> list[dict[str, Any]]:
    """Fetch games filtered by genre."""
    items = _fetch_roms(base_url, api_token, {"genres": [genre], "limit": limit})
    return [normalize_discover_item(i) for i in items]


def filter_games_by_installed(
    games: list[dict[str, Any]],
    installed_keys: set[str],
) -> list[dict[str, Any]]:
    """Exclude games whose title exactly matches an installed game."""
    result = []
    for game in games:
        if not isinstance(game, dict):
            continue
        title = (game.get("title") or game.get("name") or "").lower()
        if title and title in installed_keys:
            continue
        result.append(game)
    return result


def get_top_genres_from_games(
    all_games: list[dict[str, Any]],
    installed_games: list[dict[str, Any]],
    top_n: int = 5,
) -> list[str]:
    """Get top genres from all games, weighted by rating."""
    genre_scores: dict[str, float] = {}
    installed_genres: set[str] = set()

    for game in installed_games:
        if isinstance(game, dict):
            for g in (game.get("genres") or "").split(","):
                installed_genres.add(g.strip())

    for game in all_games:
        if not isinstance(game, dict):
            continue
        rating = game.get("rating", 0)
        if not isinstance(rating, (int, float)):
            rating = 0
        for g in (game.get("genres") or "").split(","):
            g = g.strip()
            if g:
                weight = float(rating)
                if g not in installed_genres:
                    weight *= 1.5
                genre_scores[g] = genre_scores.get(g, 0) + weight

    return [k for k, _ in sorted(genre_scores.items(), key=lambda x: x[1], reverse=True)][:top_n]


def fetch_server_platforms(base_url: str, api_token: str) -> list[dict[str, Any]]:
    """Fetch all platforms from the server. Returns [] on any error."""
    try:
        response = api_get_json(base_url, api_token, "/api/platforms", None)
    except Exception:
        return []
    if not isinstance(response, list):
        return []
    return [p for p in response if isinstance(p, dict)]


def filter_unexplored_platforms(
    platforms: list[dict[str, Any]],
    installed_platform_names: set[str],
    max_platforms: int = 3,
) -> list[dict[str, Any]]:
    """Return up to max_platforms platforms not in installed_platform_names, sorted by rom_count desc."""
    result = []
    for p in platforms:
        if not isinstance(p, dict):
            continue
        if not p.get("rom_count", 0):
            continue
        display = (p.get("display_name") or p.get("name") or "").strip().lower()
        name = (p.get("name") or "").strip().lower()
        if display in installed_platform_names or name in installed_platform_names:
            continue
        result.append(p)
    result.sort(key=lambda p: p.get("rom_count", 0), reverse=True)
    return result[:max_platforms]


def fetch_highly_rated_games(
    base_url: str,
    api_token: str,
    limit: int = 20,
) -> list[dict[str, Any]]:
    """Fetch highest rated games from the server."""
    items = _fetch_roms(
        base_url,
        api_token,
        {
            "order_by": "average_rating",
            "order_dir": "desc",
            "limit": limit,
        },
    )
    return [normalize_discover_item(i) for i in items]


def fetch_games_by_platform(
    base_url: str,
    api_token: str,
    platform_id: int,
    limit: int = 8,
) -> list[dict[str, Any]]:
    """Fetch games for a specific platform, sorted by average rating."""
    items = _fetch_roms(
        base_url,
        api_token,
        {
            "platform_ids": [platform_id],
            "order_by": "average_rating",
            "order_dir": "desc",
            "limit": limit,
        },
    )
    return [normalize_discover_item(i) for i in items]


# ---------------------------------------------------------------------------
# Legacy aliases kept so any remaining callers don't break at import time
# ---------------------------------------------------------------------------

def fetch_trending_games(
    base_url: str,
    api_token: str,
    limit: int = 20,
    min_rating: float = 3.5,
) -> tuple[list[dict[str, Any]], list[str]]:
    """Deprecated — delegates to fetch_all_games."""
    return fetch_all_games(base_url, api_token, limit=limit)


def fetch_new_games(
    base_url: str,
    api_token: str,
    limit: int = 20,
) -> list[dict[str, Any]]:
    """Deprecated — delegates to fetch_all_games."""
    games, _ = fetch_all_games(base_url, api_token, limit=limit)
    return games

