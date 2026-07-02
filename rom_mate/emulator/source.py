from __future__ import annotations

import fnmatch
from typing import Any


class EmulatorSourceResolutionError(ValueError):
    """Raised when source metadata cannot be resolved into a release asset."""


def resolve_emulator_source_release_asset(
    source_metadata: dict[str, Any],
    release_metadata: dict[str, Any] | list[dict[str, Any]],
) -> dict[str, Any]:
    source = normalize_emulator_source_metadata(source_metadata)
    provider = source["provider"]
    if provider == "direct":
        download_url = str(source.get("download_url", "")).strip()
        asset_name = str(source.get("asset_name", "")).strip()
        if not download_url:
            raise EmulatorSourceResolutionError(
                "Direct source metadata is missing 'download_url'."
            )
        if not asset_name:
            asset_name = download_url.rsplit("/", 1)[-1].strip()
        release_tag = str(source.get("release_tag", "")).strip() or "latest"
        return {
            "provider": provider,
            "owner": source["owner"],
            "repo": source["repo"],
            "release_tag": release_tag,
            "release_name": release_tag,
            "asset_name": asset_name,
            "download_url": download_url,
            "size": 0,
            "content_type": "",
        }
    if provider not in ("github", "gitea"):
        raise EmulatorSourceResolutionError(
            f"Unsupported source provider '{provider}'. Supported providers: github, gitea, direct."
        )

    release = _select_github_release(source, release_metadata)
    asset = _select_github_asset(source, release)

    return {
        "provider": provider,
        "owner": source["owner"],
        "repo": source["repo"],
        "release_tag": str(release.get("tag_name", "")).strip(),
        "release_name": str(release.get("name", "")).strip(),
        "asset_name": str(asset.get("name", "")).strip(),
        "download_url": str(asset.get("browser_download_url", "")).strip(),
        "size": int(asset.get("size", 0) or 0),
        "content_type": str(asset.get("content_type", "")).strip(),
    }


def normalize_emulator_source_metadata(source_metadata: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(source_metadata, dict):
        raise EmulatorSourceResolutionError("Source metadata must be a dictionary.")

    provider_value = source_metadata.get("provider", source_metadata.get("type", "github"))
    provider = str(provider_value).strip().casefold() if isinstance(provider_value, str) else ""
    provider_aliases = {
        "github": "github",
        "github-release": "github",
        "github_release": "github",
        "githubrelease": "github",
        "gitea": "gitea",
        "gitea-release": "gitea",
        "gitea_release": "gitea",
        "direct": "direct",
        "direct-download": "direct",
        "direct_download": "direct",
        "download": "direct",
        "url": "direct",
    }
    normalized_provider = provider_aliases.get(provider, provider)
    if not normalized_provider:
        raise EmulatorSourceResolutionError("Source metadata is missing provider.")

    owner = _normalized_required_string(source_metadata, "owner")
    repo = _normalized_required_string(source_metadata, "repo", fallback_key="repository")

    include_patterns = _normalized_patterns(
        source_metadata.get("asset_patterns", source_metadata.get("asset_globs", ["*"])),
        default=["*"],
    )
    exclude_patterns = _normalized_patterns(
        source_metadata.get("asset_exclude_patterns", source_metadata.get("exclude_asset_patterns", [])),
        default=[],
    )
    preferred_patterns = _normalized_patterns(
        source_metadata.get("asset_preferred_patterns", source_metadata.get("preferred_asset_patterns", [])),
        default=[],
    )

    release_tag = ""
    for key in ("tag", "release_tag", "version"):
        value = source_metadata.get(key, "")
        if isinstance(value, str) and value.strip():
            release_tag = value.strip()
            break

    allow_prerelease = bool(source_metadata.get("allow_prerelease", False))
    normalized = {
        "provider": normalized_provider,
        "owner": owner,
        "repo": repo,
        "release_tag": release_tag,
        "allow_prerelease": allow_prerelease,
        "asset_patterns": include_patterns,
        "asset_exclude_patterns": exclude_patterns,
        "asset_preferred_patterns": preferred_patterns,
    }

    if normalized_provider == "gitea":
        base_url = _normalized_required_string(source_metadata, "base_url")
        normalized["base_url"] = base_url.rstrip("/")

    if normalized_provider == "direct":
        download_url = _normalized_optional_string(
            source_metadata,
            "download_url",
            fallback_keys=("url", "browser_download_url"),
        )
        page_url = _normalized_optional_string(
            source_metadata,
            "page_url",
            fallback_keys=("index_url", "listing_url"),
        )
        download_url_regex = _normalized_optional_string(
            source_metadata,
            "download_url_regex",
            fallback_keys=("url_regex", "asset_url_regex"),
        )
        asset_name = _normalized_optional_string(source_metadata, "asset_name")
        if not download_url and not page_url:
            raise EmulatorSourceResolutionError(
                "Direct source metadata must include either 'download_url' or 'page_url'."
            )
        normalized.update(
            {
                "download_url": download_url,
                "page_url": page_url,
                "download_url_regex": download_url_regex,
                "asset_name": asset_name,
            }
        )
        supplemental_value = source_metadata.get("supplemental_downloads", [])
        if isinstance(supplemental_value, list):
            normalized["supplemental_downloads"] = [
                dict(item)
                for item in supplemental_value
                if isinstance(item, dict)
            ]

        platform_overrides = source_metadata.get("platform_overrides")
        if isinstance(platform_overrides, dict) and platform_overrides:
            normalized["platform_overrides"] = platform_overrides

    return normalized


def _normalized_optional_string(
    metadata: dict[str, Any],
    key: str,
    *,
    fallback_keys: tuple[str, ...] = (),
) -> str:
    value = metadata.get(key, "")
    if isinstance(value, str) and value.strip():
        return value.strip()

    for fallback_key in fallback_keys:
        fallback_value = metadata.get(fallback_key, "")
        if isinstance(fallback_value, str) and fallback_value.strip():
            return fallback_value.strip()

    return ""


def _normalized_required_string(
    metadata: dict[str, Any],
    key: str,
    *,
    fallback_key: str | None = None,
) -> str:
    value = metadata.get(key, "")
    if (not isinstance(value, str) or not value.strip()) and fallback_key:
        value = metadata.get(fallback_key, "")

    normalized = value.strip() if isinstance(value, str) else ""
    if not normalized:
        fallback_note = f" (or '{fallback_key}')" if fallback_key else ""
        raise EmulatorSourceResolutionError(
            f"Source metadata is missing required field '{key}'{fallback_note}."
        )
    return normalized


def _normalized_patterns(value: Any, *, default: list[str]) -> list[str]:
    if value is None:
        return list(default)

    if isinstance(value, str):
        normalized = value.strip()
        return [normalized] if normalized else list(default)

    if not isinstance(value, (list, tuple, set)):
        return list(default)

    patterns = [item.strip() for item in value if isinstance(item, str) and item.strip()]
    return patterns if patterns else list(default)


def _extract_releases(release_metadata: dict[str, Any] | list[dict[str, Any]]) -> list[dict[str, Any]]:
    if isinstance(release_metadata, list):
        return [release for release in release_metadata if isinstance(release, dict)]

    if isinstance(release_metadata, dict):
        if isinstance(release_metadata.get("assets"), list):
            return [release_metadata]
        if "releases" not in release_metadata:
            raise EmulatorSourceResolutionError(
                "GitHub release metadata must be a release object, a list of release objects, or a dictionary with 'releases'."
            )
        releases_value = release_metadata.get("releases", [])
        if isinstance(releases_value, list):
            return [release for release in releases_value if isinstance(release, dict)]

    raise EmulatorSourceResolutionError(
        "GitHub release metadata must be a release object, a list of release objects, or a dictionary with 'releases'."
    )


def _select_github_release(
    source: dict[str, Any],
    release_metadata: dict[str, Any] | list[dict[str, Any]],
) -> dict[str, Any]:
    releases = _extract_releases(release_metadata)
    if not releases:
        raise EmulatorSourceResolutionError(
            f"No GitHub releases were provided for '{source['owner']}/{source['repo']}'."
        )

    release_tag = source.get("release_tag", "")
    if isinstance(release_tag, str) and release_tag.strip().casefold() == "latest":
        release_tag = ""
    allow_prerelease = bool(source.get("allow_prerelease", False))

    selected: dict[str, Any] | None = None
    for release in releases:
        if bool(release.get("draft", False)):
            continue
        if bool(release.get("prerelease", False)) and not allow_prerelease:
            continue

        candidate_tag = str(release.get("tag_name", "")).strip()
        if release_tag and candidate_tag.casefold() != release_tag.casefold():
            continue

        selected = release
        break

    if selected is not None:
        return selected

    visible_tags = [
        str(release.get("tag_name", "")).strip()
        for release in releases
        if isinstance(release, dict)
    ]
    filtered_tags = [tag for tag in visible_tags if tag]

    if release_tag:
        raise EmulatorSourceResolutionError(
            "No matching GitHub release was found "
            f"for tag '{release_tag}' in '{source['owner']}/{source['repo']}'. "
            f"Available tags: {', '.join(filtered_tags) if filtered_tags else 'none'}."
        )

    raise EmulatorSourceResolutionError(
        f"No usable GitHub release was found for '{source['owner']}/{source['repo']}'. "
        "All releases were drafts or prereleases."
    )


def _asset_pattern_index(asset_name: str, patterns: list[str]) -> int | None:
    normalized_name = asset_name.casefold()
    for index, pattern in enumerate(patterns):
        if fnmatch.fnmatchcase(normalized_name, pattern.casefold()):
            return index
    return None


def _select_github_asset(source: dict[str, Any], release: dict[str, Any]) -> dict[str, Any]:
    assets_value = release.get("assets", [])
    assets = assets_value if isinstance(assets_value, list) else []

    if not assets:
        raise EmulatorSourceResolutionError(
            "Selected GitHub release has no assets. "
            f"release_tag='{str(release.get('tag_name', '')).strip()}'"
        )

    include_patterns = source.get("asset_patterns", ["*"])
    exclude_patterns = source.get("asset_exclude_patterns", [])
    preferred_patterns = source.get("asset_preferred_patterns", [])

    candidates: list[tuple[tuple[int, int, int, str], dict[str, Any]]] = []
    available_names: list[str] = []

    for asset in assets:
        if not isinstance(asset, dict):
            continue
        name = str(asset.get("name", "")).strip()
        if name:
            available_names.append(name)

        url = str(asset.get("browser_download_url", "")).strip()
        if not name or not url:
            continue

        include_index = _asset_pattern_index(name, include_patterns)
        if include_index is None:
            continue

        if _asset_pattern_index(name, exclude_patterns) is not None:
            continue

        preferred_index = _asset_pattern_index(name, preferred_patterns)
        if preferred_index is None:
            preferred_index = len(preferred_patterns)

        state = str(asset.get("state", "uploaded")).strip().casefold()
        state_penalty = 0 if state in {"", "uploaded"} else 1

        candidates.append(
            (
                (
                    include_index,
                    preferred_index,
                    state_penalty,
                    name.casefold(),
                ),
                asset,
            )
        )

    if not candidates:
        raise EmulatorSourceResolutionError(
            "No release asset matched configured patterns. "
            f"include={include_patterns}, exclude={exclude_patterns}, "
            f"available_assets={available_names}"
        )

    candidates.sort(key=lambda item: item[0])
    return candidates[0][1]
