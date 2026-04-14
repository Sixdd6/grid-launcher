from __future__ import annotations

import re
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

    return {
        "description": description,
        "description_source": description_source,
        "genres": ", ".join(genres),
        "regions": ", ".join(regions),
        "rating": rating,
        "rating_source": rating_source,
        "filesize_bytes": size_text,
    }
