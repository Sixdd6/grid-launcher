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

    def load_from_disk(self, path: "Path", max_age: int | None = None) -> None:
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
                    if max_age is not None and time.time() - entry["timestamp"] > max_age:
                        continue
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


def fetch_short_games(
    base_url: str,
    api_token: str,
    limit: int = 20,
) -> tuple[list[dict[str, Any]], list[str]]:
    try:
        params: dict[str, Any] = {
            "limit": 100,
            "with_char_index": "false",
            "with_filter_values": "true",
            "metadata_providers": ["hltb"],
        }
        response = api_get_json(base_url, api_token, "/api/roms", params)

        if not isinstance(response, dict):
            return [], []
        raw_items = [i for i in (response.get("items") or []) if isinstance(i, dict)]
        genres = _extract_genres_from_response(response)

        _HLTB_SHORT_THRESHOLD = 1200
        short_raw: list[dict[str, Any]] = []
        other_raw: list[dict[str, Any]] = []
        for item in raw_items:
            hltb = item.get("hltb_metadata") or {}
            main_story = hltb.get("main_story") or 0
            if isinstance(main_story, (int, float)) and 0 < main_story <= _HLTB_SHORT_THRESHOLD:
                short_raw.append(item)
            else:
                other_raw.append(item)

        import random as _random
        _random.shuffle(short_raw)
        _random.shuffle(other_raw)
        combined = short_raw + other_raw
        normalized = [normalize_discover_item(i) for i in combined[:limit * 3]]
        return normalized[:limit], genres
    except Exception:
        return [], []


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


def client_filter_games(
    games: list[dict[str, Any]],
    genres: set[str],
    platforms: set[str],
) -> list[dict[str, Any]]:
    if not genres and not platforms:
        return games
    genre_tokens = {g.lower() for g in genres}
    platform_tokens = {p.lower() for p in platforms}
    result = []
    for game in games:
        if not isinstance(game, dict):
            continue
        if genre_tokens:
            game_genres = (game.get("genres") or "").lower()
            if not any(token in game_genres for token in genre_tokens):
                continue
        if platform_tokens:
            game_platform = (game.get("platform") or "").lower()
            if game_platform not in platform_tokens:
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


def genre_stats_from_games(
    all_games: list[dict[str, Any]],
    installed_games: list[dict[str, Any]],
) -> dict[str, tuple[int, int]]:
    total_counts: dict[str, int] = {}
    installed_counts: dict[str, int] = {}

    for game in all_games:
        if not isinstance(game, dict):
            continue
        for g in (game.get("genres") or "").split(","):
            g = g.strip()
            if g:
                total_counts[g] = total_counts.get(g, 0) + 1

    for game in installed_games:
        if not isinstance(game, dict):
            continue
        for g in (game.get("genres") or "").split(","):
            g = g.strip()
            if g:
                installed_counts[g] = installed_counts.get(g, 0) + 1

    return {
        genre: (total_counts.get(genre, 0), installed_counts.get(genre, 0))
        for genre in total_counts
    }


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


def fetch_recommendations(
    base_url: str,
    api_token: str,
    library_games: list[dict[str, Any]],
    installed_keys: set[str],
    limit: int = 20,
    preferred_platforms: set[str] | None = None,
) -> list[dict[str, Any]]:
    try:
        genre_counts: dict[str, int] = {}
        for game in library_games:
            if not isinstance(game, dict):
                continue
            for g in (game.get("genres") or "").split(","):
                g = g.strip()
                if g:
                    genre_counts[g] = genre_counts.get(g, 0) + 1

        if not genre_counts:
            return []

        top_genres = [
            k for k, _ in sorted(genre_counts.items(), key=lambda x: x[1], reverse=True)
        ][:3]

        deduped: dict[str, dict[str, Any]] = {}
        for genre in top_genres:
            for game in fetch_games_by_genre(base_url, api_token, genre, limit=limit):
                deduped.setdefault(game["rom_id"], game)

        all_candidates = list(deduped.values())

        if preferred_platforms:
            all_candidates = [
                g for g in all_candidates
                if g.get("platform", "").strip().lower() in {p.strip().lower() for p in preferred_platforms}
            ]

        filtered = filter_games_by_installed(all_candidates, installed_keys)
        return filtered[:limit]
    except Exception:
        return []


def record_discover_event(
    path: "Path",
    event: str,
    section_id: str,
    rom_id: str,
) -> None:
    import json as _json
    import time as _time
    from pathlib import Path as _Path
    try:
        p = _Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        if p.exists() and p.stat().st_size > 1_048_576:
            return
        line = _json.dumps({"event": event, "section_id": section_id, "rom_id": rom_id, "ts": _time.time()})
        with p.open("a", encoding="utf-8") as f:
            f.write(line + "\n")
    except Exception:
        pass


def load_watchlist(path: "Path") -> set[str]:
    import json as _json
    from pathlib import Path as _Path
    try:
        p = _Path(path)
        if not p.exists():
            return set()
        raw = _json.loads(p.read_text(encoding="utf-8"))
        if not isinstance(raw, list):
            return set()
        return {str(item) for item in raw if isinstance(item, str) and item}
    except Exception:
        return set()


def save_watchlist(path: "Path", rom_ids: set[str]) -> None:
    import json as _json
    from pathlib import Path as _Path
    try:
        p = _Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        tmp = p.with_suffix(".tmp")
        tmp.write_text(_json.dumps(sorted(rom_ids)), encoding="utf-8")
        tmp.replace(p)
    except Exception:
        pass


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
    """Fetch recently added games from the server, sorted by creation date."""
    items = _fetch_roms(
        base_url,
        api_token,
        {
            "order_by": "created_at",
            "order_dir": "desc",
            "limit": limit,
        },
    )
    return [normalize_discover_item(i) for i in items]

