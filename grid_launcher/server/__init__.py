from .catalog import connected_username, fetch_platform_rom_items, games_from_rom_items, server_platform_ids
from .connection import ConnectionFailure, classify_connection_failure
from .details_cache import (
    cache_rom_id_for_details_game,
    clear_cached_rom_id_for_details_game,
    details_rom_id_cache,
    details_rom_id_cache_key,
    fetch_server_rom_payload,
    resolve_rom_id_for_game,
    resolved_rom_file_name_for_game,
    rom_file_name_from_payload,
)
from .orchestrator import fetch_connection_payloads
from .state import account_status_text, credentials_present, server_base_url
from .status import apply_server_status
from .view import (
    clear_server_connection_data,
    clear_server_search,
    on_server_platform_selected,
    on_server_search_changed,
    populate_server_platforms,
    render_server_games,
)

__all__ = [
    "ConnectionFailure",
    "account_status_text",
    "apply_server_status",
    "cache_rom_id_for_details_game",
    "classify_connection_failure",
    "clear_cached_rom_id_for_details_game",
    "clear_server_connection_data",
    "clear_server_search",
    "connected_username",
    "credentials_present",
    "details_rom_id_cache",
    "details_rom_id_cache_key",
    "fetch_connection_payloads",
    "fetch_platform_rom_items",
    "fetch_server_rom_payload",
    "games_from_rom_items",
    "on_server_platform_selected",
    "on_server_search_changed",
    "populate_server_platforms",
    "render_server_games",
    "resolve_rom_id_for_game",
    "resolved_rom_file_name_for_game",
    "rom_file_name_from_payload",
    "server_base_url",
    "server_platform_ids",
]
