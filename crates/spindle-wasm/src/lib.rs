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

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::query::{self, QueryStatus};
use spindle_core::reason::reason;
use spindle_core::scalable::reason_scalable;
use spindle_core::theory::{MetaValue, Theory};
use spindle_parser::{parse_dfl, parse_spl};

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

/// A conclusion from reasoning
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
}

/// Why-not result
#[derive(Serialize, Deserialize)]
pub struct JsWhyNotResult {
    /// The literal
    pub literal: String,
    /// Rule that would derive it
    pub would_derive: Option<String>,
    /// Blocking conditions
    pub blockers: Vec<String>,
}

/// Abduction result
#[derive(Serialize, Deserialize)]
pub struct JsAbductionResult {
    /// The goal
    pub goal: String,
    /// Solutions (each is a list of facts to add)
    pub solutions: Vec<Vec<String>>,
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

    /// Parse a DFL theory string
    #[wasm_bindgen(js_name = parseDfl)]
    pub fn parse_dfl(&mut self, input: &str) -> Result<(), JsError> {
        self.theory = parse_dfl(input).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(())
    }

    /// Parse an SPL theory string
    #[wasm_bindgen(js_name = parseSpl)]
    pub fn parse_spl(&mut self, input: &str) -> Result<(), JsError> {
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

    /// Perform reasoning and return conclusions as JSON
    #[wasm_bindgen]
    pub fn reason(&self) -> JsValue {
        let conclusions = reason(&self.theory);

        let js_conclusions: Vec<JsConclusion> = conclusions
            .iter()
            .map(|c| JsConclusion {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal: c.literal.to_string(),
                positive: c.conclusion_type.is_positive(),
            })
            .collect();

        serde_wasm_bindgen::to_value(&js_conclusions).unwrap_or(JsValue::NULL)
    }

    /// Perform scalable reasoning and return conclusions as JSON
    #[wasm_bindgen(js_name = reasonScalable)]
    pub fn reason_scalable(&self) -> JsValue {
        let result = reason_scalable(&self.theory);
        let indexed = spindle_core::index::IndexedTheory::build(self.theory.clone());
        let conclusions = result.to_conclusions(&indexed);

        let js_conclusions: Vec<JsConclusion> = conclusions
            .iter()
            .map(|c| JsConclusion {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal: c.literal.to_string(),
                positive: c.conclusion_type.is_positive(),
            })
            .collect();

        serde_wasm_bindgen::to_value(&js_conclusions).unwrap_or(JsValue::NULL)
    }

    /// Get only positive conclusions as strings
    #[wasm_bindgen(js_name = getPositiveConclusions)]
    pub fn get_positive_conclusions(&self) -> Vec<String> {
        let conclusions = reason(&self.theory);
        conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
            .collect()
    }

    /// Query a literal
    #[wasm_bindgen]
    pub fn query(&self, literal: &str) -> JsValue {
        let lit = parse_literal(literal);
        let result = query::query(&self.theory, &lit);

        let js_result = JsQueryResult {
            status: match result.status {
                QueryStatus::Provable => "provable".to_string(),
                QueryStatus::Refuted => "refuted".to_string(),
                QueryStatus::Unknown => "unknown".to_string(),
            },
            literal: literal.to_string(),
            conclusion_type: result.conclusion_type.map(|ct| ct.symbol().to_string()),
        };

        serde_wasm_bindgen::to_value(&js_result).unwrap_or(JsValue::NULL)
    }

    /// What-if query: what happens if we assume these facts?
    #[wasm_bindgen(js_name = whatIf)]
    pub fn what_if(&self, hypotheticals: Vec<String>, goal: &str) -> JsValue {
        let hyps: Vec<_> = hypotheticals
            .iter()
            .map(|s| query::HypotheticalClaim::new(parse_literal(s)))
            .collect();

        let goal_lit = parse_literal(goal);
        let result = query::what_if(&self.theory, hyps, &goal_lit);

        let js_result = JsWhatIfResult {
            provable: result.is_provable(),
            new_conclusions: result
                .new_conclusions
                .iter()
                .map(|l| l.to_string())
                .collect(),
        };

        serde_wasm_bindgen::to_value(&js_result).unwrap_or(JsValue::NULL)
    }

    /// Why-not query: why isn't this literal provable?
    #[wasm_bindgen(js_name = whyNot)]
    pub fn why_not(&self, literal: &str) -> JsValue {
        let lit = parse_literal(literal);
        let result = query::why_not(&self.theory, &lit);

        let js_result = JsWhyNotResult {
            literal: literal.to_string(),
            would_derive: result.would_derive,
            blockers: result
                .blocked_by
                .iter()
                .map(|b| b.explanation.clone())
                .collect(),
        };

        serde_wasm_bindgen::to_value(&js_result).unwrap_or(JsValue::NULL)
    }

    /// Abduction: what facts would make this goal provable?
    #[wasm_bindgen]
    pub fn abduce(&self, goal: &str, max_solutions: usize) -> JsValue {
        let goal_lit = parse_literal(goal);
        let result = query::abduce(&self.theory, &goal_lit, max_solutions);

        let js_result = JsAbductionResult {
            goal: goal.to_string(),
            solutions: result
                .solutions
                .iter()
                .map(|sol| sol.facts.iter().map(|l| l.to_string()).collect())
                .collect(),
        };

        serde_wasm_bindgen::to_value(&js_result).unwrap_or(JsValue::NULL)
    }

    /// Get the number of rules in the theory
    #[wasm_bindgen(js_name = ruleCount)]
    pub fn rule_count(&self) -> usize {
        self.theory.rule_count()
    }

    /// Clear the theory
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.theory = Theory::new();
    }

    /// Parse DFL and reason in one call, returning string output (spinguile-compatible)
    ///
    /// Output format:
    /// ```
    /// +D literal
    /// +d literal
    /// -D literal
    /// -d literal
    /// ```
    #[wasm_bindgen(js_name = reasonDfl)]
    pub fn reason_dfl(&mut self, input: &str) -> Result<String, JsError> {
        self.theory = parse_dfl(input).map_err(|e| JsError::new(&e.to_string()))?;
        let conclusions = reason(&self.theory);
        Ok(conclusions
            .iter()
            .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Parse SPL and reason in one call, returning string output (spinguile-compatible)
    ///
    /// Output format:
    /// ```
    /// +D literal
    /// +d literal
    /// META label key "value"
    /// META label key2 ("v1" "v2")
    /// ```
    #[wasm_bindgen(js_name = reasonSpl)]
    pub fn reason_spl(&mut self, input: &str) -> Result<String, JsError> {
        self.theory = parse_spl(input).map_err(|e| JsError::new(&e.to_string()))?;
        let conclusions = reason(&self.theory);

        let mut output = Vec::new();

        // Add conclusions
        for c in &conclusions {
            output.push(format!("{} {}", c.conclusion_type.symbol(), c.literal));
        }

        // Add metadata
        for (label, meta) in self.theory.metadata() {
            for (key, value) in &meta.properties {
                let value_str = match value {
                    MetaValue::String(s) => format!("\"{}\"", s),
                    MetaValue::List(items) => {
                        let quoted: Vec<_> = items.iter().map(|s| format!("\"{}\"", s)).collect();
                        format!("({})", quoted.join(" "))
                    }
                };
                output.push(format!("META {} {} {}", label, key, value_str));
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

/// Parse a literal string, handling negation prefix
fn parse_literal(s: &str) -> Literal {
    if let Some(name) = s.strip_prefix('~') {
        Literal::negated(name)
    } else if let Some(name) = s.strip_prefix('-') {
        Literal::negated(name)
    } else {
        Literal::simple(s)
    }
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

    #[test]
    fn test_parse_dfl() {
        let mut spindle = Spindle::new();
        spindle.parse_dfl("f1: >> bird\nr1: bird => flies").unwrap();
        assert_eq!(spindle.rule_count(), 2);
    }
}
