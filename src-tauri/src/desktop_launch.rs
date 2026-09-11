use std::{
    path::Path,
    process::{Command, Stdio},
};

pub(crate) const DESKTOP_LAUNCH_ARGUMENT: &str = "--construct-desktop-launch";

pub(crate) fn launch_detached(
    executable: &Path,
    arguments: &[String],
    current_directory: &Path,
) -> Result<(), String> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(current_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;

        // The terminal can close immediately after the launcher returns. Give
        // the desktop process its own session so that closing the terminal does
        // not send it the terminal session's SIGHUP.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not launch Construct: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_launch_returns_without_waiting_for_the_child() {
        let started = std::time::Instant::now();
        let arguments = vec![
            "-c".to_string(),
            "sleep 1".to_string(),
            "--construct-desktop-child".to_string(),
        ];

        launch_detached(Path::new("/bin/sh"), &arguments, Path::new("/"))
            .expect("start detached child");

        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "the launcher must not wait for the desktop child"
        );
    }

    #[test]
    fn launcher_arguments_keep_paths_as_distinct_values() {
        let arguments = [
            "--construct-desktop-child".to_string(),
            "notes with spaces/ação.md".to_string(),
        ];

        assert_eq!(arguments[0], "--construct-desktop-child");
        assert_eq!(arguments[1], "notes with spaces/ação.md");
    }
}
