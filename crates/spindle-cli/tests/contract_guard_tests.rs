//! Contract guard tests.
//!
//! These are intentionally minimal and low-brittleness. Behavioral contract
//! enforcement belongs in the matrix/schema tests; this file only keeps a
//! high-value structural guard that is hard to regress accidentally.

use std::fs;
use std::path::PathBuf;

/// Find all Rust test files recursively in the tests directory.
fn find_test_files() -> Vec<PathBuf> {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.push("tests");

    let mut files = Vec::new();
    let mut stack = vec![root];

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }
    }

    files
}

/// Guard: no deprecated Command::cargo_bin usage in test files.
#[test]
fn test_no_deprecated_cargo_bin_in_tests() {
    let test_files = find_test_files();
    let mut violations = Vec::new();

    for file_path in test_files {
        let content = fs::read_to_string(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", file_path.display()));

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
            {
                continue;
            }

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
            msg.push_str(&format!("  {file}:{line_num}: {line}\n"));
        }
        panic!("{msg}");
    }
}
