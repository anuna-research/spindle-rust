//! Portable, deterministic lookup functions and named aggregators.
use crate::term::TermDto;
use serde::{Deserialize, Serialize};
use spindle_core::function_registry::AggregatorDefinition;
use spindle_core::{
    Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature, Term, intern,
};
use std::collections::{HashMap, HashSet};

/// The type of a lookup argument or result. Matching preserves term types.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Symbol,
    Integer,
    Decimal,
    Float,
}
impl ValueType {
    fn accepts(self, term: &Term) -> bool {
        matches!(
            (self, term),
            (Self::Symbol, Term::Symbol(_))
                | (Self::Integer, Term::Integer(_))
                | (Self::Decimal, Term::Decimal(_))
                | (Self::Float, Term::Float(_))
        )
    }
}

/// A typed lookup row; missing keys produce an evaluation error.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LookupRow {
    pub args: Vec<TermDto>,
    pub result: TermDto,
}
/// A pure finite function with an explicit signature.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LookupFunction {
    pub name: String,
    pub arguments: Vec<ValueType>,
    pub returns: ValueType,
    pub rows: Vec<LookupRow>,
}
/// A named integer aggregator. Custom reducers must be associative and commutative.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedAggregator {
    pub name: String,
    pub reducer: String,
    pub identity: Option<i64>,
    #[serde(default)]
    pub count: bool,
}
/// Portable registry document accepted by both interfaces.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
    pub schema_version: String,
    #[serde(default)]
    pub functions: Vec<LookupFunction>,
    #[serde(default)]
    pub aggregators: Vec<NamedAggregator>,
}
struct Lookup {
    signature: FunctionSignature,
    arguments: Vec<ValueType>,
    rows: HashMap<Vec<Term>, Term>,
}
impl ExtensionFunction for Lookup {
    fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        if args.len() != self.arguments.len()
            || !args
                .iter()
                .zip(&self.arguments)
                .all(|(arg, ty)| ty.accepts(arg))
        {
            return Err(EvalError::TypeError(
                "lookup argument types do not match the declared signature".into(),
            ));
        }
        self.rows
            .get(args)
            .cloned()
            .ok_or_else(|| EvalError::EvalFailed("no matching lookup row".into()))
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('?')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || "-_+*/<>=!:.".contains(c))
}
impl Extensions {
    /// Validate and build a fresh registry. Builtins cannot be replaced.
    pub fn into_registry(self) -> Result<FunctionRegistry, String> {
        if self.schema_version != "spindle.extensions.v1" {
            return Err("expected spindle.extensions.v1".into());
        }
        let prelude = FunctionRegistry::with_prelude();
        let mut registry = FunctionRegistry::new();
        for function in self.functions {
            if !valid_name(&function.name)
                || prelude.contains(intern(&function.name))
                || registry.contains(intern(&function.name))
            {
                return Err(format!(
                    "invalid, duplicate or reserved function name: {}",
                    function.name
                ));
            }
            let mut rows = HashMap::new();
            for row in function.rows {
                let args = row
                    .args
                    .iter()
                    .map(|a| a.to_term().ok_or_else(|| "invalid lookup term".to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = row.result.to_term().ok_or("invalid result term")?;
                if args.len() != function.arguments.len()
                    || !args
                        .iter()
                        .zip(&function.arguments)
                        .all(|(a, t)| t.accepts(a))
                    || !function.returns.accepts(&result)
                {
                    return Err(format!(
                        "lookup row does not match signature: {}",
                        function.name
                    ));
                }
                if rows.insert(args, result).is_some() {
                    return Err(format!("duplicate lookup key: {}", function.name));
                }
            }
            registry.register(Box::new(Lookup {
                signature: FunctionSignature {
                    name: intern(&function.name),
                    arity: Arity::Fixed(function.arguments.len()),
                    description: "portable lookup function",
                },
                arguments: function.arguments,
                rows,
            }));
        }
        let mut names = HashSet::new();
        for agg in self.aggregators {
            if !valid_name(&agg.name)
                || prelude.get_aggregator(&agg.name).is_some()
                || !names.insert(agg.name.clone())
            {
                return Err(format!(
                    "invalid, duplicate or reserved aggregator name: {}",
                    agg.name
                ));
            }
            let reducer = if agg.reducer == "sum" {
                "+"
            } else {
                &agg.reducer
            };
            let function = registry
                .get(intern(reducer))
                .or_else(|| prelude.get(intern(reducer)))
                .ok_or_else(|| format!("unknown reducer: {}", agg.reducer))?;
            if !function.signature().arity.accepts(2) {
                return Err("aggregator reducer must accept two arguments".into());
            }
            registry.register_aggregator(
                &agg.name,
                AggregatorDefinition {
                    reducer: reducer.into(),
                    identity: agg.identity,
                    count: agg.count,
                },
            );
        }
        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup_is_typed_and_missing_keys_fail() {
        let document: Extensions =
            serde_json::from_str(include_str!("../../../examples/lookup-functions.json")).unwrap();
        let registry = document.into_registry().unwrap();
        let function = registry.get(intern("classification")).unwrap();
        assert_eq!(
            function.eval(&[Term::Integer(2)]).unwrap(),
            Term::Symbol(intern("small"))
        );
        assert!(function.eval(&[Term::Integer(3)]).is_err());
        assert!(function.eval(&[Term::Symbol(intern("2"))]).is_err());
        assert!(registry.get_aggregator("total").is_some());
    }
    #[test]
    fn invalid_registrations_fail_before_use() {
        let base: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/lookup-functions.json")).unwrap();
        for case in 0..5 {
            let mut document = base.clone();
            match case {
                0 => document["schema_version"] = "unknown".into(),
                1 => document["functions"][0]["name"] = "+".into(),
                2 => document["functions"][0]["returns"] = "integer".into(),
                3 => {
                    document["functions"][0]["rows"][1] =
                        document["functions"][0]["rows"][0].clone()
                }
                _ => document["aggregators"][0]["reducer"] = "missing".into(),
            }
            assert!(
                serde_json::from_value::<Extensions>(document)
                    .unwrap()
                    .into_registry()
                    .is_err()
            );
        }
    }
}
