//! Bridge from parsed SPL theories to predicate aggregate evaluation.
use super::*;
use crate::{
    arith::{ArithConstraint, ArithExpr},
    body::{BodyArg, BodyLiteral},
    intern::{intern, resolve},
};

/// Convert a source term into the supported integer/symbol fragment.
pub fn term(t: &crate::term::Term) -> Result<Term> {
    match t {
        crate::term::Term::Integer(n) => Ok(Term::Integer(*n)),
        crate::term::Term::Symbol(id) => {
            let s = resolve(*id);
            if s == "_" {
                return Err(
                    "wildcards are not supported in aggregate programs; use named variables".into(),
                );
            }
            Ok(if s.starts_with('?') {
                Term::Variable(s.into())
            } else {
                Term::Symbol(s.into())
            })
        }
        _ => Err("aggregate programs currently support integers and symbols only".into()),
    }
}

pub(super) fn runtime_term(t: &Term) -> crate::term::Term {
    match t {
        Term::Integer(n) => crate::term::Term::Integer(*n),
        Term::Symbol(s) | Term::Variable(s) => crate::term::Term::Symbol(intern(s)),
    }
}
fn literal(p: &Pattern) -> Literal {
    Literal::from_ids(
        intern(&p.name),
        p.negation,
        Default::default(),
        Default::default(),
        p.args.iter().map(runtime_term).collect(),
    )
}
fn pattern(lit: &Literal) -> Result<Pattern> {
    if lit.is_modal()
        || lit.is_temporal()
        || lit.temporal_expr.is_some()
        || lit.interval_var.is_some()
    {
        return Err("modal and temporal aggregate programs are not supported".into());
    }
    Ok(Pattern {
        name: lit.name().into(),
        negation: lit.negation,
        args: lit
            .predicate_args()
            .iter()
            .map(term)
            .collect::<Result<_>>()?,
    })
}
fn expression(e: &ArithExpr) -> Result<Expression> {
    Ok(match e {
        ArithExpr::Lit(crate::term::NumericValue::Integer(n)) => Expression::Term(Term::Integer(*n)),
        ArithExpr::Value(t) => Expression::Term(term(t)?),
        ArithExpr::Var(v) => Expression::Term(Term::Variable(resolve(*v).into())),
        ArithExpr::Call { name, args } => Expression::ExternalCall(resolve(*name).into(), args.iter().map(expression).collect::<Result<_>>()?),
        ArithExpr::Fold(_) => return Err("fold must be the direct expression of bind; use a separate binding for further calculations".into()),
        _ => return Err("aggregate expressions currently support integers and symbols only".into()),
    })
}

/// Detect folds anywhere in source body expressions, including invalid placements.
pub fn has_folds(theory: &Theory) -> bool {
    theory.rules().any(|r| {
        r.body.iter().any(|b| match b {
            BodyLiteral::Arithmetic(ArithConstraint::Bind { expr, .. }) => expr.contains_fold(),
            BodyLiteral::Arithmetic(ArithConstraint::Compare { lhs, rhs, .. }) => {
                lhs.contains_fold() || rhs.contains_fold()
            }
            BodyLiteral::Logic(l) => l
                .predicate_args()
                .iter()
                .any(|a| matches!(a, BodyArg::Arith(e) if e.contains_fold())),
        })
    })
}

fn condition(body: &BodyLiteral, registry: &FunctionRegistry) -> Result<Condition> {
    Ok(match body {
        BodyLiteral::Logic(lit) => {
            if lit.has_arith_args() {
                return Err(
                    "in aggregate programs, use bind for expressions in predicate arguments".into(),
                );
            }
            Condition::Logic(pattern(&lit.to_literal())?)
        }
        BodyLiteral::Arithmetic(ArithConstraint::Bind {
            var,
            expr: ArithExpr::Fold(fold),
        }) => {
            let (seed, reducer, extract) = if let Some(name) = &fold.aggregator {
                let agg = registry
                    .get_aggregator(name)
                    .ok_or_else(|| format!("unknown aggregator '{name}'"))?;
                (
                    agg.identity.map(|n| Expression::Term(Term::Integer(n))),
                    agg.reducer.clone(),
                    if agg.count {
                        Expression::Term(Term::Integer(1))
                    } else {
                        expression(&fold.extract)?
                    },
                )
            } else {
                (
                    fold.initial.as_ref().map(expression).transpose()?,
                    fold.reducer.clone(),
                    expression(&fold.extract)?,
                )
            };
            Condition::Fold(Fold {
                result_var: resolve(*var).into(),
                seed,
                reducer,
                extract,
                pattern: pattern(&fold.pattern)?,
                grouping_vars: vec![],
            })
        }
        BodyLiteral::Arithmetic(ArithConstraint::Bind { var, expr }) => {
            Condition::BindValue(resolve(*var).into(), expression(expr)?)
        }
        BodyLiteral::Arithmetic(ArithConstraint::Compare { op, lhs, rhs }) => {
            Condition::Compare(op.to_string(), expression(lhs)?, expression(rhs)?)
        }
    })
}

fn schema_rule(rule: &Rule, registry: &FunctionRegistry) -> Result<SchemaRule> {
    if !rule.mode.is_empty()
        || !rule.temporal.is_empty()
        || !rule.constraints.is_empty()
        || !rule.state_queries.is_empty()
    {
        return Err("modal and temporal aggregate programs are not supported".into());
    }
    Ok(SchemaRule {
        label: rule.label.clone(),
        kind: rule.rule_type,
        body: rule
            .body
            .iter()
            .map(|b| condition(b, registry))
            .collect::<Result<_>>()?,
        heads: rule.head.iter().map(pattern).collect::<Result<_>>()?,
    })
}

/// Translate parsed rules without grounding candidate heads or discarding attackers.
pub fn program(theory: &Theory) -> Result<Program> {
    program_registered(theory, &FunctionRegistry::with_prelude())
}

fn program_registered(theory: &Theory, registry: &FunctionRegistry) -> Result<Program> {
    let mut rules: Vec<_> = theory.rules().collect();
    rules.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(Program {
        rules: rules
            .into_iter()
            .map(|r| schema_rule(r, registry))
            .collect::<Result<_>>()?,
        priorities: theory
            .superiorities()
            .iter()
            .map(|s| (s.superior.clone(), s.inferior.clone()))
            .collect(),
    })
}

/// Lower aggregates through completed snapshots, returning ordinary structured rules.
pub(crate) fn prepare(
    theory: &Theory,
    registry: &FunctionRegistry,
    limit: usize,
) -> Result<Theory> {
    let trust = theory.trust_policy();
    if !trust.trust_map.is_empty()
        || !trust.thresholds.is_empty()
        || !trust.decay_map.is_empty()
        || trust.default_trust != 0.0
        || theory
            .metadata()
            .values()
            .any(|m| m.properties.contains_key("source"))
    {
        return Err("trust-weighted aggregate snapshots are not supported".into());
    }
    let program = program_registered(theory, registry)?;
    let result = binding::evaluate(&program, Some(registry), limit)?;
    let mut output = Theory::new();
    output.copy_declarative_state_from(theory);
    let names: BTreeSet<_> = program
        .rules
        .iter()
        .flat_map(patterns)
        .map(|p| p.name.as_str())
        .collect();
    let mut guards = BTreeMap::new();
    for r in result.theory.rules().filter(|r| r.label.starts_with("g:")) {
        let mut fresh = format!("__aggregate_snapshot_{}", guards.len());
        while names.contains(fresh.as_str()) || guards.values().any(|v| v == &fresh) {
            fresh.push('_');
        }
        guards.insert(r.head[0].name().to_owned(), fresh);
    }
    output.aggregate_guards.extend(guards.values().cloned());
    let decode = |lit: &Literal| -> Result<Literal> {
        if let Some(name) = guards.get(lit.name()) {
            return Ok(if lit.negation {
                Literal::negated(name)
            } else {
                Literal::simple(name)
            });
        }
        let i: usize = lit
            .name()
            .strip_prefix('a')
            .ok_or("invalid aggregate atom")?
            .parse()
            .map_err(|_| "invalid aggregate atom")?;
        let mut p = result.table.get(i).ok_or("invalid aggregate atom")?.clone();
        p.negation = lit.negation;
        Ok(literal(&p))
    };
    for r in result.theory.rules() {
        let mut decoded = r.clone();
        decoded.head = r
            .head
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>>>()?
            .into();
        decoded.body = r
            .body
            .iter()
            .map(|b| {
                decode(&b.as_logic().ok_or("invalid lowered body")?.to_literal())
                    .map(BodyLiteral::from)
            })
            .collect::<Result<Vec<_>>>()?
            .into();
        if let Some(index) = r
            .label
            .strip_prefix("u:")
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.parse::<usize>().ok())
        {
            decoded.template_label = Some(program.rules[index].label.clone());
        }
        output.add_rule(decoded);
    }
    // The ordinary reasoner resolves superiority through template labels.
    // Decoding restores source template labels, so restore their priorities too.
    for s in theory.superiorities() {
        output.add_superiority(&s.superior, &s.inferior);
    }
    for s in result.theory.superiorities() {
        output.add_superiority(&s.superior, &s.inferior);
    }
    Ok(output)
}
