from __future__ import annotations


def game_key(game: dict[str, str]) -> tuple[str, str]:
    return (game.get("title", "").strip().lower(), game.get("platform", "").strip().lower())


def rom_id_key(game: dict[str, str]) -> str:
    rom_id_value = game.get("rom_id", "")
    if not isinstance(rom_id_value, str):
        return ""
    return rom_id_value.strip().casefold()


def games_match_identity(left: dict[str, str], right: dict[str, str]) -> bool:
    left_rom_id = rom_id_key(left)
    right_rom_id = rom_id_key(right)
    if left_rom_id and right_rom_id:
        return left_rom_id == right_rom_id
    return game_key(left) == game_key(right)


def is_game_installed(game: dict[str, str], library_games: list[dict[str, str]]) -> bool:
    return any(games_match_identity(installed, game) for installed in library_games)


def installed_game_record(game: dict[str, str], library_games: list[dict[str, str]]) -> dict[str, str] | None:
    for installed in library_games:
        if games_match_identity(installed, game):
            return installed
    return None
