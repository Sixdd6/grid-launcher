from __future__ import annotations

import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Callable

from rom_mate.core.path import xdg_config_home


_DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE = """\
<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
    <type>Wii U Pro Controller</type>
    <controller>
        <api>XInput</api>
        <uuid>0</uuid>
        <display_name>Controller 1</display_name>
        <rumble>1</rumble>
        <axis>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </axis>
        <rotation>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </rotation>
        <trigger>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </trigger>
        <mappings>
            <entry><mapping>1</mapping><button>13</button></entry>
            <entry><mapping>2</mapping><button>12</button></entry>
            <entry><mapping>3</mapping><button>15</button></entry>
            <entry><mapping>4</mapping><button>14</button></entry>
            <entry><mapping>5</mapping><button>8</button></entry>
            <entry><mapping>6</mapping><button>9</button></entry>
            <entry><mapping>7</mapping><button>42</button></entry>
            <entry><mapping>8</mapping><button>43</button></entry>
            <entry><mapping>9</mapping><button>4</button></entry>
            <entry><mapping>10</mapping><button>5</button></entry>
            <entry><mapping>12</mapping><button>0</button></entry>
            <entry><mapping>13</mapping><button>1</button></entry>
            <entry><mapping>14</mapping><button>2</button></entry>
            <entry><mapping>15</mapping><button>3</button></entry>
            <entry><mapping>16</mapping><button>6</button></entry>
            <entry><mapping>17</mapping><button>7</button></entry>
            <entry><mapping>18</mapping><button>39</button></entry>
            <entry><mapping>19</mapping><button>45</button></entry>
            <entry><mapping>20</mapping><button>44</button></entry>
            <entry><mapping>21</mapping><button>38</button></entry>
            <entry><mapping>22</mapping><button>41</button></entry>
            <entry><mapping>23</mapping><button>47</button></entry>
            <entry><mapping>24</mapping><button>46</button></entry>
            <entry><mapping>25</mapping><button>40</button></entry>
        </mappings>
    </controller>
</emulated_controller>
"""

_DEFAULT_CEMU_SDL_CONTROLLER_PROFILE = """\
<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
    <type>Wii U GamePad</type>
    <controller>
        <api>SDLController</api>
        <uuid>0_030000005e0400008e02000014010000</uuid>
        <mappings>
            <entry><mapping>1</mapping><button>1</button></entry>
            <entry><mapping>2</mapping><button>0</button></entry>
            <entry><mapping>3</mapping><button>3</button></entry>
            <entry><mapping>4</mapping><button>2</button></entry>
            <entry><mapping>5</mapping><button>9</button></entry>
            <entry><mapping>6</mapping><button>10</button></entry>
            <entry><mapping>7</mapping><button>42</button></entry>
            <entry><mapping>8</mapping><button>43</button></entry>
            <entry><mapping>9</mapping><button>6</button></entry>
            <entry><mapping>10</mapping><button>4</button></entry>
            <entry><mapping>11</mapping><button>11</button></entry>
            <entry><mapping>12</mapping><button>12</button></entry>
            <entry><mapping>13</mapping><button>13</button></entry>
            <entry><mapping>14</mapping><button>14</button></entry>
            <entry><mapping>15</mapping><button>7</button></entry>
            <entry><mapping>16</mapping><button>8</button></entry>
            <entry><mapping>17</mapping><button>45</button></entry>
            <entry><mapping>18</mapping><button>39</button></entry>
            <entry><mapping>19</mapping><button>44</button></entry>
            <entry><mapping>20</mapping><button>38</button></entry>
            <entry><mapping>21</mapping><button>47</button></entry>
            <entry><mapping>22</mapping><button>41</button></entry>
            <entry><mapping>23</mapping><button>46</button></entry>
            <entry><mapping>24</mapping><button>40</button></entry>
        </mappings>
    </controller>
</emulated_controller>"""

_DEFAULT_CEMU_SETTINGS_XML = """\
<?xml version="1.0" encoding="UTF-8"?>
<content>
    <logflag>0</logflag>
    <advanced_ppc_logging>false</advanced_ppc_logging>
    <mlc_path></mlc_path>
    <permanent_storage>true</permanent_storage>
    <language>0</language>
    <use_discord_presence>false</use_discord_presence>
    <fullscreen_menubar>false</fullscreen_menubar>
    <feral_gamemode>false</feral_gamemode>
    <check_update>false</check_update>
    <receive_untested_updates>false</receive_untested_updates>
    <save_screenshot>true</save_screenshot>
    <vk_warning>false</vk_warning>
    <gp_download>true</gp_download>
    <macos_disclaimer>false</macos_disclaimer>
    <fullscreen>false</fullscreen>
    <proxy_server></proxy_server>
    <disable_screensaver>true</disable_screensaver>
    <play_boot_sound>false</play_boot_sound>
    <console_language>1</console_language>
    <window_position>
        <x>-1</x>
        <y>-1</y>
    </window_position>
    <window_size>
        <x>-1</x>
        <y>-1</y>
    </window_size>
    <window_maximized>true</window_maximized>
    <open_pad>false</open_pad>
    <pad_position>
        <x>-1</x>
        <y>-1</y>
    </pad_position>
    <pad_size>
        <x>-1</x>
        <y>-1</y>
    </pad_size>
    <pad_maximized>false</pad_maximized>
    <show_icon_column>true</show_icon_column>
    <GameList>
        <style>0</style>
        <order></order>
        <name_width>500</name_width>
        <version_width>500</version_width>
        <dlc_width>500</dlc_width>
        <game_time_width>500</game_time_width>
        <game_started_width>500</game_started_width>
        <region_width>500</region_width>
        <title_id>500</title_id>
    </GameList>
    <RecentLaunchFiles/>
    <RecentNFCFiles/>
    <GamePaths/>
    <GameCache/>
    <GraphicPack/>
    <Graphic>
        <api>0</api>
        <device>00000000000000000000000000000000</device>
        <VSync>0</VSync>
        <GX2DrawdoneSync>true</GX2DrawdoneSync>
        <UpscaleFilter>2</UpscaleFilter>
        <DownscaleFilter>0</DownscaleFilter>
        <FullscreenScaling>0</FullscreenScaling>
        <AsyncCompile>true</AsyncCompile>
        <vkAccurateBarriers>true</vkAccurateBarriers>
        <Overlay>
            <Position>0</Position>
            <TextColor>4294967295</TextColor>
            <TextScale>100</TextScale>
            <FPS>true</FPS>
            <DrawCalls>false</DrawCalls>
            <CPUUsage>false</CPUUsage>
            <CPUPerCoreUsage>false</CPUPerCoreUsage>
            <RAMUsage>false</RAMUsage>
            <VRAMUsage>false</VRAMUsage>
            <Debug>false</Debug>
        </Overlay>
        <Notification>
            <Position>1</Position>
            <TextColor>4294967295</TextColor>
            <TextScale>100</TextScale>
            <ControllerProfiles>true</ControllerProfiles>
            <ControllerBattery>true</ControllerBattery>
            <ShaderCompiling>true</ShaderCompiling>
            <FriendService>true</FriendService>
        </Notification>
    </Graphic>
    <Audio>
        <api>3</api>
        <delay>2</delay>
        <TVChannels>1</TVChannels>
        <PadChannels>1</PadChannels>
        <InputChannels>0</InputChannels>
        <TVVolume>30</TVVolume>
        <PadVolume>0</PadVolume>
        <InputVolume>20</InputVolume>
        <TVDevice>default</TVDevice>
        <PadDevice></PadDevice>
        <InputDevice></InputDevice>
    </Audio>
    <Account>
        <PersistentId>2147483649</PersistentId>
        <OnlineEnabled>false</OnlineEnabled>
        <ActiveService>0</ActiveService>
    </Account>
    <AccountService/>
    <Debug>
        <CrashDumpWindows>0</CrashDumpWindows>
        <GDBPort>1337</GDBPort>
    </Debug>
    <Input>
        <DSUC host="127.0.0.1" port="26760"/>
    </Input>
    <EmulatedUsbDevices>
        <EmulateSkylanderPortal>false</EmulateSkylanderPortal>
        <EmulateInfinityBase>false</EmulateInfinityBase>
        <EmulateDimensionsToypad>false</EmulateDimensionsToypad>
    </EmulatedUsbDevices>
</content>
"""


def _unique_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for candidate in paths:
        key = str(candidate).casefold()
        if not key or key in seen:
            continue
        seen.add(key)
        unique.append(candidate)
    return unique


def cemu_settings_path_candidates(emulator_path_text: str) -> list[Path]:
    candidates: list[Path] = []

    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if path_text:
        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if str(emulator_dir):
            candidates.append(emulator_dir / "portable" / "settings.xml")
            candidates.append(emulator_dir / "settings.xml")

    if sys.platform == "win32":
        for env_name in ("APPDATA", "LOCALAPPDATA"):
            env_value = os.environ.get(env_name, "")
            if isinstance(env_value, str) and env_value.strip():
                candidates.append(Path(env_value).expanduser() / "Cemu" / "settings.xml")
    else:
        xdg_config = xdg_config_home()
        if xdg_config is not None:
            candidates.append(xdg_config / "Cemu" / "settings.xml")
        candidates.append(Path.home() / ".var" / "app" / "info.cemu.Cemu" / "config" / "Cemu" / "settings.xml")

    return _unique_paths(candidates)


def _ensure_xml_element_value(root, tag: str, value: str) -> bool:
    elem = root.find(tag)
    if elem is None:
        elem = ET.SubElement(root, tag)
        elem.text = value
        return True
    if elem.text == value:
        return False
    elem.text = value
    return True


def _first_existing_or_writable_path(candidates: list[Path]) -> Path | None:
    existing = next((candidate for candidate in candidates if candidate.exists() and candidate.is_file()), None)
    if existing is not None:
        return existing

    for candidate in candidates:
        parent = candidate.parent
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            continue
        return candidate

    return None


def ensure_cemu_settings(emulator_path_text: str) -> dict:
    try:
        path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
        if not path_text:
            return {"config_path": None, "changed": False}

        emulator_path = Path(path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if not str(emulator_dir):
            return {"config_path": None, "changed": False}

        if sys.platform == "win32":
            portable_dir = emulator_dir / "portable"
            portable_dir.mkdir(parents=True, exist_ok=True)
            target = portable_dir / "settings.xml"
        else:
            target = _first_existing_or_writable_path(cemu_settings_path_candidates(path_text))
            if target is None:
                return {"config_path": None, "changed": False}

        if not target.exists():
            target.write_text(_DEFAULT_CEMU_SETTINGS_XML, encoding="utf-8")
            return {"config_path": str(target), "changed": True}

        content = target.read_text(encoding="utf-8")
        if not content.strip():
            raise ET.ParseError("empty settings.xml")
        parsed_root = ET.fromstring(content)
        root = parsed_root if parsed_root.tag == "content" else parsed_root.find(".//content")
        if root is None:
            raise ET.ParseError("missing content root")

        changed = False

        changed = _ensure_xml_element_value(root, "use_discord_presence", "false") or changed
        changed = _ensure_xml_element_value(root, "check_update", "false") or changed
        changed = _ensure_xml_element_value(root, "receive_untested_updates", "false") or changed
        changed = _ensure_xml_element_value(root, "gp_download", "true") or changed
        changed = _ensure_xml_element_value(root, "fullscreen", "false") or changed
        changed = _ensure_xml_element_value(root, "window_maximized", "true") or changed

        if changed:
            xml_body = ET.tostring(root, encoding="unicode", xml_declaration=False)
            target.write_text(f'<?xml version="1.0" encoding="utf-8"?>\n{xml_body}', encoding="utf-8")

        return {"config_path": str(target), "changed": changed}
    except Exception:
        return {"config_path": None, "changed": False}


def ensure_cemu_controller_config(emulator_path_text: str) -> dict:
    """Write default Cemu controller profile to portable/controllerProfiles/controller0.xml if not already present."""
    changed = False
    profile_path: str | None = None

    try:
        emulator_path = Path(emulator_path_text).expanduser()
        emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
        if sys.platform == "win32":
            target = emulator_dir / "portable" / "controllerProfiles" / "controller0.xml"
        else:
            settings_candidates = cemu_settings_path_candidates(emulator_path_text)
            existing_settings = next(
                (candidate for candidate in settings_candidates if candidate.exists() and candidate.is_file()),
                settings_candidates[0] if settings_candidates else None,
            )
            if existing_settings is None:
                return {"profile_path": None, "changed": False}
            target = existing_settings.parent / "controllerProfiles" / "controller0.xml"

        if target.exists():
            return {"profile_path": str(target), "changed": False}

        target.parent.mkdir(parents=True, exist_ok=True)
        profile = _DEFAULT_CEMU_SDL_CONTROLLER_PROFILE if sys.platform != "win32" else _DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE
        target.write_text(profile, encoding="utf-8")
        changed = True
        profile_path = str(target)
    except OSError:
        profile_path = None

    return {"profile_path": profile_path, "changed": changed}


def cemu_directory_settings(emulator_path_text: str) -> dict[str, str]:
    defaults = {
        "config_path": "",
        "mlc_path": "",
    }

    for candidate in cemu_settings_path_candidates(emulator_path_text):
        if not candidate.exists() or not candidate.is_file():
            continue

        try:
            root = ET.fromstring(candidate.read_text(encoding="utf-8"))
        except (ET.ParseError, OSError):
            continue

        candidate_nodes = [root, *root.findall(".//content")]
        for node in candidate_nodes:
            raw_mlc_path = node.findtext("mlc_path", default="")
            if isinstance(raw_mlc_path, str) and raw_mlc_path.strip():
                defaults["config_path"] = str(candidate)
                defaults["mlc_path"] = raw_mlc_path.strip()
                return defaults

    return defaults


def _save_root_from_mlc_path(raw_path: str) -> str:
    mlc_path = raw_path.strip().strip('"').strip("'")
    if not mlc_path:
        return ""

    normalized = re.sub(r"[\\/]+", "/", mlc_path).rstrip("/").casefold()
    if normalized.endswith("/usr/save"):
        return mlc_path.rstrip("\\/")
    return str(Path(mlc_path) / "usr" / "save")


def cemu_save_path_overrides(
    emulator_path_text: str,
    launch_template: str,
    split_launch_template_args: Callable[[str], list[str]],
) -> list[str]:
    raw_mlc_paths: list[str] = []

    template = launch_template.strip() if isinstance(launch_template, str) else ""
    if template:
        try:
            args = split_launch_template_args(template)
        except ValueError:
            args = []

        for index, raw_arg in enumerate(args):
            if not isinstance(raw_arg, str) or not raw_arg.strip():
                continue
            normalized_arg = raw_arg.strip()
            lowered_arg = normalized_arg.casefold()

            if lowered_arg in {"-m", "--mlc"} and index + 1 < len(args):
                next_arg = args[index + 1]
                if isinstance(next_arg, str) and next_arg.strip():
                    raw_mlc_paths.append(next_arg.strip())
                continue

            if lowered_arg.startswith("--mlc=") or lowered_arg.startswith("-m="):
                _, value = normalized_arg.split("=", 1)
                if value.strip():
                    raw_mlc_paths.append(value.strip())

    settings = cemu_directory_settings(emulator_path_text)
    configured_mlc_path = settings.get("mlc_path", "")
    if isinstance(configured_mlc_path, str) and configured_mlc_path.strip():
        raw_mlc_paths.append(configured_mlc_path.strip())

    resolved: list[str] = []
    seen: set[str] = set()
    for raw_path in raw_mlc_paths:
        save_root = _save_root_from_mlc_path(raw_path)
        key = save_root.casefold()
        if not save_root or key in seen:
            continue
        seen.add(key)
        resolved.append(save_root)
    return resolved
