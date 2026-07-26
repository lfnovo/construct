fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let result = match arguments.first().map(String::as_str) {
        Some("service") => Some(construct_lib::run_service_command(&arguments[1..])),
        Some("mcp") if arguments.get(1).map(String::as_str) == Some("serve") => {
            Some(construct_lib::run_mcp_command(&arguments[2..]))
        }
        _ => None,
    };
    if let Some(result) = result {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    construct_lib::run();
}
