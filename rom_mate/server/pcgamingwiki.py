from __future__ import annotations

"""PCGamingWiki API client for resolving Windows save file locations."""

import json
import re
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, unquote, urlparse
from urllib.request import Request, urlopen

_PCGW_API_BASE = "https://www.pcgamingwiki.com/w/api.php"

# Map PCGamingWiki path template variables to Windows environment-style paths.
_PCGW_PATH_VARS: dict[str, str | None] = {
    "userprofile\\documents": "%USERPROFILE%\\Documents",
    "userdocuments": "%USERPROFILE%\\Documents",
    "savedgames": "%USERPROFILE%\\Documents",
    "userprofile": "%USERPROFILE%",
    "appdata": "%APPDATA%",
    "localappdata": "%LOCALAPPDATA%",
    "local appdata": "%LOCALAPPDATA%",
    "applocaldata": "%LOCALAPPDATA%",
    "programdata": "%PROGRAMDATA%",
    "allusersappdata": "%PROGRAMDATA%",
    "public\\documents": "%PUBLIC%\\Documents",
    "publicdocuments": "%PUBLIC%\\Documents",
    "public": "%PUBLIC%",
    "windir": "%WINDIR%",
    "syswow64": "%WINDIR%",
    "system": "%WINDIR%",
    "game": "%GAME_DIR%",
    "steam": None,
    "uplay": None,
    "epicgames": None,
    "gog": None,
    "origin": None,
    "battlenet": None,
    "itchapp": None,
    "registry": None,
}

_PATH_VAR_RE = re.compile(r"^\s*\{\{[Pp]\|([^}]+)\}\}")
_TRAILING_WILDCARD_RE = re.compile(r"[\\/][^\\/|\n\r]*\*[^|\n\r]*$")
_SAVE_TEMPLATE_START_RE = re.compile(r"\{\{\s*game\s*data\s*/\s*saves\s*\|", re.IGNORECASE)


class PCGamingWikiError(Exception):
    pass


def _expand_pcgw_path_var(var_name: str) -> str | None:
    return _PCGW_PATH_VARS.get(var_name.strip().lower())


def _expand_pcgw_path(raw_path: str) -> str | None:
    text = str(raw_path or "").strip()
    if not text:
        return None

    match = _PATH_VAR_RE.match(text)
    if not match:
        return None

    expanded_var = _expand_pcgw_path_var(match.group(1))
    if not expanded_var:
        return None

    # Remove template artifacts and trim wildcard suffixes to keep directory paths.
    path = _PATH_VAR_RE.sub(lambda _: expanded_var, text, count=1)
    path = re.sub(r'\{\{[^{}]*\}\}', '', path).strip()
    path = _TRAILING_WILDCARD_RE.sub("", path).rstrip("\\/").strip()
    return path or None


def _extract_template_block(wikitext: str, start_index: int) -> tuple[str, int] | None:
    i = start_index
    depth = 0
    length = len(wikitext)
    while i < length - 1:
        pair = wikitext[i : i + 2]
        if pair == "{{":
            depth += 1
            i += 2
            continue
        if pair == "}}":
            depth -= 1
            i += 2
            if depth == 0:
                return wikitext[start_index:i], i
            continue
        i += 1
    return None


def parse_windows_save_paths(wikitext: str) -> list[str]:
    text = str(wikitext or "")
    found_paths: list[str] = []
    seen: set[str] = set()

    pos = 0
    while True:
        match = _SAVE_TEMPLATE_START_RE.search(text, pos)
        if not match:
            break

        extracted = _extract_template_block(text, match.start())
        if not extracted:
            break

        block, pos = extracted
        # Trim opening '{{' and closing '}}' before split-parsing template arguments.
        parts = [part.strip() for part in _split_template_args(block[2:-2])]
        if len(parts) < 3:
            continue
        if parts[1].strip().lower() != "windows":
            continue

        for arg in parts[2:]:
            expanded = _expand_pcgw_path(arg)
            if expanded and expanded not in seen:
                seen.add(expanded)
                found_paths.append(expanded)

    return found_paths


def _fetch_json(url: str) -> dict:
    payload = _fetch_json_value(url)
    if not isinstance(payload, dict):
        raise PCGamingWikiError("PCGamingWiki response must be a JSON object")
    return payload


def _fetch_json_value(url: str) -> Any:
    try:
        req = Request(url, headers={"User-Agent": "rom-mate/1.0 (pcgamingwiki-client)"})
        with urlopen(req, timeout=10) as response:
            raw = response.read().decode("utf-8")
        payload = json.loads(raw)
    except HTTPError as exc:
        try:
            body = exc.read().decode("utf-8", errors="replace")[:300]
        except Exception:
            body = ""
        detail = body if body else str(exc)
        raise PCGamingWikiError(f"PCGamingWiki HTTP {exc.code}: {detail}") from exc
    except (URLError, json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
        raise PCGamingWikiError(f"PCGamingWiki request failed: {exc}") from exc
    return payload


def _split_template_args(inner_template: str) -> list[str]:
    args: list[str] = []
    current: list[str] = []
    depth = 0
    i = 0
    while i < len(inner_template):
        pair = inner_template[i : i + 2]
        if pair == "{{":
            depth += 1
            current.append(pair)
            i += 2
            continue
        if pair == "}}" and depth > 0:
            depth -= 1
            current.append(pair)
            i += 2
            continue

        char = inner_template[i]
        if char == "|" and depth == 0:
            args.append("".join(current))
            current = []
            i += 1
            continue

        current.append(char)
        i += 1

    args.append("".join(current))
    return args


def _build_page_id_url(title: str) -> str:
    encoded_title = quote(title, safe="")
    return f"{_PCGW_API_BASE}?action=query&titles={encoded_title}&prop=info&format=json"


def _extract_page_id_from_query(payload: dict) -> int | None:
    query_section = payload.get("query")
    if not isinstance(query_section, dict):
        return None
    pages = query_section.get("pages")
    if not isinstance(pages, dict) or not pages:
        return None

    for page_id_raw, page_data in pages.items():
        if page_id_raw == "-1" or not isinstance(page_data, dict) or "missing" in page_data:
            continue
        try:
            return int(page_id_raw)
        except (TypeError, ValueError):
            continue
    return None


def _extract_title_from_url(url: str) -> str | None:
    parsed = urlparse(url)
    path = parsed.path or ""
    if "/wiki/" in path:
        title = path.split("/wiki/", 1)[1]
        title = unquote(title).replace("_", " ").strip()
        return title or None
    return None


def fetch_page_id_by_title(title: str) -> int | None:
    query_title = str(title or "").strip()
    if not query_title:
        return None

    payload = _fetch_json(_build_page_id_url(query_title))
    page_id = _extract_page_id_from_query(payload)
    if page_id is not None:
        return page_id

    encoded_title = quote(query_title, safe="")
    opensearch_url = (
        f"{_PCGW_API_BASE}?action=opensearch&search={encoded_title}"
        "&namespace=0&limit=3&format=json"
    )
    search_payload = _fetch_json_value(opensearch_url)

    search_urls: list[str] = []
    if isinstance(search_payload, list) and len(search_payload) > 3 and isinstance(search_payload[3], list):
        search_urls = [str(u) for u in search_payload[3] if isinstance(u, str)]

    if not search_urls:
        return None

    resolved_title = _extract_title_from_url(search_urls[0])
    if not resolved_title:
        return None

    fallback_payload = _fetch_json(_build_page_id_url(resolved_title))
    return _extract_page_id_from_query(fallback_payload)


def fetch_page_wikitext(page_id: int) -> str:
    url = f"{_PCGW_API_BASE}?action=parse&pageid={int(page_id)}&prop=wikitext&format=json"
    payload = _fetch_json(url)

    try:
        wikitext = payload["parse"]["wikitext"]["*"]
    except (KeyError, TypeError) as exc:
        raise PCGamingWikiError("PCGamingWiki parse response missing wikitext") from exc

    if not isinstance(wikitext, str):
        raise PCGamingWikiError("PCGamingWiki wikitext payload must be a string")
    return wikitext


def fetch_windows_save_paths(title: str) -> list[str]:
    page_id = fetch_page_id_by_title(title)
    if page_id is None:
        return []
    wikitext = fetch_page_wikitext(page_id)
    return parse_windows_save_paths(wikitext)
