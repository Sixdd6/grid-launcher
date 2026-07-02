# -*- mode: python ; coding: utf-8 -*-


a = Analysis(
    ['grid-launcher.py'],
    pathex=[],
    binaries=[],
    datas=[('assets', 'assets'), ('retroarch-core-list.json', '.'), ('romm-platform-cores.json', '.'), ('emulator-autoprofiles.json', '.')],
    hiddenimports=['brotli', 'pyzstd', 'pyppmd', 'keyring.backends.Windows', 'keyring.backends.SecretService', 'keyring.backends.kwallet'],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='grid-launcher',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
