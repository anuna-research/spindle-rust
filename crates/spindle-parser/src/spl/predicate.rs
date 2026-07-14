//! First-class predicate declarations and structured metadata targets
//! (SPEC-024 CON-008, REQ-016, REQ-017).
//!
//! These forms are recognized structurally over the existing S-expression AST.
//! No predicate indicator is ever rendered and reparsed (SPEC-024 CON-008).

use spindle_core::Theory;
use spindle_core::intern::intern;
use spindle_core::vocabulary::{
    ArgumentDecl, DeclarationOrigin, MetaTarget, PredicateDeclaration, PredicateSignature,
    PredicateSymbol, PrimitiveSort, SourceLocation,
};

use crate::ParseError;
use crate::error::ParserFormat;
use crate::predicate_indicator::parse_arity;

use super::lexer::SExpr;

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError::ParserError {
        line,
        message: message.into(),
        format: ParserFormat::Spl,
        source_line: None,
    }
}

/// Maximum number of argument binders accepted in one predicate declaration
/// (SPEC-024 NFR-004).
///
/// `parse_spl` handles untrusted input, and a declaration's binder list is both
/// allocated in full and duplicate-scanned by [`PredicateSignature::try_new`];
/// rejecting oversized declarations up front bounds that memory and CPU the same
/// way [`crate::predicate_indicator`] bounds indicator input.
pub const DECLARATION_ARGUMENT_LIMIT: usize = 255;

/// Process a predicate declaration:
/// `(predicate functor ((arg sort) ...) property*)` (SPEC-024 REQ-016).
///
/// Arity is derived structurally from the argument list. The form adds no fact
/// or rule; it stores a [`PredicateDeclaration`] with source provenance.
///
/// Optional trailing `(key value)` properties are inline metadata: they are
/// exact sugar for a separate `(meta (predicate functor arity) ...)` statement
/// and desugar into the same `MetaTarget::Predicate` store (SPEC-024 ADR-008).
pub(crate) fn process_predicate_declaration(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
    byte_offset: usize,
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(err(
            line,
            "predicate declaration requires a functor and an argument list: (predicate name ((arg sort) ...))",
        ));
    }

    let functor = args[0]
        .as_atom()
        .ok_or_else(|| err(line, "predicate declaration functor must be an atom"))?;

    let arg_list = args[1]
        .as_list()
        .ok_or_else(|| err(line, "predicate declaration argument list must be a list"))?;

    // Reject oversized binder lists before materializing them (SPEC-024 NFR-004).
    if arg_list.len() > DECLARATION_ARGUMENT_LIMIT {
        return Err(err(
            line,
            format!(
                "predicate declaration has {} arguments, exceeding the {DECLARATION_ARGUMENT_LIMIT}-argument limit",
                arg_list.len()
            ),
        ));
    }

    let mut declarations = Vec::with_capacity(arg_list.len());
    for entry in arg_list {
        let pair = entry
            .as_list()
            .ok_or_else(|| err(line, "each predicate argument must be an (name sort) list"))?;
        if pair.len() != 2 {
            return Err(err(
                line,
                "each predicate argument must be exactly (name sort)",
            ));
        }
        let name = pair[0]
            .as_atom()
            .ok_or_else(|| err(line, "predicate argument name must be an atom"))?;
        let sort_name = pair[1]
            .as_atom()
            .ok_or_else(|| err(line, "predicate argument sort must be an atom"))?;
        let sort = PrimitiveSort::from_name(sort_name).ok_or_else(|| {
            err(
                line,
                format!(
                    "unknown primitive sort {sort_name:?}; expected symbol, integer, decimal, float, number, or any"
                ),
            )
        })?;
        declarations.push(ArgumentDecl::new(name, sort));
    }

    let symbol = PredicateSymbol::try_new(intern(functor).into(), arg_list.len())
        .map_err(|e| err(line, e.to_string()))?;

    let signature =
        PredicateSignature::try_new(symbol, declarations).map_err(|e| err(line, e.to_string()))?;

    let origin = DeclarationOrigin::Parsed(SourceLocation::new(byte_offset, line));
    theory.add_predicate_declaration(PredicateDeclaration::new(signature, origin));

    // Optional trailing properties are inline metadata sugar for
    // `(meta (predicate functor arity) ...)` (SPEC-024 ADR-008).
    if args.len() > 2 {
        super::metadata::apply_meta_properties(
            theory,
            &MetaTarget::Predicate(symbol),
            &args[2..],
            line,
        )?;
    }
    Ok(())
}

/// Resolve a `meta` target from its first argument (SPEC-024 REQ-017).
///
/// Either a bare atom label or a structured `(predicate functor arity)` target.
/// The structured target is kept distinct from rule-label metadata.
pub(crate) fn parse_meta_target(arg: &SExpr, line: usize) -> Result<MetaTarget, ParseError> {
    match arg {
        SExpr::Atom { value, .. } => Ok(MetaTarget::Label(value.clone())),
        SExpr::List { items, .. } => {
            if items.len() != 3 || items[0].as_atom() != Some("predicate") {
                return Err(err(
                    line,
                    "structured meta target must be (predicate functor arity)",
                ));
            }
            let functor = items[1]
                .as_atom()
                .ok_or_else(|| err(line, "meta predicate target functor must be an atom"))?;
            let arity_atom = items[2]
                .as_atom()
                .ok_or_else(|| err(line, "meta predicate target arity must be an atom"))?;
            let arity = parse_arity(arity_atom).ok_or_else(|| {
                err(
                    line,
                    format!("meta predicate target has a malformed arity: {arity_atom:?}"),
                )
            })?;
            let symbol = PredicateSymbol::try_new(intern(functor).into(), arity as usize)
                .map_err(|e| err(line, e.to_string()))?;
            Ok(MetaTarget::Predicate(symbol))
        }
    }
}
