//! Shared projections of core query results for CLI and WASM consumers.
use crate::literal::LiteralStructJson;
use serde_json::{Value, json};
use spindle_core::{
    Literal,
    query::{RequiresResult, RequiresSearchStatus},
};

/// Serialize verified requirements, including the display-limit sentinel.
pub fn requires_output(
    goal: &Literal,
    result: &RequiresResult,
    max: usize,
    evaluated_at: Option<String>,
) -> Value {
    let mut solutions: Vec<Vec<String>> = result
        .solutions
        .iter()
        .map(|s| {
            let mut facts: Vec<_> = s.facts.iter().map(Literal::to_spl).collect();
            facts.sort();
            facts
        })
        .collect();
    solutions.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    let truncated = solutions.len() > max && !result.already_provable;
    solutions.truncate(max);
    if result.already_provable {
        solutions.clear();
    }
    let mut output = json!({
        "schema_version": "spindle.requires.v2",
        "query": {"goal_spl": goal.to_spl(), "goal_struct": LiteralStructJson::from(goal)},
        "satisfied": result.already_provable,
        "solutions": solutions.into_iter().map(|facts| json!({"facts":facts,"score":1.0})).collect::<Vec<_>>(),
        "verification_mode": "verified",
        "search_status": match result.search_status { RequiresSearchStatus::BoundedComplete => "BoundedComplete", RequiresSearchStatus::BudgetExhausted => "BudgetExhausted" },
        "verification": {"raw_examined": result.verification.raw_examined, "accepted":result.verification.accepted, "rejected":result.verification.rejected},
        "evaluated_at": evaluated_at, "trust": null, "diagnostics": []
    });
    if truncated {
        output["truncated"] = json!({"solutions":true});
        output["diagnostics"] = json!([{"severity":"warning","code":"SOLUTIONS_LIMIT_HIT","message":format!("Results limited to {max} solutions")}]);
    }
    output
}

/// Serialize raw abduction candidates with legacy display strings and typed facts.
pub fn abduction_output(goal: &str, result: &spindle_core::query::AbductionResult) -> Value {
    json!({"goal":goal, "solutions": result.solutions.iter().map(|s| {
        let mut rules: Vec<_> = s.rules_used.iter().cloned().collect(); rules.sort();
        let mut structured: Vec<_> = s.facts.iter().collect();
        structured.sort_by_key(|l| serde_json::to_string(&crate::literal::LiteralStructJsonV2::from(*l)).expect("literal DTO is serializable"));
        let facts: Vec<_> = structured.iter().map(ToString::to_string).collect();
        json!({"facts": facts, "facts_struct":structured.into_iter().map(crate::literal::LiteralStructJsonV2::from).collect::<Vec<_>>(), "rules_used": rules, "confidence":s.confidence})
    }).collect::<Vec<_>>()})
}

/// Serialize the legacy what-if shape used by JavaScript, with typed additions.
pub fn what_if_output(result: &spindle_core::query::WhatIfResult) -> Value {
    use crate::literal::LiteralStructJsonV2;
    json!({
        "provable":result.is_provable(),
        "new_conclusions":result.new_conclusions.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "new_conclusions_struct":result.new_conclusions.iter().map(LiteralStructJsonV2::from).collect::<Vec<_>>(),
        "changed_conclusions":result.changed_conclusions.iter().map(|(lit,old,new)| json!({
            "literal":lit.to_string(), "literal_struct":LiteralStructJsonV2::from(lit),
            "old_type":old.symbol(), "new_type":new.symbol()
        })).collect::<Vec<_>>()
    })
}
