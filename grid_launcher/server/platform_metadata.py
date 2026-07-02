"""Static platform metadata lookup keyed by RomM platform slug."""

PLATFORM_METADATA: dict[str, dict] = {
    "2600": {
        "manufacturer": "Atari",
        "release_year": "1977",
        "player_count": "1–2 players",
    },
    "jaguar": {
        "manufacturer": "Atari",
        "release_year": "1993",
        "player_count": "1–2 players",
    },
    "lynx": {
        "manufacturer": "Atari",
        "release_year": "1989",
        "player_count": "1 player",
    },
    "nes": {
        "manufacturer": "Nintendo",
        "release_year": "1983",
        "player_count": "1–2 players",
    },
    "snes": {
        "manufacturer": "Nintendo",
        "release_year": "1990",
        "player_count": "1–2 players",
    },
    "n64": {
        "manufacturer": "Nintendo",
        "release_year": "1996",
        "player_count": "1–4 players",
    },
    "gc": {
        "manufacturer": "Nintendo",
        "release_year": "2001",
        "player_count": "1–4 players",
    },
    "wii": {
        "manufacturer": "Nintendo",
        "release_year": "2006",
        "player_count": "1–4 players",
    },
    "wiiu": {
        "manufacturer": "Nintendo",
        "release_year": "2012",
        "player_count": "1–5 players",
    },
    "switch": {
        "manufacturer": "Nintendo",
        "release_year": "2017",
        "player_count": "1–4 players",
    },
    "gb": {
        "manufacturer": "Nintendo",
        "release_year": "1989",
        "player_count": "1–2 players",
    },
    "gbc": {
        "manufacturer": "Nintendo",
        "release_year": "1998",
        "player_count": "1–2 players",
    },
    "gba": {
        "manufacturer": "Nintendo",
        "release_year": "2001",
        "player_count": "1–2 players",
    },
    "nds": {
        "manufacturer": "Nintendo",
        "release_year": "2004",
        "player_count": "1–8 players",
    },
    "3ds": {
        "manufacturer": "Nintendo",
        "release_year": "2011",
        "player_count": "1–8 players",
    },
    "virtualboy": {
        "manufacturer": "Nintendo",
        "release_year": "1995",
        "player_count": "1 player",
    },
    "genesis-slash-megadrive": {
        "manufacturer": "Sega",
        "release_year": "1988",
        "player_count": "1–2 players",
    },
    "sms": {
        "manufacturer": "Sega",
        "release_year": "1985",
        "player_count": "1–2 players",
    },
    "gg": {
        "manufacturer": "Sega",
        "release_year": "1990",
        "player_count": "1 player",
    },
    "saturn": {
        "manufacturer": "Sega",
        "release_year": "1994",
        "player_count": "1–2 players",
    },
    "dreamcast": {
        "manufacturer": "Sega",
        "release_year": "1998",
        "player_count": "1–4 players",
    },
    "segacd": {
        "manufacturer": "Sega",
        "release_year": "1991",
        "player_count": "1–2 players",
    },
    "32x": {
        "manufacturer": "Sega",
        "release_year": "1994",
        "player_count": "1–2 players",
    },
    "ps": {
        "manufacturer": "Sony",
        "release_year": "1994",
        "player_count": "1–2 players",
    },
    "ps2": {
        "manufacturer": "Sony",
        "release_year": "2000",
        "player_count": "1–2 players",
    },
    "ps3": {
        "manufacturer": "Sony",
        "release_year": "2006",
        "player_count": "1–7 players",
    },
    "ps4--1": {
        "manufacturer": "Sony",
        "release_year": "2013",
        "player_count": "1–4 players",
    },
    "ps5": {
        "manufacturer": "Sony",
        "release_year": "2020",
        "player_count": "1–4 players",
    },
    "psp": {
        "manufacturer": "Sony",
        "release_year": "2004",
        "player_count": "1–4 players",
    },
    "psvita": {
        "manufacturer": "Sony",
        "release_year": "2011",
        "player_count": "1–4 players",
    },
    "xbox": {
        "manufacturer": "Microsoft",
        "release_year": "2001",
        "player_count": "1–4 players",
    },
    "xbox360": {
        "manufacturer": "Microsoft",
        "release_year": "2005",
        "player_count": "1–4 players",
    },
    "xboxone": {
        "manufacturer": "Microsoft",
        "release_year": "2013",
        "player_count": "1–4 players",
    },
    "turbografx-16--1": {
        "manufacturer": "NEC",
        "release_year": "1987",
        "player_count": "1–5 players",
    },
    "turbografx-16-cd": {
        "manufacturer": "NEC",
        "release_year": "1988",
        "player_count": "1–5 players",
    },
    "neogeo": {
        "manufacturer": "SNK",
        "release_year": "1990",
        "player_count": "1–2 players",
    },
    "ngp": {
        "manufacturer": "SNK",
        "release_year": "1998",
        "player_count": "1–2 players",
    },
    "ngpc": {
        "manufacturer": "SNK",
        "release_year": "1999",
        "player_count": "1–2 players",
    },
    "arcade": {
        "manufacturer": "Various",
        "release_year": "1970",
        "player_count": "1–4 players",
    },
    "msx": {
        "manufacturer": "Various",
        "release_year": "1983",
        "player_count": "1–2 players",
    },
    "colecovision": {
        "manufacturer": "Coleco",
        "release_year": "1982",
        "player_count": "1–2 players",
    },
    "3do": {
        "manufacturer": "3DO Company",
        "release_year": "1993",
        "player_count": "1–2 players",
    },
    "dos": {
        "manufacturer": "Various",
        "release_year": "1981",
        "player_count": "1 player",
    },
    "win": {
        "manufacturer": "Microsoft",
        "release_year": "1985",
        "player_count": "1 player",
    },
}

PLATFORM_LOGO_FILES: dict[str, str] = {
    "2600": "Atari - 2600.png",
    "lynx": "Atari - Lynx.png",
    "jaguar": "Atari - Jaguar.png",
    "nes": "Nintendo - Nintendo Entertainment System.png",
    "snes": "Nintendo - Super Nintendo Entertainment System.png",
    "n64": "Nintendo - Nintendo 64.png",
    "gc": "Nintendo - GameCube.png",
    "ngc": "Nintendo - GameCube.png",
    "gamecube": "Nintendo - GameCube.png",
    "wii": "Nintendo - Wii.png",
    "wiiu": "Nintendo - Wii U.png",
    "switch": "Nintendo - Switch.png",
    "gb": "Nintendo - Game Boy.png",
    "gbc": "Nintendo - Game Boy Color.png",
    "gba": "Nintendo - Game Boy Advance.png",
    "nds": "Nintendo - Nintendo DS.png",
    "3ds": "Nintendo - Nintendo 3DS.png",
    "virtualboy": "Nintendo - Virtual Boy.png",
    "genesis": "Sega - Mega Drive - Genesis.png",
    "genesis-slash-megadrive": "Sega - Mega Drive - Genesis.png",
    "sms": "Sega - Master System - Mark III.png",
    "gg": "Sega - Game Gear.png",
    "saturn": "Sega - Saturn.png",
    "dreamcast": "Sega - Dreamcast.png",
    "segacd": "Sega - Mega-CD - Sega CD.png",
    "32x": "Sega - 32X.png",
    "sega32": "Sega - 32X.png",
    "ps": "Sony - PlayStation.png",
    "psx": "Sony - PlayStation.png",
    "ps2": "Sony - PlayStation 2.png",
    "ps3": "Sony - PlayStation 3.png",
    "ps4--1": "Sony - PlayStation 4.png",
    "psp": "Sony - PlayStation Portable.png",
    "psvita": "Sony - PlayStation Vita.png",
    "xbox": "Microsoft - Xbox.png",
    "xbox360": "Microsoft - Xbox 360.png",
    "xboxone": "Microsoft - Xbox One.png",
    "neogeo": "SNK - Neo Geo.png",
    "ngp": "SNK - Neo Geo Pocket.png",
    "ngpc": "SNK - Neo Geo Pocket Color.png",
    "turbografx-16--1": "NEC - PC Engine - TurboGrafx 16.png",
    "tg16": "NEC - PC Engine - TurboGrafx 16.png",
    "pcengine": "NEC - PC Engine - TurboGrafx 16.png",
    "turbografx-16-cd": "NEC - PC Engine CD - TurboGrafx-CD.png",
    "turbografx-cd": "NEC - PC Engine CD - TurboGrafx-CD.png",
    "pcenginecd": "NEC - PC Engine CD - TurboGrafx-CD.png",
    "colecovision": "Coleco - ColecoVision.png",
    "3do": "The 3DO Company - 3DO.png",
    "msx": "Microsoft - MSX.png",
    "dos": "DOS.png",
    "win": "windows7.png",  # live server slug for Windows
    "windows": "windows7.png",  # common alias
    "win3x": "windows9x.png",  # Windows 3.x - closest match
    "windows9x": "windows9x.png",  # anticipated slug
    "arcade": "FBNeo - Arcade Games.png",
    "pico": "Pico8.png",
    "segapico": "Sega - PICO.png",
    "atari2600": "Atari - 2600.png",
    "atari5200": "Atari - 5200.png",
    "atari7800": "Atari - 7800.png",
    "atari-jaguar-cd": "Atari - Jaguar CD.png",
    "atari-st": "Atari - ST.png",
    "atari-xegs": "Atari - XEGS.png",
    "casio-loopy": "Casio - Loopy.png",
    "casio-pv-1000": "Casio - PV-1000.png",
    "colecoadam": "Coleco - ColecoVision ADAM.png",
    "c64": "Commodore - 64.png",
    "amiga": "Commodore - Amiga.png",
    "c-plus-4": "Commodore - Plus-4.png",
    "vic-20": "Commodore - VIC-20.png",
    "dc": "Sega - Dreamcast.png",
    "gamegear": "Sega - Game Gear.png",
    "sg1000": "Sega - SG-1000.png",
    "sega-pico": "Sega - PICO.png",
    "vectrex": "GCE - Vectrex.png",
    "odyssey-2": "Magnavox - Odyssey2.png",
    "intellivision": "Mattel - Intellivision.png",
    "msx2": "Microsoft - MSX2.png",
    "supergrafx": "NEC - PC Engine SuperGrafx.png",
    "pc-9800-series": "NEC - PC-98.png",
    "pc-fx": "NEC - PC-FX.png",
    "neo-geo-cd": "SNK - Neo Geo CD.png",
    "neogeoaes": "SNK - Neo Geo.png",
    "neogeomvs": "SNK - Neo Geo.png",
    "neo-geo-pocket": "SNK - Neo Geo Pocket.png",
    "neo-geo-pocket-color": "SNK - Neo Geo Pocket Color.png",
    "fds": "Nintendo - Family Computer Disk System.png",
    "famicom": "Nintendo - Nintendo Entertainment System.png",
    "64dd": "Nintendo - Nintendo 64DD.png",
    "nintendo-dsi": "Nintendo - Nintendo DSi.png",
    "new-nintendo-3ds": "Nintendo - New Nintendo 3DS.png",
    "pokemon-mini": "Nintendo - Pokemon Mini.png",
    "satellaview": "Nintendo - Satellaview.png",
    "sufami-turbo": "Nintendo - Sufami Turbo.png",
    "videopac-g7400": "Philips - Videopac+.png",
    "rca-studio-ii": "RCA - Studio II.png",
    "scummvm": "ScummVM.png",
    "sharp-x68000": "Sharp - X68000.png",
    "zx81": "Sinclair - ZX 81.png",
    "zxs": "Sinclair - ZX Spectrum.png",
    "spectravideo": "Spectravideo - SVI-318 - SVI-328.png",
    "uzebox": "Uzebox.png",
    "arcadia-2001": "Emerson - Arcadia 2001.png",
    "adventure-vision": "Entex - Adventure Vision.png",
    "epoch-super-cassette-vision": "Epoch - Super Cassette Vision.png",
    "fairchild-channel-f": "Fairchild - Channel F.png",
    "super-acan": "Funtech - Super Acan.png",
    "gp32": "GamePark - GP32.png",
    "hartung": "Hartung - Game Master.png",
    "leapster": "LeapFrog - Leapster Learning Game System.png",
    "game-dot-com": "Tiger - Game.com.png",
    "supervision": "Watara - Supervision.png",
    "creativision": "VTech - CreatiVision.png",
    "vsmile": "VTech - V.Smile.png",
    "pocket-challenge-v2": "Benesse - Pocket Challenge V2.png",
}


def _build_name_logo_index() -> dict[str, str]:
    """Build a normalized-name -> filename lookup from PLATFORM_LOGO_FILES values.

    For each filename like "Sony - PlayStation 2.png", extracts the part after
    the last " - " separator ("PlayStation 2"), normalises it to lowercase with
    spaces collapsed, and maps it to the filename. Also adds the full stem
    (e.g. "sony - playstation 2") as a key so whole-name matches work too.
    """
    index: dict[str, str] = {}
    for filename in PLATFORM_LOGO_FILES.values():
        stem = filename.removesuffix(".png")
        # full stem key (e.g. "sony - playstation 2")
        index[stem.lower()] = filename
        # short name after last " - " (e.g. "playstation 2")
        if " - " in stem:
            short = stem.rsplit(" - ", 1)[1]
            index[short.lower()] = filename
    return index


_NAME_LOGO_INDEX: dict[str, str] = _build_name_logo_index()


def logo_file_for_platform(slug: str, display_name: str) -> str:
    """Return the retroarch-asset filename for a platform, or '' if none found.

    Tries display-name lookup first (more specific), then slug fallback.
    """
    # 1. name-based lookup first (handles custom_name variants and shared slugs)
    normalized = display_name.strip().lower()
    result = _NAME_LOGO_INDEX.get(normalized, "")
    if not result:
        # also try space-stripped version (e.g. "windows 9x" -> "windows9x")
        result = _NAME_LOGO_INDEX.get(normalized.replace(" ", ""), "")
    if result:
        return result
    # 2. exact slug match fallback
    return PLATFORM_LOGO_FILES.get(slug, "")
