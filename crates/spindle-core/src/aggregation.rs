//! Finite-domain aggregate evaluation over completed Rust reasoning snapshots.
//!
//! This typed reference API implements the fragment compared with the Lean
//! aggregate oracle. The `source` bridge lowers parsed SPL through this evaluator.
//! Modal/temporal aggregation is not supported.
//! Integer operations are checked i64 operations; overflow is an explicit error.
//! Strict aggregate rules retain their kind and receive a defeasible closure
//! premise. Every stage replays the complete retained theory, including attackers.
//!
//! The Rust ordinary backend uses constructive defeat-discard; the aggregate
//! Lean proofs currently use a three-phase approximation. Differential tests
//! record that known snapshot discrepancy separately (see lean/AGGREGATION.md).
//! This API is tested against the model, not formally proved to refine it.

pub mod source;

use crate::function_registry::FunctionRegistry;
use std::collections::{BTreeMap, BTreeSet};

use crate::{ConclusionType, Literal, Rule, RuleType, Theory};

/// A relational constant or schema variable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Term {
    /// Exact integer within the Rust i64 range.
    Integer(i64),
    /// Non-numeric constant.
    Symbol(String),
    /// Named variable (independent of SPL spelling).
    Variable(String),
}

/// A nonmodal predicate pattern; negation is strong negation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pattern {
    /// Predicate name.
    pub name: String,
    /// Strong negation.
    pub negation: bool,
    /// Predicate arguments.
    pub args: Vec<Term>,
}

/// An integer expression.
#[derive(Clone, Debug)]
pub enum Expression {
    /// Constant or variable.
    Term(Term),
    /// Named arithmetic operation: +, *, binary -, nonempty min/max.
    Call(String, Vec<Expression>),
    /// Registered pure function; outside the currently verified expression fragment.
    ExternalCall(String, Vec<Expression>),
}

/// A fold over distinct surviving rows.
#[derive(Clone, Debug)]
pub struct Fold {
    /// Variable constrained to the result.
    pub result_var: String,
    /// Explicit initial value; None requires nonempty input.
    pub seed: Option<Expression>,
    /// Reducer: +/sum, min, or max.
    pub reducer: String,
    /// Integer contribution of each matching row.
    pub extract: Expression,
    /// Matching pattern; unmatched outer variables are local to each row.
    pub pattern: Pattern,
    /// Additional variables bound in the outer scope.
    pub grouping_vars: Vec<String>,
}

/// Source body conditions. Logical atoms remain residual rule premises.
#[derive(Clone, Debug)]
pub enum Condition {
    /// Ordinary logical premise.
    Logic(Pattern),
    /// Integer binding constraint.
    Bind(String, Expression),
    /// General value binding for SPL extensions; outside the verified integer-bind fragment.
    BindValue(String, Expression),
    /// Comparison using =, !=, <, <=, >, or >=.
    Compare(String, Expression, Expression),
    /// Aggregate condition.
    Fold(Fold),
}

/// A typed source rule with one or more heads.
#[derive(Clone, Debug)]
pub struct SchemaRule {
    /// Unique source label.
    pub label: String,
    /// Original rule kind, retained during lowering.
    pub kind: RuleType,
    /// Ordered body conditions.
    pub body: Vec<Condition>,
    /// Nonempty head list.
    pub heads: Vec<Pattern>,
}

/// An aggregate program and source-label superiority pairs.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Source rules.
    pub rules: Vec<SchemaRule>,
    /// Superior/inferior source labels, expanded to all ground instances.
    pub priorities: Vec<(String, String)>,
}

/// A decoded user conclusion; internal closure literals are not exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Ground structured atom.
    pub atom: Pattern,
    /// One of the four SDL tags.
    pub tag: ConclusionType,
}

/// Completed result, with the retained theory available for diagnostics.
#[derive(Debug)]
pub struct Evaluation {
    /// Tagged user atoms mentioned by the final lowered theory.
    pub conclusions: Vec<Observation>,
    /// Number of inferred stages.
    pub stage_count: usize,
    /// Ordinary lowered theory (internal atom names are opaque).
    pub theory: Theory,
    table: Vec<Pattern>,
}

type Env = BTreeMap<String, Term>;
type Key = (String, usize);
type Result<T> = std::result::Result<T, String>;

fn vars(p: &Pattern) -> BTreeSet<String> {
    p.args
        .iter()
        .filter_map(|t| match t {
            Term::Variable(v) => Some(v.clone()),
            _ => None,
        })
        .collect()
}
fn expression_vars(e: &Expression) -> BTreeSet<String> {
    match e {
        Expression::Term(Term::Variable(v)) => BTreeSet::from([v.clone()]),
        Expression::Term(_) => BTreeSet::new(),
        Expression::Call(_, args) | Expression::ExternalCall(_, args) => {
            args.iter().flat_map(expression_vars).collect()
        }
    }
}
fn scope(r: &SchemaRule) -> Vec<String> {
    let mut out: BTreeSet<_> = r.heads.iter().flat_map(vars).collect();
    for c in &r.body {
        match c {
            Condition::Logic(p) => out.extend(vars(p)),
            Condition::Bind(v, e) | Condition::BindValue(v, e) => {
                out.insert(v.clone());
                out.extend(expression_vars(e));
            }
            Condition::Compare(_, a, b) => {
                out.extend(expression_vars(a));
                out.extend(expression_vars(b));
            }
            Condition::Fold(f) => {
                out.insert(f.result_var.clone());
                out.extend(f.grouping_vars.clone());
                if let Some(e) = &f.seed {
                    out.extend(expression_vars(e));
                }
                out.extend(
                    expression_vars(&f.extract)
                        .difference(&vars(&f.pattern))
                        .cloned(),
                );
            }
        }
    }
    out.into_iter().collect()
}
fn assignments(names: &[String], domain: &[Term], budget: &mut usize) -> Result<Vec<Env>> {
    let count = names
        .iter()
        .try_fold(1usize, |n, _| n.checked_mul(domain.len()))
        .ok_or("aggregate grounding limit exceeded")?;
    *budget = budget
        .checked_sub(count)
        .ok_or("aggregate grounding limit exceeded")?;
    let mut envs = vec![Env::new()];
    for name in names {
        envs = envs
            .into_iter()
            .flat_map(|env| {
                domain.iter().map(move |value| {
                    let mut next = env.clone();
                    next.insert(name.clone(), value.clone());
                    next
                })
            })
            .collect();
    }
    Ok(envs)
}
fn apply(p: &Pattern, env: &Env) -> Pattern {
    let mut p = p.clone();
    for arg in &mut p.args {
        if let Term::Variable(v) = arg
            && let Some(value) = env.get(v)
        {
            *arg = value.clone();
        }
    }
    p
}
fn key(p: &Pattern) -> Key {
    (p.name.clone(), p.args.len())
}
fn patterns(r: &SchemaRule) -> impl Iterator<Item = &Pattern> {
    r.heads.iter().chain(r.body.iter().filter_map(|c| match c {
        Condition::Logic(p) => Some(p),
        Condition::Fold(f) => Some(&f.pattern),
        _ => None,
    }))
}
fn supported(e: &Expression, registry: Option<&FunctionRegistry>) -> bool {
    match e {
        Expression::Term(_) => true,
        Expression::ExternalCall(name, args) => {
            registry
                .and_then(|r| r.get(crate::intern::intern(name)))
                .is_some_and(|f| f.signature().arity.accepts(args.len()))
                && args.iter().all(|a| supported(a, registry))
        }
        Expression::Call(name, args) => {
            args.iter().all(|a| supported(a, registry))
                && match name.as_str() {
                    "+" | "*" => true,
                    "-" => args.len() == 2,
                    "min" | "max" => !args.is_empty(),
                    _ => false,
                }
        }
    }
}
fn validate(p: &Program, domain: &[Term], registry: Option<&FunctionRegistry>) -> Result<()> {
    if domain.iter().any(|t| matches!(t, Term::Variable(_))) {
        return Err("nonground domain".into());
    }
    let labels: BTreeSet<_> = p.rules.iter().map(|r| &r.label).collect();
    if labels.len() != p.rules.len()
        || p.priorities
            .iter()
            .any(|(a, b)| !labels.contains(a) || !labels.contains(b))
    {
        return Err("invalid source labels or priorities".into());
    }
    for r in &p.rules {
        if r.heads.is_empty()
            || (r.kind == RuleType::Fact && !r.body.is_empty())
            || patterns(r).any(|p| p.name.is_empty() || p.name == "_" || p.name.starts_with('?'))
        {
            return Err("unsupported schema".into());
        }
        for c in &r.body {
            let valid = match c {
                Condition::Logic(_) => true,
                Condition::Bind(_, e) | Condition::BindValue(_, e) => supported(e, registry),
                Condition::Compare(op, a, b) => {
                    ["=", "!=", "<", "<=", ">", ">="].contains(&op.as_str())
                        && supported(a, registry)
                        && supported(b, registry)
                }
                Condition::Fold(f) => {
                    ["+", "sum", "min", "max"].contains(&f.reducer.as_str())
                        && supported(&f.extract, registry)
                        && f.seed.as_ref().is_none_or(|e| supported(e, registry))
                }
            };
            if !valid {
                return Err("unsupported expression or reducer".into());
            }
        }
    }
    Ok(())
}
fn schedule(p: &Program) -> Result<Vec<usize>> {
    let mut keys = BTreeMap::new();
    for pat in p.rules.iter().flat_map(patterns) {
        let next = keys.len() + p.rules.len();
        keys.entry(key(pat)).or_insert(next);
    }
    let mut edges = Vec::new();
    for (i, r) in p.rules.iter().enumerate() {
        for h in &r.heads {
            let j = keys[&key(h)];
            edges.extend([(i, j, 0), (j, i, 0)]);
        }
        for c in &r.body {
            match c {
                Condition::Logic(p) => edges.push((keys[&key(p)], i, 0)),
                Condition::Fold(f) => edges.push((keys[&key(&f.pattern)], i, 1)),
                _ => (),
            }
        }
    }
    let mut stages = vec![0; keys.len() + p.rules.len()];
    for _ in 0..=stages.len() {
        let mut changed = false;
        for &(a, b, w) in &edges {
            if stages[b] < stages[a] + w {
                stages[b] = stages[a] + w;
                changed = true;
            }
        }
        if !changed {
            stages.truncate(p.rules.len());
            return Ok(stages);
        }
    }
    Err("aggregate dependency cycle".into())
}
fn expression(e: &Expression, env: &Env, registry: Option<&FunctionRegistry>) -> Result<i64> {
    match e {
        Expression::Term(t) => match t {
            Term::Integer(n) => Ok(*n),
            Term::Variable(v) => match env.get(v) {
                Some(Term::Integer(n)) => Ok(*n),
                _ => Err("unbound or noninteger expression".into()),
            },
            _ => Err("noninteger expression".into()),
        },
        Expression::ExternalCall(_, _) => match value(e, env, registry)? {
            Term::Integer(n) => Ok(n),
            _ => Err("noninteger aggregate expression".into()),
        },
        Expression::Call(name, args) => {
            let values = args
                .iter()
                .map(|a| expression(a, env, registry))
                .collect::<Result<Vec<_>>>()?;
            let value = match (name.as_str(), values.as_slice()) {
                ("+", _) => values.iter().try_fold(0_i64, |a, b| a.checked_add(*b)),
                ("*", _) => values.iter().try_fold(1_i64, |a, b| a.checked_mul(*b)),
                ("-", [a, b]) => a.checked_sub(*b),
                ("min", _) => values.iter().copied().min(),
                ("max", _) => values.iter().copied().max(),
                _ => None,
            };
            value.ok_or_else(|| "unsupported expression or integer overflow".into())
        }
    }
}
fn value(e: &Expression, env: &Env, registry: Option<&FunctionRegistry>) -> Result<Term> {
    match e {
        Expression::Term(Term::Variable(v)) => env
            .get(v)
            .cloned()
            .ok_or_else(|| "unbound expression".into()),
        Expression::Term(t) => Ok(t.clone()),
        Expression::ExternalCall(name, args) => {
            let f = registry
                .and_then(|r| r.get(crate::intern::intern(name)))
                .ok_or("unknown extension function")?;
            if !f.signature().arity.accepts(args.len()) {
                return Err("extension argument count mismatch".into());
            }
            let args = args
                .iter()
                .map(|a| value(a, env, registry).map(|v| source::runtime_term(&v)))
                .collect::<Result<Vec<_>>>()?;
            let result = f.eval(&args).map_err(|e| e.to_string())?;
            let value = source::term(&result)?;
            if matches!(value, Term::Variable(_)) {
                return Err("extension returned a nonground aggregate value".into());
            }
            Ok(value)
        }
        Expression::Call(_, _) => expression(e, env, registry).map(Term::Integer),
    }
}

fn fold(
    f: &Fold,
    env: &Env,
    rows: &[Pattern],
    domain: &[Term],
    registry: Option<&FunctionRegistry>,
) -> Result<Option<i64>> {
    let mut value = f
        .seed
        .as_ref()
        .map(|e| expression(e, env, registry))
        .transpose()?;
    for row in rows.iter().collect::<BTreeSet<_>>() {
        if key(row) != key(&f.pattern) || row.negation != f.pattern.negation {
            continue;
        }
        let mut local = env.clone();
        let mut matched = true;
        for (pat, actual) in f.pattern.args.iter().zip(&row.args) {
            match pat {
                Term::Variable(v) => match local.get(v) {
                    Some(bound) if bound != actual => {
                        matched = false;
                        break;
                    }
                    Some(_) => (),
                    None => {
                        local.insert(v.clone(), actual.clone());
                    }
                },
                _ if pat != actual => {
                    matched = false;
                    break;
                }
                _ => (),
            }
        }
        if !matched {
            continue;
        }
        let n = expression(&f.extract, &local, registry)?;
        value = Some(match value {
            None => n,
            Some(a) => match f.reducer.as_str() {
                "+" | "sum" => a.checked_add(n).ok_or("integer overflow")?,
                "min" => a.min(n),
                "max" => a.max(n),
                _ => return Err("unsupported reducer".into()),
            },
        });
    }
    if value.is_some_and(|n| !domain.contains(&Term::Integer(n))) {
        return Err("aggregate result outside domain".into());
    }
    Ok(value)
}
fn check(
    c: &Condition,
    env: &Env,
    rows: &[Pattern],
    domain: &[Term],
    registry: Option<&FunctionRegistry>,
) -> Result<bool> {
    Ok(match c {
        Condition::Logic(_) => true,
        Condition::Bind(v, e) => env.get(v) == Some(&Term::Integer(expression(e, env, registry)?)),
        Condition::BindValue(v, e) => env.get(v) == Some(&value(e, env, registry)?),
        Condition::Compare(op, a, b) => {
            let (a, b) = (expression(a, env, registry)?, expression(b, env, registry)?);
            match op.as_str() {
                "=" => a == b,
                "!=" => a != b,
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                ">=" => a >= b,
                _ => return Err("unsupported comparison".into()),
            }
        }
        Condition::Fold(f) => fold(f, env, rows, domain, registry)?
            .is_some_and(|n| env.get(&f.result_var) == Some(&Term::Integer(n))),
    })
}
fn encoded(table: &[Pattern], p: &Pattern) -> Result<Literal> {
    if !vars(p).is_empty() {
        return Err("ungrounded instance".into());
    }
    let mut base = p.clone();
    base.negation = false;
    let i = table
        .iter()
        .position(|a| a == &base)
        .ok_or("atom outside table")?;
    let lit = Literal::simple(format!("a{i}"));
    Ok(if p.negation { lit.complement() } else { lit })
}
fn observations(table: &[Pattern], theory: &Theory) -> Result<Vec<Observation>> {
    let conclusions = crate::reason::reason_prepared(theory).map_err(|e| e.to_string())?;
    // Compare the mentioned-literal universe of Lean's ground Theory, not extra
    // complements synthesized internally by the Rust engine.
    let mentioned: BTreeSet<_> = theory
        .rules()
        .flat_map(|r| {
            r.head.iter().cloned().chain(
                r.body
                    .iter()
                    .filter_map(|b| b.as_logic().map(|p| p.to_literal())),
            )
        })
        .map(|l| (l.name().to_owned(), l.negation))
        .collect();
    let mut result = Vec::new();
    for p in table {
        for negation in [false, true] {
            let mut p = p.clone();
            p.negation = negation;
            let lit = encoded(table, &p)?;
            if !mentioned.contains(&(lit.name().to_owned(), lit.negation)) {
                continue;
            }
            for c in &conclusions {
                if c.literal == lit {
                    result.push(Observation {
                        atom: p.clone(),
                        tag: c.conclusion_type,
                    });
                }
            }
        }
    }
    Ok(result)
}

/// Infer strata, ground over an explicit domain, and evaluate completed prefixes.
///
/// Uses defeasible snapshot evidence for aggregate premises. Errors are returned
/// for unsupported syntax, cycles, integer overflow, or values outside the domain.
/// Domain enumeration is exponential in the number of outer variables; callers
/// should use small finite inputs. Use normal `pipeline::prepare` for SPL input.
/// Registered functions and general value binds require the source bridge; this
/// reference entry point retains the verified integer-expression restrictions.
pub fn evaluate(program: &Program, domain: &[Term]) -> Result<Evaluation> {
    evaluate_registered(program, domain, None, usize::MAX)
}

fn evaluate_registered(
    program: &Program,
    domain: &[Term],
    registry: Option<&FunctionRegistry>,
    limit: usize,
) -> Result<Evaluation> {
    validate(program, domain, registry)?;
    let mut budget = limit;
    let stages = schedule(program)?;
    let stage_count = stages.iter().copied().max().unwrap_or(0) + 1;
    let mut table = BTreeSet::new();
    for p in program.rules.iter().flat_map(patterns) {
        for env in assignments(
            &vars(p).into_iter().collect::<Vec<_>>(),
            domain,
            &mut budget,
        )? {
            let mut atom = apply(p, &env);
            atom.negation = false;
            table.insert(atom);
        }
    }
    let table: Vec<_> = table.into_iter().collect();
    let mut theory = Theory::new();
    let mut labels: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for stage in 0..stage_count {
        let rows: Vec<_> = observations(&table, &theory)?
            .into_iter()
            .filter(|o| o.tag.is_positive())
            .map(|o| o.atom)
            .collect();
        for (i, r) in program
            .rules
            .iter()
            .enumerate()
            .filter(|(i, _)| stages[*i] == stage)
        {
            for (j, env) in assignments(&scope(r), domain, &mut budget)?
                .into_iter()
                .enumerate()
            {
                // Evaluate every condition: a false guard cannot hide a later error.
                let checks = r
                    .body
                    .iter()
                    .map(|c| check(c, &env, &rows, domain, registry))
                    .collect::<Result<Vec<_>>>()?;
                if !checks.into_iter().all(|b| b) {
                    continue;
                }
                let mut body = r
                    .body
                    .iter()
                    .filter_map(|c| match c {
                        Condition::Logic(p) => Some(p),
                        _ => None,
                    })
                    .map(|p| encoded(&table, &apply(p, &env)))
                    .collect::<Result<Vec<_>>>()?;
                if r.body.iter().any(|c| matches!(c, Condition::Fold(_))) {
                    let guard = Literal::simple(format!("g{stage}"));
                    theory.add_rule(Rule::defeasible(
                        format!("g:{stage}"),
                        vec![],
                        guard.clone(),
                    ));
                    body.push(guard);
                }
                for (h, head) in r.heads.iter().enumerate() {
                    let label = format!("u:{i}:{j}:{h}");
                    theory.add_rule(Rule::new(
                        label.clone(),
                        r.kind,
                        body.clone(),
                        vec![encoded(&table, &apply(head, &env))?]
                            .into_iter()
                            .collect::<crate::rule::RuleHead>(),
                    ));
                    labels.entry(r.label.clone()).or_default().push(label);
                }
            }
        }
        for (a, b) in &program.priorities {
            for x in labels.get(a).into_iter().flatten() {
                for y in labels.get(b).into_iter().flatten() {
                    theory.add_superiority(x, y);
                }
            }
        }
    }
    Ok(Evaluation {
        conclusions: observations(&table, &theory)?,
        stage_count,
        theory,
        table,
    })
}
