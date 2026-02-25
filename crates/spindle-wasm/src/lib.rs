//! WebAssembly bindings for Spindle Defeasible Logic Engine
//!
//! This crate provides JavaScript/TypeScript bindings for the Spindle
//! reasoning engine via WebAssembly.
//!
//! # Example (JavaScript)
//!
//! ```javascript
//! import init, { Spindle } from 'spindle-wasm';
//!
//! await init();
//!
//! const spindle = new Spindle();
//! spindle.addFact("bird");
//! spindle.addFact("penguin");
//! spindle.addDefeasibleRule(["bird"], "flies", "r1");
//! spindle.addDefeasibleRule(["penguin"], "~flies", "r2");
//! spindle.addSuperiority("r2", "r1");
//!
//! const conclusions = spindle.reason();
//! console.log(conclusions);
//! // => ["+D bird", "+D penguin", "+d ~flies", ...]
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use spindle_contract::error::ProblemDetails;
use spindle_contract::literal::{LiteralStructJson, LiteralStructJsonV2};
use spindle_contract::reason::{
    ConclusionEntry, ConclusionEntryV2, GroundingStats, ReasonOutput, ReasonOutputV2, SCHEMA_V1,
    SCHEMA_V2, TheoryStats,
};
use spindle_core::error::SpindleError;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{self, QueryStatus};
use spindle_core::reason::reason;
use spindle_core::temporal::Temporal;
use spindle_core::theory::{MetaValue, Theory};
use spindle_parser::parse_spl;

// Set up better panic messages in debug mode
#[cfg(feature = "console_error_panic_hook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    set_panic_hook();
}

// Re-export contract types with Js-prefixed aliases for backward compatibility
pub type JsReasonOutput = ReasonOutput;
pub type JsGroundingStats = GroundingStats;
pub type JsTheoryStats = TheoryStats;
pub type JsConclusionStruct = ConclusionEntry;

/// A conclusion from reasoning (legacy simple format)
#[derive(Serialize, Deserialize)]
pub struct JsConclusion {
    /// Conclusion type: "+D", "-D", "+d", "-d"
    pub conclusion_type: String,
    /// The literal
    pub literal: String,
    /// Is this a positive conclusion?
    pub positive: bool,
}

/// Query result
#[derive(Serialize, Deserialize)]
pub struct JsQueryResult {
    /// Query status: "provable", "refuted", "unknown"
    pub status: String,
    /// The literal queried
    pub literal: String,
    /// Conclusion type if provable
    pub conclusion_type: Option<String>,
}

/// What-if result
#[derive(Serialize, Deserialize)]
pub struct JsWhatIfResult {
    /// Is the goal provable under hypotheticals?
    pub provable: bool,
    /// New conclusions enabled
    pub new_conclusions: Vec<String>,
    /// Conclusions that changed type (literal, old_type, new_type)
    pub changed_conclusions: Vec<JsChangedConclusion>,
}

#[derive(Serialize, Deserialize)]
pub struct JsChangedConclusion {
    pub literal: String,
    pub old_type: String,
    pub new_type: String,
}

/// Why-not result
#[derive(Serialize, Deserialize)]
pub struct JsWhyNotResult {
    /// The literal
    pub literal: String,
    /// Whether the literal is actually provable
    pub is_provable: bool,
    /// Rule that would derive it
    pub would_derive: Option<String>,
    /// Blocking conditions (structured)
    pub blockers: Vec<JsBlocker>,
}

#[derive(Serialize, Deserialize)]
pub struct JsBlocker {
    pub blocking_type: String,
    pub rule_label: String,
    pub blocking_rule: Option<String>,
    pub explanation: String,
}

/// Abduction result
#[derive(Serialize, Deserialize)]
pub struct JsAbductionResult {
    /// The goal
    pub goal: String,
    /// Solutions (each is a list of facts to add)
    pub solutions: Vec<JsAbductionSolution>,
}

#[derive(Serialize, Deserialize)]
pub struct JsAbductionSolution {
    pub facts: Vec<String>,
    pub rules_used: Vec<String>,
    pub confidence: f64,
}

/// The main Spindle reasoning engine
#[wasm_bindgen]
pub struct Spindle {
    theory: Theory,
}

#[wasm_bindgen]
impl Spindle {
    /// Create a new empty Spindle instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Spindle {
        Spindle {
            theory: Theory::new(),
        }
    }

    /// Parse an SPL theory string
    #[wasm_bindgen(js_name = parseSpl)]
    pub fn parse_spl(&mut self, input: &str) -> Result<(), JsError> {
        reject_dfl_input(input)?;
        self.theory = parse_spl(input).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(())
    }

    /// Add a fact to the theory
    #[wasm_bindgen(js_name = addFact)]
    pub fn add_fact(&mut self, name: &str) -> String {
        self.theory.add_fact(name)
    }

    /// Add a strict rule to the theory
    #[wasm_bindgen(js_name = addStrictRule)]
    pub fn add_strict_rule(&mut self, body: Vec<String>, head: &str) -> String {
        let body_refs: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
        self.theory.add_strict_rule(&body_refs, head)
    }

    /// Add a defeasible rule to the theory
    #[wasm_bindgen(js_name = addDefeasibleRule)]
    pub fn add_defeasible_rule(&mut self, body: Vec<String>, head: &str) -> String {
        let body_refs: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
        self.theory.add_defeasible_rule(&body_refs, head)
    }

    /// Add a defeater to the theory
    #[wasm_bindgen(js_name = addDefeater)]
    pub fn add_defeater(&mut self, body: Vec<String>, head: &str) -> String {
        let body_refs: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
        self.theory.add_defeater(&body_refs, head)
    }

    /// Add a superiority relation
    #[wasm_bindgen(js_name = addSuperiority)]
    pub fn add_superiority(&mut self, superior: &str, inferior: &str) {
        self.theory.add_superiority(superior, inferior);
    }

    /// Perform reasoning and return structured JSON output
    #[wasm_bindgen]
    pub fn reason(&self) -> Result<JsValue, JsError> {
        let prepared = prepare(&self.theory, PrepareOptions::default())
            .map_err(|e| JsError::new(&e.to_string()))?;

        let conclusions = reason(&prepared.theory).map_err(|e| JsError::new(&e.to_string()))?;

        let mut output_conclusions: Vec<ConclusionEntry> = conclusions
            .iter()
            .map(|c| ConclusionEntry {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal_spl: c.literal.to_spl(),
                literal_struct: LiteralStructJson::from(&c.literal),
                positive: c.conclusion_type.is_positive(),
                trust_degree: None,
                trust_sources: None,
            })
            .collect();
        // Sort for deterministic output matching CLI behavior
        output_conclusions.sort_by(|a, b| {
            a.literal_spl
                .cmp(&b.literal_spl)
                .then_with(|| a.conclusion_type.cmp(&b.conclusion_type))
        });

        let output = ReasonOutput {
            schema_version: SCHEMA_V1.to_string(),
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            grounding: GroundingStats {
                performed: prepared.grounding_report.performed,
                had_variables: prepared.grounding_report.had_variables,
                instances: prepared.grounding_report.instances,
                limit_hit: prepared.grounding_report.limit_hit,
            },
            conclusions: output_conclusions,
            diagnostics: vec![],
            stats: Some(TheoryStats {
                rule_count: prepared.theory.rule_count(),
                fact_count: prepared.theory.facts().count(),
            }),
        };

        Ok(serde_wasm_bindgen::to_value(&output)?)
    }

    /// Perform reasoning and return v2 structured JSON output with typed term arguments
    #[wasm_bindgen(js_name = reasonV2)]
    pub fn reason_v2(&self) -> Result<JsValue, JsError> {
        let prepared = prepare(&self.theory, PrepareOptions::default())
            .map_err(|e| JsError::new(&e.to_string()))?;

        let conclusions = reason(&prepared.theory).map_err(|e| JsError::new(&e.to_string()))?;

        let mut output_conclusions: Vec<ConclusionEntryV2> = conclusions
            .iter()
            .map(|c| ConclusionEntryV2 {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal_spl: c.literal.to_spl(),
                literal_struct: LiteralStructJsonV2::from(&c.literal),
                positive: c.conclusion_type.is_positive(),
                trust_degree: None,
                trust_sources: None,
            })
            .collect();
        output_conclusions.sort_by(|a, b| {
            a.literal_spl
                .cmp(&b.literal_spl)
                .then_with(|| a.conclusion_type.cmp(&b.conclusion_type))
        });

        let output = ReasonOutputV2 {
            schema_version: SCHEMA_V2.to_string(),
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            grounding: GroundingStats {
                performed: prepared.grounding_report.performed,
                had_variables: prepared.grounding_report.had_variables,
                instances: prepared.grounding_report.instances,
                limit_hit: prepared.grounding_report.limit_hit,
            },
            conclusions: output_conclusions,
            diagnostics: vec![],
            stats: Some(TheoryStats {
                rule_count: prepared.theory.rule_count(),
                fact_count: prepared.theory.facts().count(),
            }),
        };

        Ok(serde_wasm_bindgen::to_value(&output)?)
    }

    /// Get only positive conclusions as strings
    #[wasm_bindgen(js_name = getPositiveConclusions)]
    pub fn get_positive_conclusions(&self) -> Result<Vec<String>, JsError> {
        let conclusions = reason(&self.theory).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
            .collect())
    }

    /// Query a literal
    #[wasm_bindgen]
    pub fn query(&self, literal: &str) -> Result<JsValue, JsError> {
        let lit = parse_literal(literal);
        let result = query::query(&self.theory, &lit).map_err(|e| JsError::new(&e.to_string()))?;

        let js_result = JsQueryResult {
            status: match result.status {
                QueryStatus::Provable => "provable".to_string(),
                QueryStatus::Refuted => "refuted".to_string(),
                QueryStatus::Unknown => "unknown".to_string(),
            },
            literal: literal.to_string(),
            conclusion_type: result.conclusion_type.map(|ct| ct.symbol().to_string()),
        };

        Ok(serde_wasm_bindgen::to_value(&js_result)?)
    }

    /// What-if query: what happens if we assume these facts?
    #[wasm_bindgen(js_name = whatIf)]
    pub fn what_if(&self, hypotheticals: Vec<String>, goal: &str) -> Result<JsValue, JsError> {
        let hyps: Vec<_> = hypotheticals
            .iter()
            .map(|s| query::HypotheticalClaim::new(parse_literal(s)))
            .collect();

        let goal_lit = parse_literal(goal);
        let result = query::what_if(&self.theory, hyps, &goal_lit)
            .map_err(|e| JsError::new(&e.to_string()))?;

        let js_result = JsWhatIfResult {
            provable: result.is_provable(),
            new_conclusions: result
                .new_conclusions
                .iter()
                .map(|l| l.to_string())
                .collect(),
            changed_conclusions: result
                .changed_conclusions
                .iter()
                .map(|(lit, old, new)| JsChangedConclusion {
                    literal: lit.to_string(),
                    old_type: old.symbol().to_string(),
                    new_type: new.symbol().to_string(),
                })
                .collect(),
        };

        Ok(serde_wasm_bindgen::to_value(&js_result)?)
    }

    /// Why-not query: why isn't this literal provable?
    #[wasm_bindgen(js_name = whyNot)]
    pub fn why_not(&self, literal: &str) -> Result<JsValue, JsError> {
        let lit = parse_literal(literal);
        let result =
            query::why_not(&self.theory, &lit).map_err(|e| JsError::new(&e.to_string()))?;

        let js_result = JsWhyNotResult {
            literal: literal.to_string(),
            is_provable: result.is_provable(),
            would_derive: result.would_derive,
            blockers: result
                .blocked_by
                .iter()
                .map(|b| JsBlocker {
                    blocking_type: b.blocking_type.to_string(),
                    rule_label: b.rule_label.clone(),
                    blocking_rule: b.blocking_rule.clone(),
                    explanation: b.explanation.clone(),
                })
                .collect(),
        };

        Ok(serde_wasm_bindgen::to_value(&js_result)?)
    }

    /// Abduction: what facts would make this goal provable?
    #[wasm_bindgen]
    pub fn abduce(&self, goal: &str, max_solutions: usize) -> Result<JsValue, JsError> {
        let goal_lit = parse_literal(goal);
        let result = query::abduce(&self.theory, &goal_lit, max_solutions)
            .map_err(|e| JsError::new(&e.to_string()))?;

        let js_result = JsAbductionResult {
            goal: goal.to_string(),
            solutions: result
                .solutions
                .iter()
                .map(|sol| JsAbductionSolution {
                    facts: sol.facts.iter().map(|l| l.to_string()).collect(),
                    rules_used: sol.rules_used.iter().cloned().collect(),
                    confidence: sol.confidence,
                })
                .collect(),
        };

        Ok(serde_wasm_bindgen::to_value(&js_result)?)
    }

    /// Get the number of rules in the theory
    #[wasm_bindgen(js_name = ruleCount)]
    pub fn rule_count(&self) -> usize {
        self.theory.rule_count()
    }

    /// Get parsed rules as JSON for client-side processing
    /// Returns array of {label, ruleType, body: string[], head: string[]}
    #[wasm_bindgen(js_name = getRules)]
    pub fn get_rules(&self) -> JsValue {
        #[derive(Serialize)]
        struct JsRule {
            label: String,
            #[serde(rename = "type")]
            rule_type: String,
            body: Vec<String>,
            head: Vec<String>,
        }

        let rules: Vec<JsRule> = self
            .theory
            .rules()
            .map(|r| JsRule {
                label: r.label.clone(),
                rule_type: format!("{:?}", r.rule_type),
                body: r.body.iter().map(|l| l.to_string()).collect(),
                head: r.head.iter().map(|l| l.to_string()).collect(),
            })
            .collect();

        serde_wasm_bindgen::to_value(&rules).unwrap_or(JsValue::NULL)
    }

    /// Clear the theory
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.theory = Theory::new();
    }

    /// Parse SPL and reason in one call, returning string output (spinguile-compatible)
    ///
    /// Output format:
    /// ```text
    /// +D literal
    /// +d literal
    /// META label key "value"
    /// META label key2 ("v1" "v2")
    /// DEPS task1:dep1,dep2|task2:dep3
    /// ```
    #[wasm_bindgen(js_name = reasonSpl)]
    pub fn reason_spl(&mut self, input: &str) -> Result<String, JsError> {
        reject_dfl_input(input)?;
        self.theory = parse_spl(input).map_err(|e| JsError::new(&e.to_string()))?;
        let conclusions = reason(&self.theory).map_err(|e| JsError::new(&e.to_string()))?;

        let mut output = Vec::new();

        // Add conclusions
        for c in &conclusions {
            output.push(format!("{} {}", c.conclusion_type.symbol(), c.literal));
        }

        // Add metadata
        for (label, meta) in self.theory.metadata() {
            for (key, value) in &meta.properties {
                let value_str = match value {
                    MetaValue::String(s) => format!("\"{s}\""),
                    MetaValue::List(items) => {
                        let quoted: Vec<_> = items.iter().map(|s| format!("\"{s}\"")).collect();
                        format!("({})", quoted.join(" "))
                    }
                };
                output.push(format!("META {label} {key} {value_str}"));
            }
        }

        Ok(output.join("\n"))
    }
}

impl Default for Spindle {
    fn default() -> Self {
        Self::new()
    }
}

fn reject_dfl_input(input: &str) -> Result<(), JsError> {
    if has_dfl_shape(input) {
        return Err(JsError::new(
            "UNSUPPORTED_INPUT_FORMAT: DFL format is no longer supported; use SPL.",
        ));
    }
    Ok(())
}

fn has_dfl_shape(content: &str) -> bool {
    content.lines().any(is_dfl_line)
}

fn is_dfl_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }

    if let Some((label, rest)) = trimmed.split_once(':') {
        let label = label.trim();
        if is_dfl_identifier(label)
            && (rest.contains(">>")
                || rest.contains("->")
                || rest.contains("=>")
                || rest.contains("~>"))
        {
            return true;
        }
    }

    if let Some((superior, inferior)) = trimmed.split_once('>') {
        let superior = superior.trim();
        let inferior = inferior.trim();
        if is_dfl_identifier(superior) && is_dfl_identifier(inferior) {
            return true;
        }
    }

    false
}

fn is_dfl_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Parse a literal string, handling negation prefix, predicate arguments, and (not ...) syntax
///
/// Supported formats:
/// - `"bird"` → positive literal
/// - `"~flies"` or `"-flies"` → negated literal
/// - `"(not flies)"` → negated literal (SPL-style)
/// - `"parent(X, Y)"` → literal with predicate arguments
/// - `"~parent(X, Y)"` → negated literal with predicate arguments
fn parse_literal(s: &str) -> Literal {
    let s = s.trim();

    // Handle SPL-style (not ...) syntax
    if s.starts_with("(not ") && s.ends_with(')') {
        let inner = s[5..s.len() - 1].trim();
        return parse_literal_inner(inner, true);
    }

    // Handle ~ or - negation prefix
    if let Some(name) = s.strip_prefix('~') {
        return parse_literal_inner(name, true);
    }
    if let Some(name) = s.strip_prefix('-') {
        return parse_literal_inner(name, true);
    }

    parse_literal_inner(s, false)
}

/// Parse the inner part of a literal, handling predicate arguments like `parent(X, Y)`
fn parse_literal_inner(s: &str, negated: bool) -> Literal {
    if let Some(paren_pos) = s.find('(')
        && s.ends_with(')')
    {
        let name = &s[..paren_pos];
        let args_str = &s[paren_pos + 1..s.len() - 1];
        let args: Vec<String> = args_str
            .split(',')
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .collect();
        return Literal::new(name, negated, Mode::empty(), Temporal::empty(), args);
    }

    if negated {
        Literal::negated(s)
    } else {
        Literal::simple(s)
    }
}

/// Convert a SpindleError to a ProblemDetails JsValue for structured WASM error output.
///
/// Returns a JsValue containing the RFC 9457 Problem Details object.
pub fn error_to_problem_details(err: &SpindleError) -> JsValue {
    let pd = ProblemDetails::from(err);
    serde_wasm_bindgen::to_value(&pd).unwrap_or_else(|_| {
        JsValue::from_str(&format!(
            "{{\"type\":\"tag:spindle.dev,2026:error:{}\",\"title\":\"{}\"}}",
            err.code(),
            err.category().default_title()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spindle_basic() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

        assert_eq!(spindle.rule_count(), 2);
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_parse_spl_rejects_dfl() {
        let mut spindle = Spindle::new();
        let result = spindle
            .parse_spl("f1: >> bird\nr1: bird => flies")
            .map(|_| ());
        assert!(result.is_err(), "DFL must be rejected");
    }

    #[test]
    fn test_parse_spl() {
        let mut spindle = Spindle::new();
        spindle
            .parse_spl("(given bird)\n(normally bird flies)")
            .unwrap();
        assert_eq!(spindle.rule_count(), 2);
    }

    #[test]
    fn test_add_strict_rule() {
        let mut spindle = Spindle::new();
        spindle.add_fact("mammal");
        let label = spindle.add_strict_rule(vec!["mammal".to_string()], "animal");
        assert!(!label.is_empty());
        assert_eq!(spindle.rule_count(), 2);
    }

    #[test]
    fn test_add_defeater() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_fact("penguin");
        spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
        let label = spindle.add_defeater(vec!["penguin".to_string()], "~flies");
        assert!(!label.is_empty());
        assert_eq!(spindle.rule_count(), 4);
    }

    #[test]
    fn test_add_superiority() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_fact("penguin");
        let r1 = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
        let r2 = spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");
        spindle.add_superiority(&r2, &r1);
        assert_eq!(spindle.rule_count(), 4);
    }

    #[test]
    fn test_get_positive_conclusions() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(!conclusions.is_empty());
        assert!(conclusions.iter().any(|c| c.contains("bird")));
    }

    #[test]
    fn test_chain_reasoning() {
        let mut spindle = Spindle::new();
        spindle.add_fact("a");
        spindle.add_defeasible_rule(vec!["a".to_string()], "b");
        spindle.add_defeasible_rule(vec!["b".to_string()], "c");
        spindle.add_defeasible_rule(vec!["c".to_string()], "d");

        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(conclusions.iter().any(|c| c.contains(" a")));
        assert!(conclusions.iter().any(|c| c.contains(" b")));
        assert!(conclusions.iter().any(|c| c.contains(" c")));
        assert!(conclusions.iter().any(|c| c.contains(" d")));
    }

    #[test]
    fn test_strict_rule_chain() {
        let mut spindle = Spindle::new();
        spindle.add_fact("premise");
        spindle.add_strict_rule(vec!["premise".to_string()], "intermediate");
        spindle.add_strict_rule(vec!["intermediate".to_string()], "conclusion");

        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(conclusions.iter().any(|c| c.contains("premise")));
        assert!(conclusions.iter().any(|c| c.contains("intermediate")));
        assert!(conclusions.iter().any(|c| c.contains("conclusion")));
    }

    #[test]
    fn test_defeater_blocks() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_fact("injured");
        spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
        spindle.add_defeater(vec!["injured".to_string()], "~flies");

        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(conclusions.iter().any(|c| c.contains("bird")));
        assert!(conclusions.iter().any(|c| c.contains("injured")));
    }

    #[test]
    fn test_penguin_example_extended() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        spindle.add_fact("penguin");
        let r1 = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
        let r2 = spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");
        spindle.add_superiority(&r2, &r1);

        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(conclusions.iter().any(|c| c.contains("bird")));
        assert!(conclusions.iter().any(|c| c.contains("penguin")));
        assert!(conclusions.iter().any(|c| c.contains("~flies")));
    }

    // Note: parse error tests require WASM target due to JsError
    // See tests/wasm.rs for WASM-specific error handling tests

    #[test]
    fn test_workflow_without_jsvalue() {
        let mut spindle = Spindle::new();

        // Parse theory
        spindle
            .parse_spl(
                r#"
            (given expert)
            (given novice)
            (normally r1 expert reliable)
            (normally r2 novice (not reliable))
            (prefer r1 r2)
            "#,
            )
            .unwrap();

        // Reason using get_positive_conclusions (returns Vec<String>, not JsValue)
        let conclusions = spindle.get_positive_conclusions().unwrap();
        assert!(!conclusions.is_empty());

        // Clear and verify
        spindle.clear();
        assert_eq!(spindle.rule_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut spindle = Spindle::new();
        spindle.add_fact("bird");
        assert_eq!(spindle.rule_count(), 1);
        spindle.clear();
        assert_eq!(spindle.rule_count(), 0);
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_reason_spl_rejects_dfl() {
        let mut spindle = Spindle::new();
        let result = spindle
            .reason_spl("f1: >> bird\nr1: bird => flies")
            .map(|_| ());
        assert!(result.is_err(), "DFL must be rejected");
    }

    #[test]
    fn test_reason_spl() {
        let mut spindle = Spindle::new();
        let result = spindle
            .reason_spl("(given bird)\n(normally bird flies)")
            .unwrap();
        assert!(result.contains("bird"));
        assert!(result.contains("flies"));
    }

    #[test]
    fn test_reason_spl_with_meta() {
        let mut spindle = Spindle::new();
        let result = spindle
            .reason_spl(
                r#"(given bird)
(normally r1 bird flies)
(meta r1 (description "Birds fly"))"#,
            )
            .unwrap();
        assert!(result.contains("bird"));
        assert!(result.contains("META"));
        assert!(result.contains("description"));
    }

    #[test]
    fn test_reason_spl_with_list_meta() {
        let mut spindle = Spindle::new();
        let result = spindle
            .reason_spl(
                r#"(given bird)
(normally r1 bird flies)
(meta r1 (tags ("a" "b")))"#,
            )
            .unwrap();
        assert!(result.contains("bird"));
    }

    #[test]
    fn test_parse_literal_tilde() {
        let lit = parse_literal("~flies");
        assert!(lit.negation);
        assert_eq!(lit.name(), "flies");
    }

    #[test]
    fn test_parse_literal_dash() {
        let lit = parse_literal("-flies");
        assert!(lit.negation);
        assert_eq!(lit.name(), "flies");
    }

    #[test]
    fn test_parse_literal_positive() {
        let lit = parse_literal("bird");
        assert!(!lit.negation);
        assert_eq!(lit.name(), "bird");
    }

    #[test]
    fn test_default() {
        let spindle = Spindle::default();
        assert_eq!(spindle.rule_count(), 0);
    }

    // ==========================================================================
    // EXTENDED COVERAGE TESTS (native-compatible)
    // These tests use functions that don't return JsValue
    // ==========================================================================

    #[test]
    fn test_reason_spl_with_complex_theory() {
        let mut spindle = Spindle::new();
        let result = spindle
            .reason_spl(
                r#"
                (given bird)
                (given penguin)
                (normally r1 bird flies)
                (normally r2 penguin (not flies))
                (prefer r2 r1)
                "#,
            )
            .unwrap();
        assert!(result.contains("bird"));
        assert!(result.contains("penguin"));
    }

    // =========================================================================
    // REGRESSION TESTS - WASM Parity Bug Hunt Fixes
    // =========================================================================

    #[test]
    fn test_parse_literal_not_syntax() {
        let lit = parse_literal("(not flies)");
        assert!(lit.negation);
        assert_eq!(lit.name(), "flies");
    }

    #[test]
    fn test_parse_literal_predicate_args() {
        let lit = parse_literal("parent(X, Y)");
        assert!(!lit.negation);
        assert_eq!(lit.name(), "parent");
        assert!(lit.is_predicate());
        assert_eq!(lit.predicates().len(), 2);
    }

    #[test]
    fn test_parse_literal_negated_predicate_args() {
        let lit = parse_literal("~parent(X, Y)");
        assert!(lit.negation);
        assert_eq!(lit.name(), "parent");
        assert!(lit.is_predicate());
        assert_eq!(lit.predicates().len(), 2);
    }

    #[test]
    fn test_parse_literal_not_syntax_trimmed() {
        let lit = parse_literal("  (not  bird )  ");
        assert!(lit.negation);
        assert_eq!(lit.name(), "bird");
    }

    // Note: Tests that call reason(), query(), what_if(),
    // why_not(), abduce(), or get_rules() require WASM target because they
    // return JsValue. Run these with: wasm-pack test --node
}
