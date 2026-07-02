from __future__ import annotations

import datetime
import re
from datetime import timezone
from typing import Any

SOURCE_PRIORITY: tuple[str, ...] = (
    "launchbox_metadata",
    "ss_metadata",
    "igdb_metadata",
    "moby_metadata",
)

_DESCRIPTION_KEYS: tuple[str, ...] = (
    "description",
    "summary",
    "overview",
    "synopsis",
    "plot",
)

_GENRE_KEYS: tuple[str, ...] = (
    "genres",
    "genre",
)

_REGION_KEYS: tuple[str, ...] = (
    "regions",
    "region",
    "countries",
    "country",
)

_RATING_KEYS: tuple[str, ...] = (
    "rating",
    "ratings",
    "user_rating",
    "community_rating",
    "avg_rating",
    "score",
    "total_rating",
    "aggregated_rating",
    "moby_score",
)

_NUMBER_PATTERN = re.compile(r"-?\d+(?:\.\d+)?")


def _clean_text(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    return value.strip()


def _iter_text_values(value: Any) -> list[str]:
    if isinstance(value, str):
        cleaned = value.strip()
        return [cleaned] if cleaned else []

    if isinstance(value, dict):
        values: list[str] = []
        for key in ("name", "value", "title", "label"):
            candidate = value.get(key)
            if isinstance(candidate, str) and candidate.strip():
                values.append(candidate.strip())
                break
        return values

    if isinstance(value, list):
        values: list[str] = []
        for entry in value:
            values.extend(_iter_text_values(entry))
        return values

    return []


def _dedupe(values: list[str]) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for value in values:
        normalized = value.casefold()
        if normalized in seen:
            continue
        seen.add(normalized)
        result.append(value)
    return result


def _source_field_text(source_data: dict[str, Any], keys: tuple[str, ...]) -> str:
    for key in keys:
        candidate = _clean_text(source_data.get(key))
        if candidate:
            return candidate
    return ""


def _source_field_list(source_data: dict[str, Any], keys: tuple[str, ...]) -> list[str]:
    values: list[str] = []
    for key in keys:
        raw_value = source_data.get(key)
        values.extend(_iter_text_values(raw_value))
    return _dedupe(values)


def _extract_numeric_rating(raw: Any) -> tuple[float, float] | None:
    if isinstance(raw, (int, float)) and not isinstance(raw, bool):
        value = float(raw)
        if value < 0:
            return None
        if value <= 5:
            return value, 5.0
        if value <= 10:
            return value, 10.0
        return value, 100.0

    if not isinstance(raw, str):
        return None

    text = raw.strip()
    if not text:
        return None

    slash_match = re.search(r"(-?\d+(?:\.\d+)?)\s*/\s*(-?\d+(?:\.\d+)?)", text)
    if slash_match:
        numerator = float(slash_match.group(1))
        denominator = float(slash_match.group(2))
        if numerator < 0 or denominator <= 0:
            return None
        return numerator, denominator

    percent_match = re.search(r"(-?\d+(?:\.\d+)?)\s*%", text)
    if percent_match:
        value = float(percent_match.group(1))
        if value < 0:
            return None
        return value, 100.0

    number_match = _NUMBER_PATTERN.search(text)
    if not number_match:
        return None

    value = float(number_match.group(0))
    if value < 0:
        return None

    if value <= 5:
        return value, 5.0
    if value <= 10:
        return value, 10.0
    return value, 100.0


def normalize_rating_to_five(raw: Any) -> float | None:
    extracted = _extract_numeric_rating(raw)
    if extracted is None:
        return None

    value, scale = extracted
    if scale <= 0:
        return None

    normalized = (value / scale) * 5.0
    if normalized < 0:
        return None
    if normalized > 5.0:
        normalized = 5.0
    return round(normalized, 1)


def format_rating_to_five(raw: Any) -> str:
    normalized = normalize_rating_to_five(raw)
    if normalized is None:
        return ""
    return f"{normalized:.1f}/5"


def _extract_year(value: Any) -> str:
    if isinstance(value, bool):
        return ""

    if isinstance(value, int):
        if value > 10000:
            try:
                year = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc).year
            except (OverflowError, OSError, ValueError):
                return ""
            return str(year) if 1900 <= year <= 2100 else ""
        if 1900 <= value <= 2100:
            return str(value)
        return ""

    if isinstance(value, str):
        match = re.search(r"\d{4}", value)
        if not match:
            return ""
        year = int(match.group(0))
        return str(year) if 1900 <= year <= 2100 else ""

    return ""


def _format_release_date(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, bool):
        return ""
    if isinstance(value, int):
        if value > 10000 or (0 <= value < 1900):
            try:
                return datetime.datetime.fromtimestamp(value, tz=timezone.utc).strftime("%Y-%m-%d")
            except (OverflowError, OSError, ValueError):
                return ""
        if 1900 <= value <= 2200:
            return str(value)
        return ""
    if isinstance(value, str):
        try:
            parsed = datetime.datetime.fromisoformat(value)
            return parsed.strftime("%Y-%m-%d")
        except (ValueError, TypeError):
            pass
        match = re.search(r"\d{4}", value)
        if match:
            return match.group(0)
        return ""
    return ""


def details_metadata_from_item(item: dict[str, Any]) -> dict[str, str]:
    source_blocks: dict[str, dict[str, Any]] = {}
    for key in SOURCE_PRIORITY:
        value = item.get(key)
        source_blocks[key] = value if isinstance(value, dict) else {}

    description = ""
    description_source = ""
    genres: list[str] = []
    regions: list[str] = []
    rating = ""
    rating_source = ""
    release_year = ""

    for source_key in SOURCE_PRIORITY:
        source_data = source_blocks[source_key]
        if not description:
            candidate = _source_field_text(source_data, _DESCRIPTION_KEYS)
            if candidate:
                description = candidate
                description_source = source_key

        source_genres = _source_field_list(source_data, _GENRE_KEYS)
        if source_genres and not genres:
            genres = list(source_genres)
        elif source_genres:
            for value in source_genres:
                if value.casefold() not in {entry.casefold() for entry in genres}:
                    genres.append(value)

        source_regions = _source_field_list(source_data, _REGION_KEYS)
        if source_regions and not regions:
            regions = list(source_regions)
        elif source_regions:
            for value in source_regions:
                if value.casefold() not in {entry.casefold() for entry in regions}:
                    regions.append(value)

        if not rating:
            for key in _RATING_KEYS:
                candidate = format_rating_to_five(source_data.get(key))
                if candidate:
                    rating = candidate
                    rating_source = source_key
                    break

    for raw_value in (
        source_blocks["launchbox_metadata"].get("first_release_date"),
        source_blocks["ss_metadata"].get("first_release_date"),
        source_blocks["igdb_metadata"].get("first_release_date"),
        item.get("flashpoint_metadata", {}).get("first_release_date") if isinstance(item.get("flashpoint_metadata"), dict) else None,
    ):
        release_year = _extract_year(raw_value)
        if release_year:
            break

    if not release_year:
        metadatum = item.get("metadatum")
        if isinstance(metadatum, dict):
            release_year = _extract_year(metadatum.get("first_release_date"))

    if not description:
        description = _clean_text(item.get("summary"))
        if description:
            description_source = "summary"

    if not regions:
        regions = _source_field_list(item, _REGION_KEYS)

    if not genres:
        metadatum = item.get("metadatum")
        if isinstance(metadatum, dict):
            genres = _source_field_list(metadatum, _GENRE_KEYS)

    if not rating:
        for key in _RATING_KEYS:
            candidate = format_rating_to_five(item.get(key))
            if candidate:
                rating = candidate
                rating_source = "rom"
                break

    size_bytes = item.get("fs_size_bytes")
    size_text = str(size_bytes) if isinstance(size_bytes, int) and size_bytes > 0 else ""

    _gamelist_meta = item.get("gamelist_metadata")
    _gamelist_meta_block = _gamelist_meta if isinstance(_gamelist_meta, dict) else {}
    fanart_url = source_blocks.get("ss_metadata", {}).get("fanart_url") or _gamelist_meta_block.get("fanart_url") or ""
    if not isinstance(fanart_url, str):
        fanart_url = ""

    companies = ""
    for _src_key in ("launchbox_metadata", "igdb_metadata", "ss_metadata"):
        _companies = source_blocks.get(_src_key, {}).get("companies")
        if isinstance(_companies, list) and _companies:
            companies = ", ".join(str(c) for c in _companies)
            break
    if not companies:
        _meta_companies = item.get("metadatum", {}).get("companies") if isinstance(item.get("metadatum"), dict) else None
        if isinstance(_meta_companies, list) and _meta_companies:
            companies = ", ".join(str(c) for c in _meta_companies if c)
        elif isinstance(_meta_companies, str):
            companies = _meta_companies.strip()

    first_release_date = ""
    for _raw_date in (
        source_blocks["launchbox_metadata"].get("first_release_date"),
        source_blocks["ss_metadata"].get("first_release_date"),
        source_blocks["igdb_metadata"].get("first_release_date"),
        item.get("flashpoint_metadata", {}).get("first_release_date") if isinstance(item.get("flashpoint_metadata"), dict) else None,
    ):
        first_release_date = _format_release_date(_raw_date)
        if first_release_date:
            break
    if not first_release_date:
        metadatum = item.get("metadatum")
        if isinstance(metadatum, dict):
            first_release_date = _format_release_date(metadatum.get("first_release_date"))

    _raw_tags = item.get("tags")
    if isinstance(_raw_tags, list) and _raw_tags:
        _tag_names = []
        for _t in _raw_tags:
            if isinstance(_t, dict):
                _tag_names.append(str(_t.get("name", "") or ""))
            else:
                _tag_names.append(str(_t))
        tags = ", ".join(t for t in _tag_names if t)
    else:
        tags = ""

    return {
        "description": description,
        "description_source": description_source,
        "genres": ", ".join(genres),
        "regions": ", ".join(regions),
        "rating": rating,
        "rating_source": rating_source,
        "release_year": release_year,
        "filesize_bytes": size_text,
        "revision": str(item.get("revision") or ""),
        "languages": ", ".join(item.get("languages", [])) if isinstance(item.get("languages"), list) else "",
        "tags": tags,
        "fanart_url": fanart_url,
        "companies": companies,
        "first_release_date": first_release_date,
    }
