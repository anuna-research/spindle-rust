//! Redaction rules for CLI error output (SPEC-010 §8.3)
//!
//! When `--debug-errors` is NOT set, error messages are redacted to avoid
//! leaking:
//! - Absolute file paths (replaced with just the filename)
//! - OS-level error details like "(os error 13)" or errno descriptions
//!
//! When `--debug-errors` IS set, messages pass through unmodified.

/// Check whether a string starts with an absolute path prefix.
///
/// Recognises Unix (`/`), UNC (`\\server`), and Windows drive-letter
/// (`C:\`, `C:/`) prefixes.
fn is_absolute_path_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() {
        return false;
    }
    // Unix: /...
    if b[0] == b'/' {
        return true;
    }
    // UNC: \\server\share\...
    if b.len() >= 2 && b[0] == b'\\' && b[1] == b'\\' {
        return true;
    }
    // Windows drive-letter: C:\... or C:/...
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    {
        return true;
    }
    false
}

/// Redact an absolute path to just its filename component.
///
/// "/Users/alice/theories/my_theory.spl" -> "my_theory.spl"
/// "C:\\Users\\alice\\theory.spl"        -> "theory.spl"
/// "\\\\server\\share\\file.spl"         -> "file.spl"
/// "stdin"                               -> "stdin" (unchanged)
/// Relative paths are left unchanged.
pub(crate) fn redact_path(path: &str) -> String {
    if !is_absolute_path_prefix(path) {
        return path.to_string();
    }
    // Find the last separator (either `/` or `\`) so Windows paths
    // are handled correctly regardless of the host OS.
    let last_sep = path.rfind('/').into_iter().chain(path.rfind('\\')).max();
    match last_sep {
        Some(pos) if pos + 1 < path.len() => path[pos + 1..].to_string(),
        _ => path.to_string(),
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

/// Redact absolute paths embedded in a message string.
///
/// Handles:
/// - Quoted paths (single/double): `'/Users/alice/f.spl'`, `"C:\Users\f.spl"`
/// - Unquoted paths at word boundaries: `Error reading /tmp/f.spl: ...`
pub(crate) fn redact_paths_in_text(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let bytes = message.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // --- quoted absolute path ---
        if (bytes[i] == b'\'' || bytes[i] == b'"')
            && i + 1 < bytes.len()
            && is_absolute_path_prefix(&message[i + 1..])
        {
            let quote = bytes[i] as char;
            if let Some(end) = message[i + 1..].find(quote) {
                let path = &message[i + 1..i + 1 + end];
                let redacted = redact_path(path);
                result.push(quote);
                result.push_str(&redacted);
                result.push(quote);
                i = i + 1 + end + 1;
                continue;
            }
        }

        // --- unquoted absolute path at a word boundary ---
        if is_absolute_path_prefix(&message[i..])
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            let start = i;
            let mut end = i;
            while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
                end += 1;
            }
            // Trim trailing punctuation that is not part of the path
            while end > start && matches!(bytes[end - 1], b':' | b',' | b';' | b')' | b']') {
                end -= 1;
            }
            if end > start {
                let redacted = redact_path(&message[start..end]);
                result.push_str(&redacted);
                i = end;
                continue;
            }
        }

        // Regular character (handles multi-byte UTF-8)
        let c = message[i..].chars().next().unwrap();
        result.push(c);
        i += c.len_utf8();
    }

    result
}

/// Apply all redaction rules to an error detail message.
///
/// This is the main entry point -- call with `debug=false` to redact,
/// or `debug=true` to pass through unchanged.
pub(crate) fn redact_detail(message: &str, debug: bool) -> String {
    if debug {
        return message.to_string();
    }
    let message = redact_paths_in_text(message);
    redact_os_errors(&message)
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
    fn test_redact_absolute_path_unix() {
        assert_eq!(redact_path("/Users/alice/theory.spl"), "theory.spl");
        assert_eq!(redact_path("/tmp/test.spl"), "test.spl");
    }

    #[test]
    fn test_redact_absolute_path_windows_drive() {
        assert_eq!(redact_path("C:\\Users\\alice\\theory.spl"), "theory.spl");
        assert_eq!(redact_path("D:/data/test.spl"), "test.spl");
    }

    #[test]
    fn test_redact_absolute_path_unc() {
        assert_eq!(redact_path("\\\\server\\share\\theory.spl"), "theory.spl");
    }

    #[test]
    fn test_redact_relative_path_unchanged() {
        assert_eq!(redact_path("theory.spl"), "theory.spl");
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
    fn test_redact_paths_in_text_single_quoted() {
        assert_eq!(
            redact_paths_in_text("Error reading file '/Users/alice/theory.spl': something"),
            "Error reading file 'theory.spl': something"
        );
    }

    #[test]
    fn test_redact_paths_in_text_double_quoted() {
        assert_eq!(
            redact_paths_in_text("Error reading file \"/tmp/data/test.spl\": something"),
            "Error reading file \"test.spl\": something"
        );
    }

    #[test]
    fn test_redact_paths_in_text_no_path() {
        assert_eq!(
            redact_paths_in_text("normal error message"),
            "normal error message"
        );
    }

    #[test]
    fn test_redact_paths_in_text_relative_path_unchanged() {
        assert_eq!(
            redact_paths_in_text("Error reading file 'theory.spl': something"),
            "Error reading file 'theory.spl': something"
        );
    }

    #[test]
    fn test_redact_paths_in_text_quoted_windows_drive() {
        assert_eq!(
            redact_paths_in_text("Error reading 'C:\\Users\\alice\\theory.spl': denied"),
            "Error reading 'theory.spl': denied"
        );
    }

    #[test]
    fn test_redact_paths_in_text_quoted_unc() {
        assert_eq!(
            redact_paths_in_text("Error reading '\\\\server\\share\\file.spl': denied"),
            "Error reading 'file.spl': denied"
        );
    }

    #[test]
    fn test_redact_paths_in_text_unquoted_unix() {
        assert_eq!(
            redact_paths_in_text("Error reading /tmp/foo.spl: No such file"),
            "Error reading foo.spl: No such file"
        );
    }

    #[test]
    fn test_redact_paths_in_text_unquoted_windows() {
        assert_eq!(
            redact_paths_in_text("Error reading C:\\Users\\alice\\file.spl: denied"),
            "Error reading file.spl: denied"
        );
    }

    #[test]
    fn test_redact_paths_in_text_unquoted_preserves_nonpath() {
        // "and/or" should not be treated as a path
        assert_eq!(
            redact_paths_in_text("use stdin and/or a file"),
            "use stdin and/or a file"
        );
    }

    #[test]
    fn test_redact_detail_debug_passthrough() {
        let msg = "Error reading '/tmp/foo.spl': No such file (os error 2)";
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
    fn test_redact_detail_strips_paths_and_os_errors() {
        assert_eq!(
            redact_detail(
                "Error reading file '/Users/alice/theory.spl': No such file or directory (os error 2)",
                false
            ),
            "Error reading file 'theory.spl': No such file or directory"
        );
    }

    #[test]
    fn test_redact_source_name_debug() {
        assert_eq!(
            redact_source_name("/Users/alice/theory.spl", true),
            "/Users/alice/theory.spl"
        );
    }

    #[test]
    fn test_redact_source_name_normal() {
        assert_eq!(
            redact_source_name("/Users/alice/theory.spl", false),
            "theory.spl"
        );
        assert_eq!(redact_source_name("stdin", false), "stdin");
    }

    #[test]
    fn test_redact_source_name_windows() {
        assert_eq!(
            redact_source_name("C:\\Users\\alice\\theory.spl", false),
            "theory.spl"
        );
    }
}
