//! Predicate-indicator recognizer (SPEC-024 CON-002, REQ-003).
//!
//! This is an explicit, fully-consuming recognizer for the Prolog-style
//! `functor/arity` notation used for display and CLI interoperability. It is
//! **not** a suffix regular expression: a slash-bearing functor must be quoted,
//! so `"rate/limit"/2` is unambiguous while `rate/limit/2` is rejected
//! (SPEC-024 ADR-002, LangSec).

use spindle_core::intern::intern;
use spindle_core::vocabulary::PredicateSymbol;

/// Default maximum indicator length, used when a caller does not supply one
/// (SPEC-024 NFR-004).
pub const DEFAULT_INDICATOR_LIMIT: usize = 4096;

/// Errors from predicate-indicator recognition (SPEC-024 §9).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PredicateIndicatorError {
    /// The input exceeded the configured size limit.
    #[error("predicate indicator exceeds the {limit}-byte input limit")]
    TooLong {
        /// The configured limit.
        limit: usize,
    },
    /// The functor was empty.
    #[error("predicate indicator has an empty functor")]
    EmptyFunctor,
    /// A bare functor contained a character outside the bare grammar.
    #[error("invalid character {ch:?} in bare functor; quote the functor to include it")]
    InvalidFunctorChar {
        /// The offending character.
        ch: char,
    },
    /// A quoted functor was not terminated.
    #[error("unterminated quoted functor")]
    UnterminatedQuote,
    /// A quoted functor contained an invalid escape.
    #[error("invalid escape {ch:?} in quoted functor")]
    InvalidEscape {
        /// The character following the backslash.
        ch: char,
    },
    /// A control character appeared in a quoted functor.
    #[error("control character in quoted functor")]
    ControlCharacter,
    /// The `/` separator was missing.
    #[error("predicate indicator is missing the '/' arity separator")]
    MissingSeparator,
    /// The arity was empty or malformed.
    #[error("predicate indicator has a malformed arity")]
    MalformedArity,
    /// The arity did not fit in a `u32`.
    #[error("predicate indicator arity exceeds u32::MAX")]
    ArityOverflow,
    /// Input remained after a complete parse.
    #[error("unexpected trailing input after predicate indicator")]
    TrailingInput,
}

/// Characters permitted in a bare (unquoted) indicator functor (SPEC-024
/// CON-002): a Unicode alphabetic scalar, an ASCII digit, or one of the listed
/// punctuation marks. `is_alphanumeric` is deliberately **not** used, because it
/// also admits Unicode numeric scalars (e.g. `²`) that the grammar's
/// `digit = "0".."9"` excludes.
fn is_bare_char(c: char) -> bool {
    c.is_alphabetic()
        || c.is_ascii_digit()
        || matches!(
            c,
            '-' | '_' | '?' | '~' | ':' | '.' | '+' | '*' | '<' | '>' | '=' | '!'
        )
}

/// Parse a predicate indicator with the default size limit.
pub fn parse_predicate_indicator(input: &str) -> Result<PredicateSymbol, PredicateIndicatorError> {
    parse_predicate_indicator_with_limit(input, DEFAULT_INDICATOR_LIMIT)
}

/// Parse a predicate indicator, bounded by the caller's `limit` (SPEC-024 NFR-004).
///
/// The recognizer either consumes the complete input or returns an error, uses
/// checked arity conversion, and performs no backtracking.
pub fn parse_predicate_indicator_with_limit(
    input: &str,
    limit: usize,
) -> Result<PredicateSymbol, PredicateIndicatorError> {
    if input.len() > limit {
        return Err(PredicateIndicatorError::TooLong { limit });
    }

    let (functor, rest) = if let Some(after_quote) = input.strip_prefix('"') {
        parse_quoted_functor(after_quote)?
    } else {
        parse_bare_functor(input)?
    };

    if functor.is_empty() {
        return Err(PredicateIndicatorError::EmptyFunctor);
    }

    let arity_str = rest
        .strip_prefix('/')
        .ok_or(PredicateIndicatorError::MissingSeparator)?;

    let arity = parse_arity_field(arity_str)?;

    PredicateSymbol::try_new(intern(&functor).into(), arity as usize)
        .map_err(|_| PredicateIndicatorError::ArityOverflow)
}

/// Parse a bare functor, returning `(functor, remaining)` where `remaining`
/// begins at the first `/` (or is empty if none).
fn parse_bare_functor(input: &str) -> Result<(String, &str), PredicateIndicatorError> {
    let mut functor = String::new();
    for (idx, c) in input.char_indices() {
        if c == '/' {
            return Ok((functor, &input[idx..]));
        }
        if !is_bare_char(c) {
            return Err(PredicateIndicatorError::InvalidFunctorChar { ch: c });
        }
        functor.push(c);
    }
    // No separator at all.
    Ok((functor, ""))
}

/// Parse a quoted functor body (the input starts just after the opening `"`),
/// returning `(functor, remaining)` where `remaining` begins after the closing
/// quote.
fn parse_quoted_functor(input: &str) -> Result<(String, &str), PredicateIndicatorError> {
    let mut functor = String::new();
    let mut chars = input.char_indices();
    while let Some((idx, c)) = chars.next() {
        match c {
            '"' => {
                // Byte after the closing quote.
                let end = idx + c.len_utf8();
                return Ok((functor, &input[end..]));
            }
            '\\' => {
                let (_, next) = chars
                    .next()
                    .ok_or(PredicateIndicatorError::UnterminatedQuote)?;
                match next {
                    '"' | '\\' | '/' => functor.push(next),
                    other => return Err(PredicateIndicatorError::InvalidEscape { ch: other }),
                }
            }
            c if c.is_control() => return Err(PredicateIndicatorError::ControlCharacter),
            c => functor.push(c),
        }
    }
    Err(PredicateIndicatorError::UnterminatedQuote)
}

/// Parse an `arity` per CON-002: `"0" | non-zero-digit digit*`. Rejects leading
/// zeroes, signs, whitespace, and non-digits. Returns `None` on any violation.
pub(crate) fn parse_arity(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    if s == "0" {
        return Some(0);
    }
    let mut chars = s.chars();
    let first = chars.next()?;
    if !('1'..='9').contains(&first) {
        return None;
    }
    if !chars.all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u32>().ok()
}

/// Parse the arity field of an indicator, distinguishing the three failure
/// categories the spec names separately (SPEC-024 CON-002, §9): trailing input,
/// a grammar violation (empty or a leading zero), and genuine `u32` overflow.
fn parse_arity_field(s: &str) -> Result<u32, PredicateIndicatorError> {
    let digits_len = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let (digits, trailing) = s.split_at(digits_len);

    if digits.is_empty() {
        return Err(PredicateIndicatorError::MalformedArity);
    }
    if !trailing.is_empty() {
        // Any non-digit remainder after the arity token is trailing input.
        return Err(PredicateIndicatorError::TrailingInput);
    }
    if digits.len() > 1 && digits.starts_with('0') {
        // A leading zero violates `arity = "0" | non-zero-digit digit*`; it is a
        // grammar error, not a magnitude overflow.
        return Err(PredicateIndicatorError::MalformedArity);
    }
    digits
        .parse::<u32>()
        .map_err(|_| PredicateIndicatorError::ArityOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::vocabulary::HasPredicateSymbol;

    fn indicator(s: &str) -> String {
        parse_predicate_indicator(s)
            .unwrap()
            .indicator()
            .to_string()
    }

    #[test]
    fn roundtrips_simple() {
        assert_eq!(indicator("assign-to/2"), "assign-to/2");
        assert_eq!(indicator("p/0"), "p/0");
    }

    #[test]
    fn quoted_slash_functor_roundtrips() {
        assert_eq!(indicator("\"rate/limit\"/2"), "\"rate/limit\"/2");
    }

    #[test]
    fn rejects_bare_slash_functor() {
        assert!(parse_predicate_indicator("rate/limit/2").is_err());
    }

    #[test]
    fn rejects_leading_zero_and_sign_and_ws() {
        assert_eq!(parse_arity("01"), None);
        assert_eq!(parse_arity("+1"), None);
        assert_eq!(parse_arity("-1"), None);
        assert_eq!(parse_arity(" 1"), None);
        // Leading-zero arities are a grammar violation, not an overflow.
        assert_eq!(
            parse_predicate_indicator("p/01"),
            Err(PredicateIndicatorError::MalformedArity)
        );
        assert_eq!(
            parse_predicate_indicator("p/00"),
            Err(PredicateIndicatorError::MalformedArity)
        );
    }

    #[test]
    fn trailing_input_after_arity_rejected() {
        assert_eq!(
            parse_predicate_indicator("p/2x"),
            Err(PredicateIndicatorError::TrailingInput)
        );
        assert_eq!(
            parse_predicate_indicator("p/2 "),
            Err(PredicateIndicatorError::TrailingInput)
        );
        assert_eq!(
            parse_predicate_indicator("p/2/3"),
            Err(PredicateIndicatorError::TrailingInput)
        );
    }

    #[test]
    fn unicode_numeric_scalar_not_a_bare_functor() {
        // U+00B2 SUPERSCRIPT TWO is numeric but not an ASCII digit or alphabetic,
        // so it is excluded from the bare grammar (SPEC-024 CON-002).
        assert_eq!(
            parse_predicate_indicator("\u{00B2}/1"),
            Err(PredicateIndicatorError::InvalidFunctorChar { ch: '\u{00B2}' })
        );
    }

    #[test]
    fn rejects_empty_and_trailing() {
        assert_eq!(
            parse_predicate_indicator("/2"),
            Err(PredicateIndicatorError::EmptyFunctor)
        );
        assert_eq!(
            parse_predicate_indicator("p/"),
            Err(PredicateIndicatorError::MalformedArity)
        );
        assert_eq!(
            parse_predicate_indicator("p"),
            Err(PredicateIndicatorError::MissingSeparator)
        );
    }

    #[test]
    fn arity_overflow_rejected() {
        assert_eq!(
            parse_predicate_indicator("p/99999999999999999999"),
            Err(PredicateIndicatorError::ArityOverflow)
        );
    }

    #[test]
    fn invalid_escape_rejected() {
        assert_eq!(
            parse_predicate_indicator("\"a\\x\"/1"),
            Err(PredicateIndicatorError::InvalidEscape { ch: 'x' })
        );
    }

    #[test]
    fn size_limit_enforced() {
        let long = format!("{}/1", "a".repeat(100));
        assert_eq!(
            parse_predicate_indicator_with_limit(&long, 10),
            Err(PredicateIndicatorError::TooLong { limit: 10 })
        );
    }

    #[test]
    fn escaped_quote_roundtrips_without_changing_content() {
        // Functor containing a literal double-quote.
        let sym = parse_predicate_indicator("\"a\\\"b\"/1").unwrap();
        assert_eq!(sym.functor().resolve(), "a\"b");
        assert_eq!(sym.arity(), 1);
        // Confirm the symbol matches a one-arg literal built with the same functor.
        let lit = spindle_core::Literal::new(
            "a\"b",
            false,
            Default::default(),
            Default::default(),
            vec!["x".to_string()],
        );
        assert_eq!(sym, lit.predicate_symbol().unwrap());
    }
}
