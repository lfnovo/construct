use std::{
    path::Path,
    process::{Command, Stdio},
};

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

    #[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn detached_launch_keeps_paths_as_distinct_child_arguments() {
        let output = std::env::temp_dir().join(format!(
            "construct-desktop-launch-{}.txt",
            uuid::Uuid::new_v4()
        ));
        let path = "notes with spaces/ação.md".to_string();
        let arguments = vec![
            "-c".to_string(),
            "printf '%s\\n%s\\n' \"$1\" \"$2\" > \"$3\"".to_string(),
            "launcher".to_string(),
            crate::DESKTOP_CHILD_ARGUMENT.to_string(),
            path.clone(),
            output.to_string_lossy().to_string(),
        ];

        launch_detached(Path::new("/bin/sh"), &arguments, Path::new("/"))
            .expect("start detached child");
        for _ in 0..50 {
            if output.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(
            std::fs::read_to_string(&output).expect("read recorded child arguments"),
            format!("{}\n{path}\n", crate::DESKTOP_CHILD_ARGUMENT),
        );
        let _ = std::fs::remove_file(output);
    }
}
