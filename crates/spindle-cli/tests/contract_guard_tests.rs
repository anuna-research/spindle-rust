//! Contract Guard Tests
//!
//! Meta tests that scan the source code to prevent regression on contract boundaries.
//! These tests ensure:
//! 1. No direct std::process::exit calls outside the boundary emitter
//! 2. No eprintln!() calls in command handlers
//! 3. No deprecated Command::cargo_bin usage in tests
//!
//! This is intentionally structural to prevent future bypasses.

use std::fs;
use std::path::PathBuf;

/// Get the path to main.rs
fn main_rs_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/main.rs");
    path
}

/// Read the main.rs source file
fn read_main_rs() -> String {
    fs::read_to_string(main_rs_path()).expect("Failed to read main.rs")
}

/// Find all occurrences of a pattern in the source
fn find_occurrences(source: &str, pattern: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            if line.contains(pattern)
                && !line.trim().starts_with("//")
                && !line.trim().starts_with("///")
                && !line.trim().starts_with("//!")
            {
                Some((i + 1, line.to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Find all function boundaries in the source
/// Returns a list of (function_name, start_line, end_line)
fn find_function_boundaries(source: &str) -> Vec<(String, usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut functions = Vec::new();
    let mut current_fn: Option<(String, usize)> = None;
    let mut brace_depth = 0;
    let mut entered_body = false;

    for (i, line) in lines.iter().enumerate() {
        let line_number = i + 1;

        // Check if this line starts a function
        if let Some(fn_name) = extract_fn_name(line) {
            // If we were tracking a previous function, close it
            if let Some((name, start)) = current_fn.take() {
                functions.push((name, start, line_number - 1));
            }
            current_fn = Some((fn_name, line_number));
            brace_depth = 0;
            entered_body = false;
        }

        if let Some((_, start_line)) = current_fn {
            // Count braces
            let open_braces = line.chars().filter(|&c| c == '{').count() as i32;
            let close_braces = line.chars().filter(|&c| c == '}').count() as i32;

            brace_depth += open_braces - close_braces;

            // Mark that we've entered the function body once we see an opening brace
            if open_braces > 0 {
                entered_body = true;
            }

            // If we've entered the body and brace_depth returns to 0, function is complete
            if entered_body && brace_depth == 0 && line_number > start_line {
                if let Some((name, start)) = current_fn.take() {
                    functions.push((name, start, line_number));
                }
                entered_body = false;
            }
        }
    }

    // Handle case where file ends with a function
    if let Some((name, start)) = current_fn {
        functions.push((name, start, lines.len()));
    }

    functions
}

/// Extract function name from a line like "fn foo(" or "fn foo <"
fn extract_fn_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("fn ") {
        return None;
    }

    // Extract function name after "fn "
    let after_fn = &trimmed[3..];
    // Take until first non-identifier character
    let name: String = after_fn
        .chars()
        .take_while(|&c| c.is_alphanumeric() || c == '_')
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Check if a line is inside the emit_and_exit function
fn is_in_emit_and_exit_function(source: &str, line_num: usize) -> bool {
    let functions = find_function_boundaries(source);

    for (name, start, end) in functions {
        if name == "emit_and_exit" {
            return line_num >= start && line_num <= end;
        }
    }

    false
}

/// Check if a line is in a command handler function
fn is_in_command_handler(source: &str, line_num: usize) -> bool {
    let handler_names = [
        "run_reason",
        "run_validate",
        "run_stats",
        "run_query",
        "run_explain",
        "run_why_not",
        "run_requires",
        "run_capabilities",
    ];

    let functions = find_function_boundaries(source);

    for (name, start, end) in functions {
        if handler_names.contains(&name.as_str()) {
            if line_num >= start && line_num <= end {
                return true;
            }
        }
    }

    false
}

/// Test: No direct std::process::exit calls outside emit_and_exit
#[test]
fn test_no_direct_exit_outside_boundary() {
    let source = read_main_rs();
    let occurrences = find_occurrences(&source, "std::process::exit");

    // Build list of violations (exits outside emit_and_exit)
    let mut violations = Vec::new();

    for (line_num, line) in &occurrences {
        if !is_in_emit_and_exit_function(&source, *line_num) {
            violations.push((*line_num, line.clone()));
        }
    }

    if !violations.is_empty() {
        let mut msg = String::from(
            "Found disallowed std::process::exit() calls outside emit_and_exit() function:\n",
        );
        for (line_num, line) in violations {
            msg.push_str(&format!("  Line {}: {}\n", line_num, line.trim()));
        }
        msg.push_str("\nAll process exits must go through the centralized emit_and_exit() boundary function.");
        panic!("{}", msg);
    }
}

/// Test: No eprintln! in command handlers
#[test]
fn test_no_eprintln_in_command_handlers() {
    let source = read_main_rs();
    let occurrences = find_occurrences(&source, "eprintln!");

    // Build list of violations (eprintln in command handlers)
    let mut violations = Vec::new();

    for (line_num, line) in &occurrences {
        if is_in_command_handler(&source, *line_num) {
            violations.push((*line_num, line.clone()));
        }
    }

    if !violations.is_empty() {
        let mut msg =
            String::from("Found disallowed eprintln!() calls in command handler functions:\n");
        for (line_num, line) in violations {
            msg.push_str(&format!("  Line {}: {}\n", line_num, line.trim()));
        }
        msg.push_str("\nCommand handlers must return CliError instead of printing to stderr.");
        panic!("{}", msg);
    }
}

/// Test: Verify emit_and_exit exists and is the only exit point
#[test]
fn test_boundary_function_exists() {
    let source = read_main_rs();

    // Check that emit_and_exit function exists
    assert!(
        source.contains("fn emit_and_exit"),
        "The emit_and_exit boundary function must exist in main.rs"
    );

    // Check it has the right signature (returns !)
    assert!(
        source.contains("fn emit_and_exit(") && source.contains(") -> !"),
        "emit_and_exit must have the correct signature and return type (!)"
    );
}

/// Test: Verify no println! in command handlers (they should use CommandOutput)
#[test]
fn test_no_println_in_command_handlers() {
    let source = read_main_rs();
    let occurrences = find_occurrences(&source, "println!(");

    // Build list of violations (println in command handlers)
    let mut violations = Vec::new();

    for (line_num, line) in &occurrences {
        if is_in_command_handler(&source, *line_num) {
            // Allow println! in the emit_and_exit call (that's the boundary)
            if !line.contains("emit_and_exit") {
                violations.push((*line_num, line.clone()));
            }
        }
    }

    if !violations.is_empty() {
        let mut msg = String::from("Found println!() calls in command handler functions:\n");
        for (line_num, line) in violations {
            msg.push_str(&format!("  Line {}: {}\n", line_num, line.trim()));
        }
        msg.push_str(
            "\nCommand handlers should return CommandOutput instead of directly printing.",
        );
        panic!("{}", msg);
    }
}

/// Test: Verify CliError type has required fields
#[test]
fn test_cli_error_structure() {
    let source = read_main_rs();

    // Check CliError struct has required fields per contract §8.3
    let cli_error_section = source
        .split("struct CliError")
        .nth(1)
        .expect("CliError struct must exist");

    let next_struct = cli_error_section
        .find("struct ")
        .unwrap_or(cli_error_section.len());
    let cli_error_def = &cli_error_section[..next_struct];

    assert!(
        cli_error_def.contains("exit_code"),
        "CliError must have exit_code field"
    );
    assert!(
        cli_error_def.contains("code"),
        "CliError must have code field"
    );
    assert!(
        cli_error_def.contains("message"),
        "CliError must have message field"
    );
    assert!(
        cli_error_def.contains("details"),
        "CliError must have details field"
    );
    assert!(
        cli_error_def.contains("diagnostics"),
        "CliError must have diagnostics field"
    );
}

/// Test: Verify TheorySource enum exists
#[test]
fn test_theory_source_enum_exists() {
    let source = read_main_rs();

    assert!(
        source.contains("enum TheorySource"),
        "TheorySource enum must exist"
    );

    assert!(
        source.contains("File(PathBuf)") || source.contains("File("),
        "TheorySource must have File variant"
    );

    assert!(
        source.contains("Stdin"),
        "TheorySource must have Stdin variant"
    );
}

/// Test: Verify resolve_theory_source function exists
#[test]
fn test_resolve_theory_source_function_exists() {
    let source = read_main_rs();

    assert!(
        source.contains("fn resolve_theory_source"),
        "resolve_theory_source function must exist"
    );

    // Check it returns Result<TheorySource, CliError>
    let func_section = source
        .split("fn resolve_theory_source")
        .nth(1)
        .expect("resolve_theory_source function must exist");

    assert!(
        func_section.contains("Result<TheorySource, CliError>"),
        "resolve_theory_source must return Result<TheorySource, CliError>"
    );
}

/// Test: Verify exit code constants pattern (2, 3, 4 per contract §8.1)
#[test]
fn test_exit_code_patterns() {
    let source = read_main_rs();

    // Check for validation error code (2)
    assert!(
        source.contains("exit_code: 2") || source.contains("exit_code(2)"),
        "Exit code 2 (user/validation error) must be used"
    );

    // Check for execution error code (3)
    assert!(
        source.contains("exit_code: 3") || source.contains("exit_code(3)"),
        "Exit code 3 (execution error) must be used"
    );
}

/// Test: Verify all command handlers return Result<CommandOutput, CliError>
#[test]
fn test_command_handler_signatures() {
    let source = read_main_rs();

    let handlers = [
        "fn run_reason",
        "fn run_validate",
        "fn run_stats",
        "fn run_query",
        "fn run_explain",
        "fn run_why_not",
        "fn run_requires",
        "fn run_capabilities",
    ];

    for handler in &handlers {
        // Find the function signature - it might span multiple lines
        let pattern = format!("{}(", handler);
        let pos = source.find(&pattern);
        assert!(pos.is_some(), "{} must exist", handler);

        // Extract the signature (from fn name to the next { or ;)
        let start = pos.unwrap();
        let end = source[start..]
            .find('{')
            .or_else(|| source[start..].find(';'))
            .unwrap_or(source.len() - start);
        let signature = &source[start..start + end];

        assert!(
            signature.contains("Result<CommandOutput, CliError>"),
            "{} must return Result<CommandOutput, CliError>, found: {}",
            handler,
            signature.lines().next().unwrap_or("unknown")
        );
    }
}

// =============================================================================
// Test file guard tests
// =============================================================================

/// Find all Rust test files in the tests directory
fn find_test_files() -> Vec<PathBuf> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Test: No deprecated Command::cargo_bin usage in test files
#[test]
fn test_no_deprecated_cargo_bin_in_tests() {
    let test_files = find_test_files();
    let mut violations = Vec::new();

    for file_path in test_files {
        let content = fs::read_to_string(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", file_path.display()));

        // Check for deprecated Command::cargo_bin usage (not cargo_bin_cmd!)
        for (i, line) in content.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
            {
                continue;
            }

            // Check for Command::cargo_bin or Command::cargo_bin_env
            // Exclude lines that are part of this very check (contains 'contains("Command::cargo_bin')
            if (line.contains("Command::cargo_bin(") || line.contains("Command::cargo_bin_env("))
                && !line.contains("cargo_bin_cmd!")
                && !line.contains("contains(\"Command::cargo_bin")
            {
                violations.push((
                    file_path.display().to_string(),
                    i + 1,
                    line.trim().to_string(),
                ));
            }
        }
    }

    if !violations.is_empty() {
        let mut msg = String::from(
            "Found deprecated Command::cargo_bin usage in test files. Use cargo_bin_cmd! macro instead:\n",
        );
        for (file, line_num, line) in violations {
            msg.push_str(&format!("  {}:{}: {}\n", file, line_num, line));
        }
        panic!("{}", msg);
    }
}
