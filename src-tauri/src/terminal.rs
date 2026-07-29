use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TerminalApplicationId {
    AppleTerminal,
    Iterm2,
    Ghostty,
    Wezterm,
    WindowsTerminal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalApplication {
    pub(crate) id: TerminalApplicationId,
    pub(crate) label: String,
}

struct LaunchSpec {
    program: PathBuf,
    arguments: Vec<OsString>,
    wait_for_exit: bool,
}

pub(crate) fn available_applications() -> Vec<TerminalApplication> {
    #[cfg(target_os = "macos")]
    {
        macos_applications()
            .into_iter()
            .map(|(application, _)| application)
            .collect()
    }
    #[cfg(target_os = "windows")]
    {
        if windows_terminal_available() {
            vec![TerminalApplication {
                id: TerminalApplicationId::WindowsTerminal,
                label: "Windows Terminal".to_string(),
            }]
        } else {
            Vec::new()
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

pub(crate) fn launch(
    application_id: TerminalApplicationId,
    directory: &Path,
) -> Result<TerminalApplication, String> {
    let (application, spec) = launch_spec(application_id, directory)?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if spec.wait_for_exit {
        let status = command.status().map_err(|error| {
            format!(
                "Could not open {} at '{}': {error}",
                application.label,
                directory.display()
            )
        })?;
        if !status.success() {
            return Err(format!(
                "Could not open {} at '{}'.",
                application.label,
                directory.display()
            ));
        }
    } else {
        command.spawn().map_err(|error| {
            format!(
                "Could not open {} at '{}': {error}",
                application.label,
                directory.display()
            )
        })?;
    }
    Ok(application)
}

fn launch_spec(
    application_id: TerminalApplicationId,
    directory: &Path,
) -> Result<(TerminalApplication, LaunchSpec), String> {
    #[cfg(target_os = "macos")]
    {
        let (application, app_path) = macos_applications()
            .into_iter()
            .find(|(application, _)| application.id == application_id)
            .ok_or_else(|| "The selected terminal application is not installed.".to_string())?;
        let directory = directory.as_os_str().to_os_string();
        let spec = match application_id {
            TerminalApplicationId::AppleTerminal | TerminalApplicationId::Iterm2 => LaunchSpec {
                program: PathBuf::from("/usr/bin/open"),
                arguments: vec![OsString::from("-a"), app_path.into_os_string(), directory],
                wait_for_exit: true,
            },
            TerminalApplicationId::Ghostty => LaunchSpec {
                program: app_path.join("Contents/MacOS/ghostty"),
                arguments: vec![
                    OsString::from("+new-window"),
                    OsString::from("--working-directory"),
                    directory,
                ],
                wait_for_exit: false,
            },
            TerminalApplicationId::Wezterm => LaunchSpec {
                program: app_path.join("Contents/MacOS/wezterm"),
                arguments: vec![OsString::from("start"), OsString::from("--cwd"), directory],
                wait_for_exit: false,
            },
            TerminalApplicationId::WindowsTerminal => {
                return Err("Windows Terminal is not available on macOS.".to_string())
            }
        };
        Ok((application, spec))
    }
    #[cfg(target_os = "windows")]
    {
        if application_id != TerminalApplicationId::WindowsTerminal || !windows_terminal_available()
        {
            return Err("The selected terminal application is not installed.".to_string());
        }
        Ok((
            TerminalApplication {
                id: TerminalApplicationId::WindowsTerminal,
                label: "Windows Terminal".to_string(),
            },
            LaunchSpec {
                program: PathBuf::from("wt.exe"),
                arguments: vec![OsString::from("-d"), directory.as_os_str().to_os_string()],
                wait_for_exit: false,
            },
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = application_id;
        let _ = directory;
        Err("Opening a terminal is not supported on this platform.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn macos_applications() -> Vec<(TerminalApplication, PathBuf)> {
    let mut applications = Vec::new();
    if let Some(path) = first_existing_path(&[
        PathBuf::from("/System/Applications/Utilities/Terminal.app"),
        PathBuf::from("/Applications/Utilities/Terminal.app"),
    ]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::AppleTerminal,
                label: "Terminal".to_string(),
            },
            path,
        ));
    }
    if let Some(path) = find_user_application(&["iTerm.app", "iTerm2.app"]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::Iterm2,
                label: "iTerm2".to_string(),
            },
            path,
        ));
    }
    if let Some(path) = find_user_application(&["Ghostty.app"]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::Ghostty,
                label: "Ghostty".to_string(),
            },
            path,
        ));
    }
    if let Some(path) = find_user_application(&["WezTerm.app"]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::Wezterm,
                label: "WezTerm".to_string(),
            },
            path,
        ));
    }
    applications
}

#[cfg(target_os = "macos")]
fn find_user_application(names: &[&str]) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for name in names {
        candidates.push(PathBuf::from("/Applications").join(name));
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Applications").join(name));
        }
    }
    first_existing_path(&candidates)
}

#[cfg(target_os = "macos")]
fn first_existing_path(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|path| path.is_dir()).cloned()
}

#[cfg(target_os = "windows")]
fn windows_terminal_available() -> bool {
    Command::new("where.exe")
        .arg("wt.exe")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_terminal_launch_uses_structured_arguments() {
        let directory = Path::new("/tmp/Folder with spaces");
        let (_, spec) = launch_spec(TerminalApplicationId::AppleTerminal, directory)
            .expect("Apple Terminal should be available on macOS");
        assert_eq!(spec.program, PathBuf::from("/usr/bin/open"));
        assert_eq!(
            spec.arguments,
            vec![
                OsString::from("-a"),
                OsString::from("/System/Applications/Utilities/Terminal.app"),
                OsString::from("/tmp/Folder with spaces"),
            ]
        );
        assert!(spec.wait_for_exit);
    }

    #[test]
    fn application_ids_have_stable_serialized_values() {
        assert_eq!(
            serde_json::to_string(&TerminalApplicationId::AppleTerminal)
                .expect("serialize application"),
            "\"apple-terminal\""
        );
        assert_eq!(
            serde_json::to_string(&TerminalApplicationId::WindowsTerminal)
                .expect("serialize application"),
            "\"windows-terminal\""
        );
    }
}
