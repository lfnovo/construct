#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationMode {
    Desktop,
    Mcp,
    Okf,
    Service,
}

fn invocation_mode(arguments: &[String]) -> InvocationMode {
    match arguments.first().map(String::as_str) {
        Some("okf") => InvocationMode::Okf,
        Some("service") => InvocationMode::Service,
        Some("mcp") if arguments.get(1).map(String::as_str) == Some("serve") => InvocationMode::Mcp,
        _ => InvocationMode::Desktop,
    }
}

#[cfg(all(target_os = "windows", feature = "desktop"))]
fn detach_desktop_console() {
    // SAFETY: FreeConsole accepts no pointers and only detaches the calling
    // process from its current console. Failure means there was no console to
    // detach from, which is already the desired desktop state.
    let _ = unsafe { windows_sys::Win32::System::Console::FreeConsole() };
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = invocation_mode(&arguments);

    if mode == InvocationMode::Okf {
        match construct_lib::run_okf_command(&arguments[1..]) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("construct okf: {error}");
                std::process::exit(2);
            }
        }
    }

    #[cfg(not(feature = "desktop"))]
    {
        eprintln!(
            "This standalone Construct build only supports `construct okf lint`; \
             run `construct okf lint --help` for usage."
        );
        std::process::exit(2);
    }

    #[cfg(feature = "desktop")]
    {
        let result = match mode {
            InvocationMode::Service => Some(construct_lib::run_service_command(&arguments[1..])),
            InvocationMode::Mcp => Some(construct_lib::run_mcp_command(&arguments[2..])),
            InvocationMode::Desktop => None,
            InvocationMode::Okf => unreachable!("OKF mode exits before desktop dispatch"),
        };
        if let Some(result) = result {
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        #[cfg(target_os = "windows")]
        detach_desktop_console();
        construct_lib::run();
    }
}

#[cfg(test)]
mod tests {
    use super::{invocation_mode, InvocationMode};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn classifies_empty_and_unknown_arguments_as_desktop() {
        assert_eq!(invocation_mode(&[]), InvocationMode::Desktop);
        assert_eq!(
            invocation_mode(&arguments(&["--unexpected"])),
            InvocationMode::Desktop
        );
        assert_eq!(
            invocation_mode(&arguments(&["mcp"])),
            InvocationMode::Desktop
        );
    }

    #[test]
    fn classifies_console_modes_without_detaching_them() {
        assert_eq!(
            invocation_mode(&arguments(&["okf", "lint", "."])),
            InvocationMode::Okf
        );
        assert_eq!(
            invocation_mode(&arguments(&["service", "--data-dir", "."])),
            InvocationMode::Service
        );
        assert_eq!(
            invocation_mode(&arguments(&["mcp", "serve", "--allow-all"])),
            InvocationMode::Mcp
        );
    }
}
