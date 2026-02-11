//! S-expression lexer for the SPL format.
//!
//! This module handles the low-level tokenization of SPL input:
//! - Comment stripping (`;` line comments, `#lang` directives)
//! - Quoted string parsing with escape sequences
//! - Parenthesis-delimited list parsing
//! - Atom (identifier/number/variable) recognition
//! - Byte-offset tracking for error reporting

use nom::{
    IResult, Parser,
    bytes::complete::take_while1,
    character::complete::multispace0,
    error::{Error, ErrorKind},
};

/// S-expression representation
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SExpr {
    Atom { value: String, offset: usize },
    List { items: Vec<SExpr>, offset: usize },
}

impl SExpr {
    pub(crate) fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom { value, .. } => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List { items, .. } => Some(items),
            _ => None,
        }
    }

    pub(crate) fn offset(&self) -> usize {
        match self {
            SExpr::Atom { offset, .. } | SExpr::List { offset, .. } => *offset,
        }
    }
}

/// Calculate line number from byte offset in the input string.
pub(crate) fn line_of_offset(input: &str, offset: usize) -> usize {
    let clamped = offset.min(input.len());
    input[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

/// Extract the text of a specific line (1-indexed) from the input string.
///
/// Returns `None` if the line number is out of range or empty.
pub(crate) fn source_line_text(input: &str, line: usize) -> Option<String> {
    if line == 0 {
        return None;
    }
    let text = input.lines().nth(line - 1)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Calculate line number from a nom parse error on cleaned input.
///
/// Extracts the remaining (unparsed) input length from the nom error and
/// computes the byte offset into the cleaned string.
pub(crate) fn line_of_from_error(cleaned: &str, err: &nom::Err<Error<&str>>) -> usize {
    let remaining_len = match err {
        nom::Err::Error(e) | nom::Err::Failure(e) => e.input.len(),
        nom::Err::Incomplete(_) => 0,
    };
    let offset = cleaned.len().saturating_sub(remaining_len);
    line_of_offset(cleaned, offset)
}

/// Remove semicolon comments and `#lang` directives from input, respecting
/// quoted strings so that `;` inside strings is preserved.
pub(crate) fn remove_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Skip #lang directives (Racket-specific)
            if trimmed.starts_with("#lang") {
                return "".to_string();
            }

            let mut result = String::new();
            let mut in_string = false;
            let mut escaped = false;

            for c in line.chars() {
                if in_string {
                    result.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        in_string = false;
                    }
                } else {
                    if c == ';' {
                        break; // Comment starts
                    }
                    result.push(c);
                    if c == '"' {
                        in_string = true;
                    }
                }
            }
            result
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse multiple top-level S-expressions, tracking byte offsets for each.
pub(crate) fn parse_expressions_with_positions(input: &str) -> IResult<&str, Vec<(SExpr, usize)>> {
    let mut results = Vec::new();
    let mut remaining = input;

    loop {
        // Skip whitespace
        let (after_ws, _) = multispace0::<&str, Error<&str>>(remaining)?;
        if after_ws.is_empty() {
            break;
        }

        // Record offset before parsing the expression
        let offset = input.len() - after_ws.len();

        match parse_sexpr(input, after_ws) {
            Ok((rest, expr)) => {
                results.push((expr, offset));
                remaining = rest;
            }
            Err(_) => break,
        }
    }

    let final_remaining = remaining;
    // Skip trailing whitespace
    let (final_remaining, _) = multispace0::<&str, Error<&str>>(final_remaining)?;
    Ok((final_remaining, results))
}

/// Parse a single S-expression. `full_input` is the entire cleaned string
/// (used to compute byte offsets); `input` is the current parse position.
fn parse_sexpr<'a>(full_input: &'a str, input: &'a str) -> IResult<&'a str, SExpr> {
    parse_sexpr_inner(full_input, input)
}

/// Dispatch to list, string, or atom parsing.
fn parse_sexpr_inner<'a>(full_input: &'a str, input: &'a str) -> IResult<&'a str, SExpr> {
    if input.starts_with('(') {
        parse_list(full_input, input)
    } else if input.starts_with('"') {
        parse_string(full_input, input)
    } else {
        parse_atom(full_input, input)
    }
}

/// Parse a parenthesised list: `( ... )`
fn parse_list<'a>(full_input: &'a str, input: &'a str) -> IResult<&'a str, SExpr> {
    let offset = full_input.len() - input.len();
    let mut remaining = &input[1..]; // skip '('
    let mut items = Vec::new();

    loop {
        let (after_ws, _) = multispace0::<&str, Error<&str>>(remaining)?;
        remaining = after_ws;

        if let Some(rest) = remaining.strip_prefix(')') {
            return Ok((rest, SExpr::List { items, offset }));
        }

        let (rest, expr) = parse_sexpr_inner(full_input, remaining)?;
        items.push(expr);
        remaining = rest;
    }
}

/// Parse a quoted string: `"..."` with backslash escape sequences.
fn parse_string<'a>(full_input: &'a str, input: &'a str) -> IResult<&'a str, SExpr> {
    let Some(input) = input.strip_prefix('"') else {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Char)));
    };
    let offset = full_input.len() - (input.len() + 1);
    let mut escaped = false;
    let mut out = String::new();
    let mut end_idx = None;

    for (idx, c) in input.char_indices() {
        if escaped {
            out.push(c);
            escaped = false;
            continue;
        }

        if c == '\\' {
            escaped = true;
            continue;
        }

        if c == '"' {
            end_idx = Some(idx);
            break;
        }

        out.push(c);
    }

    let end_idx = match end_idx {
        Some(idx) => idx,
        None => {
            return Err(nom::Err::Error(Error::new(input, ErrorKind::Char)));
        }
    };

    let remaining = &input[end_idx + 1..];
    Ok((remaining, SExpr::Atom { value: out, offset }))
}

/// Parse an atom: an identifier, number, or variable token.
fn parse_atom<'a>(full_input: &'a str, input: &'a str) -> IResult<&'a str, SExpr> {
    let offset = full_input.len() - input.len();
    let (input, s) = take_while1(|c: char| {
        c.is_alphanumeric()
            || c == '-'
            || c == '_'
            || c == '?'
            || c == '~'
            || c == ':'
            || c == '.'
            || c == '+'
    })
    .parse(input)?;
    Ok((
        input,
        SExpr::Atom {
            value: s.to_string(),
            offset,
        },
    ))
}
