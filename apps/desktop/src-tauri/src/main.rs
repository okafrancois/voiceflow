#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Read;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(cli_args) = bundled_cli_args(&args) {
        return run_bundled_cli(cli_args);
    }
    voiceflow_lib::run();
    ExitCode::SUCCESS
}

fn bundled_cli_args(args: &[String]) -> Option<&[String]> {
    (args.first().map(String::as_str) == Some("--cli")).then(|| &args[1..])
}

fn run_bundled_cli(args: &[String]) -> ExitCode {
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

    match voiceflow_lib::services::platform_quality::run_bridge_cli(args, stdin.as_deref(), None) {
        Ok(response) => match serde_json::to_string_pretty(&response) {
            Ok(json) => {
                println!("{json}");
                if response.ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(error) => {
                eprintln!("Failed to encode bridge response: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bundled_cli_args;

    #[test]
    fn bundled_cli_mode_requires_an_explicit_prefix_and_preserves_arguments() {
        let cli = vec!["--cli".to_string(), "status".to_string()];
        let desktop = vec!["status".to_string()];

        assert_eq!(bundled_cli_args(&cli), Some(&cli[1..]));
        assert_eq!(bundled_cli_args(&desktop), None);
    }
}
