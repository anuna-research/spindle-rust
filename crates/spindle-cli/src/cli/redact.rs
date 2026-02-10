//! Redaction rules for CLI error output (SPEC-010 §8.3)
//!
//! When `--debug-errors` is NOT set, error messages are redacted to avoid
//! leaking:
//! - Absolute file paths (replaced with just the filename)
//! - OS-level error details like "(os error 13)" or errno descriptions
//!
//! When `--debug-errors` IS set, messages pass through unmodified.

use std::path::Path;

/// Redact an absolute path to just its filename component.
///
/// "/Users/alice/theories/my_theory.dfl" -> "my_theory.dfl"
/// "stdin" -> "stdin" (unchanged)
/// Relative paths are left unchanged.
pub(crate) fn redact_path(path: &str) -> String {
    if path.starts_with('/') || path.starts_with('\\') {
        Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string())
    } else {
        path.to_string()
    }
}

/// Redact OS error details from a message string.
///
/// Strips patterns like " (os error 13)" suffixes.
pub(crate) fn redact_os_errors(message: &str) -> String {
    // Match " (os error N)" at end of string or before other text
    if let Some(start) = message.find("(os error ") {
        // Walk back to consume leading whitespace
        let trim_start = message[..start].trim_end().len();
        if let Some(end) = message[start..].find(')') {
            let mut result = String::with_capacity(message.len());
            result.push_str(&message[..trim_start]);
            result.push_str(&message[start + end + 1..]);
            return result;
        }
    }
    message.to_string()
}

/// Apply all redaction rules to an error detail message.
///
/// This is the main entry point -- call with `debug=false` to redact,
/// or `debug=true` to pass through unchanged.
pub(crate) fn redact_detail(message: &str, debug: bool) -> String {
    if debug {
        return message.to_string();
    }
    redact_os_errors(message)
}

/// Apply path redaction to a source_name, unless in debug mode.
pub(crate) fn redact_source_name(name: &str, debug: bool) -> String {
    if debug {
        return name.to_string();
    }
    redact_path(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_absolute_path() {
        assert_eq!(redact_path("/Users/alice/theory.dfl"), "theory.dfl");
        assert_eq!(redact_path("/tmp/test.spl"), "test.spl");
    }

    #[test]
    fn test_redact_relative_path_unchanged() {
        assert_eq!(redact_path("theory.dfl"), "theory.dfl");
        assert_eq!(redact_path("stdin"), "stdin");
    }

    #[test]
    fn test_redact_os_error() {
        assert_eq!(
            redact_os_errors("No such file or directory (os error 2)"),
            "No such file or directory"
        );
        assert_eq!(
            redact_os_errors("Permission denied (os error 13)"),
            "Permission denied"
        );
    }

    #[test]
    fn test_redact_os_error_no_match() {
        assert_eq!(
            redact_os_errors("normal error message"),
            "normal error message"
        );
    }

    #[test]
    fn test_redact_detail_debug_passthrough() {
        let msg = "Error reading '/tmp/foo.dfl': No such file (os error 2)";
        assert_eq!(redact_detail(msg, true), msg);
    }

    #[test]
    fn test_redact_detail_normal_mode() {
        assert_eq!(
            redact_detail("Error: No such file or directory (os error 2)", false),
            "Error: No such file or directory"
        );
    }

    #[test]
    fn test_redact_source_name_debug() {
        assert_eq!(
            redact_source_name("/Users/alice/theory.dfl", true),
            "/Users/alice/theory.dfl"
        );
    }

    #[test]
    fn test_redact_source_name_normal() {
        assert_eq!(
            redact_source_name("/Users/alice/theory.dfl", false),
            "theory.dfl"
        );
        assert_eq!(redact_source_name("stdin", false), "stdin");
    }
}
