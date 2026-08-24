use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let needs_stdin = matches!(
        args.first().map(String::as_str),
        Some("code-context" | "format-code")
    );
    let stdin = if needs_stdin {
        let mut input = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut input) {
            eprintln!("Failed to read stdin: {error}");
            return ExitCode::FAILURE;
        }
        Some(input)
    } else {
        None
    };

    match voiceflow_lib::services::platform_quality::run_bridge_cli(&args, stdin.as_deref(), None) {
        Ok(response) => {
            match serde_json::to_string_pretty(&response) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("Failed to encode bridge response: {error}");
                    return ExitCode::FAILURE;
                }
            }
            if response.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
