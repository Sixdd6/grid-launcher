//! The one place GRID starts the Steam client: `steam://` links handed to
//! Steam's executable (`docs/stores-phase4-plan.md` "How GRID opens them").
//!
//! The child is detached (its own process group on unix, a detached
//! process on Windows) with null stdin, stdout and stderr: nothing Steam
//! prints (its console names accounts) ever reaches GRID. It is never
//! waited on in the caller's thread; a reaper thread waits for it, so it
//! never stays a zombie (spike 3: `std` does not reap a dropped child).

use std::ffi::OsString;
use std::io;
use std::process::{Command, Stdio};

use super::paths::FLATPAK_ID;
use super::{ClientExe, SteamLink};

/// The program and arguments that hand `link` to the client:
/// `<exe> [-silent] <link>`, `flatpak run com.valvesoftware.Steam [-silent]
/// <link>`, or the OS's link opener (`xdg-open`, `open`, `explorer`) with
/// the link alone. Only [`SteamLink::Run`] gets `-silent` (the other links
/// must show Steam's dialog).
pub fn argv(exe: &ClientExe, link: SteamLink) -> (OsString, Vec<OsString>) {
    let mut args: Vec<OsString> = Vec::new();
    let silent = || link.silent().then(|| OsString::from("-silent"));
    let program = match exe {
        ClientExe::Path(path) => {
            args.extend(silent());
            path.clone().into_os_string()
        }
        ClientExe::Flatpak => {
            args.push("run".into());
            args.push(FLATPAK_ID.into());
            args.extend(silent());
            OsString::from("flatpak")
        }
        ClientExe::XdgOpen => OsString::from(link_opener()),
    };
    args.push(link.to_string().into());
    (program, args)
}

/// This OS's "open a URL with its handler" program.
fn link_opener() -> &'static str {
    if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

/// Starts the client with `link`: detached, null stdio, reaped by a
/// background thread. An exec error is returned for the app to word.
pub fn open(exe: &ClientExe, link: SteamLink) -> io::Result<()> {
    let (program, args) = argv(exe, link);
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .envs(crate::launch::spawn::clean_env())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group: a signal to GRID's group never reaches
        // the Steam client it started.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = command.spawn()?;
    let reaper = std::thread::Builder::new()
        .name("steam-open-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        });
    if reaper.is_err() {
        tracing::debug!("Steam: no reaper thread for the client process");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn shown(exe: &ClientExe, link: SteamLink) -> Vec<String> {
        let (program, args) = argv(exe, link);
        std::iter::once(program)
            .chain(args)
            .map(|part| part.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn argv_per_client_shape() {
        let native = ClientExe::Path(PathBuf::from("/usr/bin/steam"));
        assert_eq!(
            shown(&native, SteamLink::Run(620)),
            ["/usr/bin/steam", "-silent", "steam://rungameid/620"]
        );
        assert_eq!(
            shown(&native, SteamLink::Install(620)),
            ["/usr/bin/steam", "steam://install/620"]
        );
        assert_eq!(
            shown(&native, SteamLink::Uninstall(620)),
            ["/usr/bin/steam", "steam://uninstall/620"]
        );
        assert_eq!(
            shown(&native, SteamLink::Validate(620)),
            ["/usr/bin/steam", "steam://validate/620"]
        );
        assert_eq!(
            shown(&ClientExe::Flatpak, SteamLink::Run(620)),
            [
                "flatpak",
                "run",
                "com.valvesoftware.Steam",
                "-silent",
                "steam://rungameid/620"
            ]
        );
        assert_eq!(
            shown(&ClientExe::Flatpak, SteamLink::Install(620)),
            [
                "flatpak",
                "run",
                "com.valvesoftware.Steam",
                "steam://install/620"
            ]
        );
        let opener = shown(&ClientExe::XdgOpen, SteamLink::Run(620));
        assert_eq!(opener.len(), 2);
        assert_eq!(opener[1], "steam://rungameid/620");
        if cfg!(target_os = "linux") {
            assert_eq!(opener[0], "xdg-open");
        }
    }

    #[test]
    fn a_missing_executable_is_an_io_error() {
        let exe = ClientExe::Path(PathBuf::from("/nonexistent/grid-test/steam"));
        assert!(open(&exe, SteamLink::Run(620)).is_err());
    }
}
