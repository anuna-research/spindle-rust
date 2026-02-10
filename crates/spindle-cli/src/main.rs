//! Spindle CLI - Command-line interface for defeasible logic reasoning

#[cfg(test)]
mod tests;

mod cli;

use chrono::DateTime;
use clap::Parser;
use clap::error::ErrorKind;
use spindle_core::temporal::TimePoint;

use cli::app::{Cli, Commands};
use cli::commands::{capabilities, explain, query, reason, requires, stats, validate, why_not};
use cli::error::CliError;
use cli::output::emit_and_exit;

fn main() {
    let json_requested_in_raw_args = std::env::args_os()
        .skip(1)
        .any(|arg| arg == std::ffi::OsStr::new("--json"));

    let cli =
        match Cli::try_parse() {
            Ok(cli) => cli,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    // Keep help/version textual and user-friendly.
                    let _ = err.print();
                    std::process::exit(0);
                }
                kind => {
                    if json_requested_in_raw_args {
                        emit_and_exit(
                            Err(CliError::validation("CLI_PARSE_ERROR", err.to_string())
                                .with_details(serde_json::json!({
                                    "kind": format!("{kind:?}")
                                }))),
                            None,
                            true,
                        );
                    } else {
                        err.exit();
                    }
                }
            },
        };

    // Determine schema version and json flag based on the command before we move out of cli.command
    let (schema_version, json_flag) = match &cli.command {
        Commands::Reason { json, .. } => (Some("spindle.reason.v1"), *json || cli.json),
        Commands::Query { json, .. } => (Some("spindle.query.v1"), *json || cli.json),
        Commands::Explain { json, .. } => (Some("spindle.explain.v1"), *json || cli.json),
        Commands::WhyNot { json, .. } => (Some("spindle.why_not.v1"), *json || cli.json),
        Commands::Requires { json, .. } => (Some("spindle.requires.v1"), *json || cli.json),
        Commands::Capabilities { json, .. } => (Some("spindle.capabilities.v1"), *json || cli.json),
        Commands::Validate { .. } | Commands::Stats { .. } => (None, cli.json),
    };

    // Parse reference time if provided (after determining json_flag so errors use correct format)
    let reference_time = if let Some(ref s) = cli.at {
        match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => Some(TimePoint::from_millis(dt.timestamp_millis())),
            Err(e) => {
                emit_and_exit(
                    Err(CliError::parse(
                        "INVALID_TIME_FORMAT",
                        format!("Error parsing time '{s}': {e}"),
                    )),
                    schema_version,
                    json_flag,
                );
            }
        }
    } else {
        None
    };

    let result = match cli.command {
        Commands::Reason {
            file,
            scalable,
            positive,
            json: _,
        } => reason::run_reason(
            file.as_ref(),
            scalable,
            positive,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Validate { file, stdin } => {
            validate::run_validate(file.as_ref(), stdin || cli.stdin, json_flag)
        }
        Commands::Stats { file, stdin } => {
            stats::run_stats(file.as_ref(), stdin || cli.stdin, json_flag)
        }
        Commands::Query {
            literal,
            file,
            json: _,
        } => query::run_query(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Explain {
            literal,
            file,
            json: _,
        } => explain::run_explain(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::WhyNot {
            literal,
            file,
            json: _,
        } => why_not::run_why_not(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Requires {
            literal,
            file,
            max,
            json: _,
        } => requires::run_requires(
            file.as_ref(),
            &literal,
            max,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Capabilities { json: _ } => capabilities::run_capabilities(json_flag),
    };

    emit_and_exit(result, schema_version, json_flag);
}
