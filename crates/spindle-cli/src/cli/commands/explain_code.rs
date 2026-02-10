//! Explain-code command: prints explanation for a stable error code

use crate::cli::error::CliError;
use crate::cli::output::CommandOutput;

/// Known error codes and their explanations.
struct ErrorCodeInfo {
    code: &'static str,
    meaning: &'static str,
    causes: &'static [&'static str],
    example: &'static str,
}

const ERROR_CODES: &[ErrorCodeInfo] = &[
    ErrorCodeInfo {
        code: "RULE_NOT_FOUND",
        meaning: "A referenced rule label does not exist in the theory.",
        causes: &[
            "Typo in a superiority relation referencing a non-existent rule",
            "Rule was removed but superiority references remain",
        ],
        example: "r1: bird => flies\nsup(r1, r2)  ; r2 is not defined",
    },
    ErrorCodeInfo {
        code: "INVALID_LITERAL",
        meaning: "A literal could not be parsed as a valid identifier.",
        causes: &[
            "Special characters in a literal name",
            "Empty literal string",
        ],
        example: "spindle query 'valid_name' --stdin  ; OK\nspindle query 'bad@name' --stdin   ; INVALID_LITERAL",
    },
    ErrorCodeInfo {
        code: "THEORY_ERROR",
        meaning: "The theory is structurally invalid.",
        causes: &[
            "Contradictory strict rules without resolution",
            "Malformed theory construction",
        ],
        example: "Ensure all rules have valid head and body literals.",
    },
    ErrorCodeInfo {
        code: "REASONING_ERROR",
        meaning: "An error occurred during the reasoning process.",
        causes: &[
            "Internal reasoning engine failure",
            "Unsupported theory configuration",
        ],
        example: "Try simplifying the theory or using --scalable mode.",
    },
    ErrorCodeInfo {
        code: "VALIDATION_ERROR",
        meaning: "Input failed structural or semantic validation.",
        causes: &[
            "Empty theory provided",
            "Invalid command argument combination",
        ],
        example: "spindle validate --stdin < theory.dfl",
    },
    ErrorCodeInfo {
        code: "LEXER_ERROR",
        meaning: "The lexer encountered an unexpected character in DFL input.",
        causes: &[
            "Unrecognized characters in the input",
            "Encoding issues (non-UTF-8 input)",
        ],
        example: "Check the input file for special characters. DFL uses ASCII operators: =>, ->",
    },
    ErrorCodeInfo {
        code: "PARSE_ERROR",
        meaning: "The parser could not understand the input structure.",
        causes: &[
            "Missing or extra tokens in rule definitions",
            "Incorrect rule syntax",
        ],
        example: "r1: bird => flies   ; correct DFL\nr1: bird -> flies   ; wrong arrow for defeasible rule",
    },
    ErrorCodeInfo {
        code: "UNEXPECTED_TOKEN",
        meaning: "The parser found a token where a different one was expected.",
        causes: &["Mismatched delimiters", "Missing operator between terms"],
        example: "Check the syntax around the reported position.",
    },
    ErrorCodeInfo {
        code: "PREPARATION_ERROR",
        meaning: "Theory preparation (grounding, normalization) failed.",
        causes: &[
            "Variable grounding exceeded limits",
            "Invalid temporal references",
        ],
        example: "Try reducing variable usage or checking temporal expressions.",
    },
    ErrorCodeInfo {
        code: "MISSING_INPUT_SOURCE",
        meaning: "No input source was specified.",
        causes: &["Neither a file path nor --stdin was provided"],
        example: "spindle reason theory.dfl      ; from file\nspindle reason --stdin         ; from stdin",
    },
    ErrorCodeInfo {
        code: "CONFLICTING_INPUT_SOURCES",
        meaning: "Both a file path and --stdin were specified.",
        causes: &["Provide exactly one input source, not both"],
        example: "spindle reason theory.dfl      ; file only\nspindle reason --stdin         ; stdin only",
    },
    ErrorCodeInfo {
        code: "FILE_READ_ERROR",
        meaning: "The specified file could not be read.",
        causes: &[
            "File does not exist",
            "Insufficient permissions",
            "Path is a directory",
        ],
        example: "Check that the file path is correct and readable.",
    },
    ErrorCodeInfo {
        code: "CLI_PARSE_ERROR",
        meaning: "The command-line arguments could not be parsed.",
        causes: &["Unknown flag or subcommand", "Missing required argument"],
        example: "spindle --help                 ; show usage\nspindle reason --help          ; show subcommand help",
    },
    ErrorCodeInfo {
        code: "INVALID_TIME_FORMAT",
        meaning: "The --at time value is not valid RFC 3339.",
        causes: &["Time string does not match ISO 8601 / RFC 3339 format"],
        example: "spindle reason --at 2024-06-15T12:00:00Z --stdin",
    },
    ErrorCodeInfo {
        code: "INVALID_ARGUMENT",
        meaning: "A command argument has an invalid value.",
        causes: &["Value out of allowed range or wrong type"],
        example: "spindle requires goal --max 10 --stdin  ; --max must be >= 1",
    },
];

pub(crate) fn run_explain_code(code: &str) -> Result<CommandOutput, CliError> {
    let code_upper = code.to_uppercase();

    if let Some(info) = ERROR_CODES.iter().find(|e| e.code == code_upper) {
        let mut text = String::new();
        text.push_str(&format!("{}\n", info.code));
        text.push_str(&format!("  {}\n\n", info.meaning));
        text.push_str("Common causes:\n");
        for cause in info.causes {
            text.push_str(&format!("  - {cause}\n"));
        }
        text.push_str(&format!("\nExample:\n  {}\n", info.example));
        Ok(CommandOutput::text(text))
    } else {
        Err(CliError::validation(
            "UNKNOWN_ERROR_CODE",
            format!("Unknown error code: {code_upper}"),
        ))
    }
}
