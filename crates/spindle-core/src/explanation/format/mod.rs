//! Output formatters for explanations.
//!
//! Each format (natural language, JSON, JSON-LD, DOT) lives in its own
//! submodule and implements the [`ExplanationFormatter`] trait.  The old
//! convenience methods on [`Explanation`] (`to_natural_language`, `to_json`,
//! `to_jsonld`, `to_dot`) are preserved as thin wrappers for backward
//! compatibility.

pub mod dot;
pub mod json;
pub mod jsonld;
pub mod natural_language;

use super::types::Explanation;

// Re-export formatter structs at the `format::` level for convenience.
pub use dot::DotFormatter;
pub use json::JsonFormatter;
pub use jsonld::JsonLdFormatter;
pub use natural_language::NaturalLanguageFormatter;

/// A stateless, infallible formatter that converts an [`Explanation`] into a
/// string representation.
///
/// # Design constraints
///
/// - **Stateless**: Formatters carry no mutable state. Configuration (e.g.,
///   indentation width, color palette) is provided at construction time and
///   stored in immutable fields.
/// - **Infallible**: `format()` returns `String`, never `Result`. Rendering
///   an in-memory proof tree cannot fail; any errors belong upstream in the
///   reasoning pipeline.
/// - **Borrowed input**: The formatter borrows the `Explanation`, allowing
///   the caller to format the same explanation in multiple formats without
///   cloning.
pub trait ExplanationFormatter {
    /// Render the explanation to a string.
    fn format(&self, explanation: &Explanation) -> String;
}
