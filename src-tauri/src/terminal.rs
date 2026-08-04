use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(target_os = "windows")]
use std::env;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TerminalApplicationId {
    AppleTerminal,
    Iterm2,
    Ghostty,
    Wezterm,
    Warp,
    SystemDefault,
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
    working_directory: Option<PathBuf>,
    interactive_console: bool,
    new_console: bool,
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
        windows_applications()
            .into_iter()
            .map(|(application, _)| application)
            .collect()
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
    command.args(&spec.arguments);
    if let Some(working_directory) = &spec.working_directory {
        command.current_dir(working_directory);
    }
    if !spec.interactive_console {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    #[cfg(target_os = "windows")]
    if spec.new_console {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        command.creation_flags(CREATE_NEW_CONSOLE);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = spec.new_console;
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
        let spec = macos_launch_spec(application_id, app_path, directory)?;
        Ok((application, spec))
    }
    #[cfg(target_os = "windows")]
    {
        let (application, program) = windows_applications()
            .into_iter()
            .find(|(application, _)| application.id == application_id)
            .ok_or_else(|| "The selected terminal application is not installed.".to_string())?;
        let spec = match application_id {
            TerminalApplicationId::SystemDefault => LaunchSpec {
                program,
                arguments: Vec::new(),
                working_directory: Some(directory.to_path_buf()),
                interactive_console: true,
                new_console: true,
                wait_for_exit: false,
            },
            TerminalApplicationId::WindowsTerminal => LaunchSpec {
                program,
                arguments: vec![OsString::from("-d"), directory.as_os_str().to_os_string()],
                working_directory: None,
                interactive_console: false,
                new_console: false,
                wait_for_exit: false,
            },
            TerminalApplicationId::Warp => LaunchSpec {
                program,
                arguments: Vec::new(),
                working_directory: Some(directory.to_path_buf()),
                interactive_console: false,
                new_console: false,
                wait_for_exit: false,
            },
            _ => {
                return Err(
                    "The selected terminal application is not available on Windows.".to_string(),
                )
            }
        };
        Ok((application, spec))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = application_id;
        let _ = directory;
        Err("Opening a terminal is not supported on this platform.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn macos_launch_spec(
    application_id: TerminalApplicationId,
    app_path: PathBuf,
    directory: &Path,
) -> Result<LaunchSpec, String> {
    let directory = directory.as_os_str().to_os_string();
    match application_id {
        TerminalApplicationId::AppleTerminal | TerminalApplicationId::Iterm2 => Ok(LaunchSpec {
            program: PathBuf::from("/usr/bin/open"),
            arguments: vec![OsString::from("-a"), app_path.into_os_string(), directory],
            working_directory: None,
            interactive_console: false,
            new_console: false,
            wait_for_exit: true,
        }),
        TerminalApplicationId::Ghostty => {
            let mut working_directory = OsString::from("--working-directory=");
            working_directory.push(directory);
            Ok(LaunchSpec {
                program: PathBuf::from("/usr/bin/open"),
                arguments: vec![
                    OsString::from("-na"),
                    app_path.into_os_string(),
                    OsString::from("--args"),
                    working_directory,
                ],
                working_directory: None,
                interactive_console: false,
                new_console: false,
                wait_for_exit: true,
            })
        }
        TerminalApplicationId::Wezterm => Ok(LaunchSpec {
            program: app_path.join("Contents/MacOS/wezterm"),
            arguments: vec![OsString::from("start"), OsString::from("--cwd"), directory],
            working_directory: None,
            interactive_console: false,
            new_console: false,
            wait_for_exit: false,
        }),
        TerminalApplicationId::Warp => Ok(LaunchSpec {
            program: PathBuf::from("/usr/bin/open"),
            arguments: vec![OsString::from("-a"), app_path.into_os_string(), directory],
            working_directory: None,
            interactive_console: false,
            new_console: false,
            wait_for_exit: true,
        }),
        TerminalApplicationId::SystemDefault | TerminalApplicationId::WindowsTerminal => {
            Err("The selected terminal application is not available on macOS.".to_string())
        }
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
    if let Some(path) = find_user_application(&["Warp.app"]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::Warp,
                label: "Warp".to_string(),
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
fn windows_applications() -> Vec<(TerminalApplication, PathBuf)> {
    let mut applications = vec![(
        TerminalApplication {
            id: TerminalApplicationId::SystemDefault,
            label: "System default".to_string(),
        },
        windows_system_shell(),
    )];
    if let Some(path) = find_windows_executable("wt.exe", &[]) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::WindowsTerminal,
                label: "Windows Terminal".to_string(),
            },
            path,
        ));
    }
    let mut warp_candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        warp_candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("Warp")
                .join("warp.exe"),
        );
    }
    for variable in ["ProgramFiles", "ProgramW6432"] {
        if let Some(program_files) = env::var_os(variable) {
            warp_candidates.push(PathBuf::from(program_files).join("Warp").join("warp.exe"));
        }
    }
    if let Some(path) = find_windows_executable("warp.exe", &warp_candidates) {
        applications.push((
            TerminalApplication {
                id: TerminalApplicationId::Warp,
                label: "Warp".to_string(),
            },
            path,
        ));
    }
    applications
}

#[cfg(target_os = "windows")]
fn windows_system_shell() -> PathBuf {
    env::var_os("COMSPEC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cmd.exe"))
}

#[cfg(target_os = "windows")]
fn find_windows_executable(executable: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = candidates.iter().find(|path| path.is_file()) {
        return Some(path.clone());
    }
    let output = Command::new("where.exe")
        .arg(executable)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.is_file())
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

    #[cfg(target_os = "macos")]
    #[test]
    fn warp_launch_opens_the_validated_directory_as_a_folder() {
        let spec = macos_launch_spec(
            TerminalApplicationId::Warp,
            PathBuf::from("/Applications/Warp.app"),
            Path::new("/tmp/Folder with spaces"),
        )
        .expect("build Warp launch spec");
        assert_eq!(spec.program, PathBuf::from("/usr/bin/open"));
        assert_eq!(
            spec.arguments,
            vec![
                OsString::from("-a"),
                OsString::from("/Applications/Warp.app"),
                OsString::from("/tmp/Folder with spaces"),
            ]
        );
        assert!(spec.wait_for_exit);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ghostty_launch_uses_launch_services_with_a_working_directory() {
        let spec = macos_launch_spec(
            TerminalApplicationId::Ghostty,
            PathBuf::from("/Applications/Ghostty.app"),
            Path::new("/tmp/Folder with spaces"),
        )
        .expect("build Ghostty launch spec");
        assert_eq!(spec.program, PathBuf::from("/usr/bin/open"));
        assert_eq!(
            spec.arguments,
            vec![
                OsString::from("-na"),
                OsString::from("/Applications/Ghostty.app"),
                OsString::from("--args"),
                OsString::from("--working-directory=/tmp/Folder with spaces"),
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
        assert_eq!(
            serde_json::to_string(&TerminalApplicationId::Warp).expect("serialize application"),
            "\"warp\""
        );
        assert_eq!(
            serde_json::to_string(&TerminalApplicationId::SystemDefault)
                .expect("serialize application"),
            "\"system-default\""
        );
    }
}
