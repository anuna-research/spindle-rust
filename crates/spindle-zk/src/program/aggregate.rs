//! Finite-domain symbolic lowering following aggregation::evaluate. Candidate
//! enumeration uses public data only; snapshot membership and retained instances
//! are Boolean wires. No private-fact assignment enumeration is performed.

use super::integer::{self, Reducer};
use super::*;
use spindle_core::aggregation::{
    self as agg, Condition, Expression, Pattern, SchemaRule, Term as Value,
};
use spindle_core::intern::intern;
use std::collections::BTreeSet;

type Env = BTreeMap<String, Value>;
const MAX_CANDIDATES: usize = AGGREGATE_LIMITS[1];

fn vars(p: &Pattern) -> BTreeSet<String> {
    p.args
        .iter()
        .filter_map(|v| match v {
            Value::Variable(v) => Some(v.clone()),
            _ => None,
        })
        .collect()
}

fn expr_vars(e: &Expression) -> BTreeSet<String> {
    match e {
        Expression::Term(Value::Variable(v)) => BTreeSet::from([v.clone()]),
        Expression::Term(_) => BTreeSet::new(),
        Expression::Call(_, args) | Expression::ExternalCall(_, args) => {
            args.iter().flat_map(expr_vars).collect()
        }
    }
}

fn scope(r: &SchemaRule) -> Vec<String> {
    let mut names: BTreeSet<_> = r.heads.iter().flat_map(vars).collect();
    for c in &r.body {
        match c {
            Condition::Logic(p) => names.extend(vars(p)),
            Condition::Bind(v, e) | Condition::BindValue(v, e) => {
                names.insert(v.clone());
                names.extend(expr_vars(e));
            }
            Condition::Compare(_, a, b) => {
                names.extend(expr_vars(a));
                names.extend(expr_vars(b));
            }
            Condition::Fold(f) => {
                names.insert(f.result_var.clone());
                names.extend(f.grouping_vars.clone());
                if let Some(e) = &f.seed {
                    names.extend(expr_vars(e));
                }
                names.extend(expr_vars(&f.extract).difference(&vars(&f.pattern)).cloned());
            }
        }
    }
    names.into_iter().collect()
}

fn assignments(names: &[String], domain: &[Value], budget: &mut usize) -> Result<Vec<Env>> {
    if names.len() > AGGREGATE_LIMITS[2] {
        return Err(Error::ResourceLimit(
            "at most 16 aggregate variables per scope".into(),
        ));
    }
    let count = names
        .iter()
        .try_fold(1usize, |n, _| n.checked_mul(domain.len()))
        .ok_or_else(|| Error::ResourceLimit("aggregate grounding".into()))?;
    *budget = budget
        .checked_sub(count)
        .ok_or_else(|| Error::ResourceLimit("aggregate grounding".into()))?;
    let mut envs = vec![Env::new()];
    for name in names {
        envs = envs
            .into_iter()
            .flat_map(|env| {
                domain.iter().map(move |v| {
                    let mut next = env.clone();
                    next.insert(name.clone(), v.clone());
                    next
                })
            })
            .collect();
    }
    Ok(envs)
}

fn apply(p: &Pattern, env: &Env) -> Pattern {
    let mut p = p.clone();
    for term in &mut p.args {
        if let Value::Variable(v) = term {
            *term = env[v].clone();
        }
    }
    p
}

fn literal(p: &Pattern) -> Literal {
    Literal::from_ids(
        intern(&p.name),
        p.negation,
        Default::default(),
        Default::default(),
        p.args
            .iter()
            .map(|v| match v {
                Value::Integer(n) => Term::Integer(*n),
                Value::Symbol(s) => Term::Symbol(intern(s)),
                Value::Variable(_) => unreachable!("ground candidate"),
            })
            .collect(),
    )
}

fn patterns(r: &SchemaRule) -> impl Iterator<Item = &Pattern> {
    r.heads.iter().chain(r.body.iter().filter_map(|c| match c {
        Condition::Logic(p) => Some(p),
        Condition::Fold(f) => Some(&f.pattern),
        _ => None,
    }))
}

fn supported(e: &Expression) -> bool {
    match e {
        Expression::Term(_) => true,
        Expression::Call(name, args) | Expression::ExternalCall(name, args) => {
            args.iter().all(supported)
                && match name.as_str() {
                    "+" | "*" => true,
                    "-" => args.len() == 2,
                    "min" | "max" => !args.is_empty(),
                    _ => false,
                }
        }
    }
}

// Every variable is a public candidate assignment here, not a private value.
// Errors are represented separately and gated by row selection for extraction.
fn expression(e: &Expression, env: &Env) -> Option<i64> {
    match e {
        Expression::Term(Value::Integer(n)) => Some(*n),
        Expression::Term(Value::Variable(v)) => match env.get(v) {
            Some(Value::Integer(n)) => Some(*n),
            _ => None,
        },
        Expression::Term(_) => None,
        Expression::Call(name, args) | Expression::ExternalCall(name, args) => {
            let values: Vec<_> = args
                .iter()
                .map(|a| expression(a, env))
                .collect::<Option<_>>()?;
            match name.as_str() {
                "+" => values.into_iter().try_fold(0i64, i64::checked_add),
                "*" => values.into_iter().try_fold(1i64, i64::checked_mul),
                "-" if values.len() == 2 => values[0].checked_sub(values[1]),
                "min" => values.into_iter().min(),
                "max" => values.into_iter().max(),
                _ => None,
            }
        }
    }
}

fn value(e: &Expression, env: &Env) -> Option<Value> {
    match e {
        Expression::Term(Value::Variable(v)) => env.get(v).cloned(),
        Expression::Term(v) => Some(v.clone()),
        _ => expression(e, env).map(Value::Integer),
    }
}

fn equal(b: &mut Builder, bits: &integer::Integer, value: i64) -> Wire {
    let expected = integer::constant(value);
    let wires: Vec<_> = bits
        .iter()
        .zip(expected)
        .map(
            |(&bit, expected)| {
                if expected == TRUE { bit } else { b.not(bit) }
            },
        )
        .collect();
    b.all(wires)
}

fn matched(pattern: &Pattern, row: &Pattern, env: &Env) -> Option<Env> {
    if pattern.name != row.name
        || pattern.negation != row.negation
        || pattern.args.len() != row.args.len()
    {
        return None;
    }
    let mut local = env.clone();
    for (p, actual) in pattern.args.iter().zip(&row.args) {
        match p {
            Value::Variable(v) => match local.get(v) {
                Some(bound) if bound != actual => return None,
                Some(_) => (),
                None => {
                    local.insert(v.clone(), actual.clone());
                }
            },
            _ if p != actual => return None,
            _ => (),
        }
    }
    Some(local)
}

pub(super) fn compile(theory: &Theory, inputs: &[Literal]) -> Result<Program> {
    let domain = theory.aggregate_domain().ok_or_else(|| {
        Error::Unsupported(
            "aggregate proofs currently require an explicit aggregate-domain bound".into(),
        )
    })?;
    let domain: Vec<_> = domain
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if domain.len() > AGGREGATE_LIMITS[0] {
        return Err(Error::ResourceLimit("aggregate domain".into()));
    }
    if domain.iter().any(|v| matches!(v, Value::Variable(_))) {
        return Err(Error::InvalidInput(
            "aggregate domain must be ground".into(),
        ));
    }
    let source = agg::source::program(theory).map_err(Error::Unsupported)?;
    for rule in &source.rules {
        if rule.heads.len() > MAX_LITERALS
            || patterns(rule).any(|p| p.args.len() > AGGREGATE_LIMITS[3])
        {
            return Err(Error::ResourceLimit(
                "aggregate head count or predicate arity".into(),
            ));
        }
        if rule.heads.is_empty()
            || (rule.kind == RuleType::Fact && !rule.body.is_empty())
            || patterns(rule).any(|p| p.name.is_empty() || p.name == "_" || p.name.starts_with('?'))
        {
            return Err(Error::InvalidInput("invalid aggregate rule schema".into()));
        }
        for c in &rule.body {
            let supported = match c {
                Condition::Logic(_) => true,
                Condition::Bind(_, e) | Condition::BindValue(_, e) => supported(e),
                Condition::Compare(op, a, b) => {
                    ["=", "!=", "<", "<=", ">", ">="].contains(&op.as_str())
                        && supported(a)
                        && supported(b)
                }
                Condition::Fold(f) => {
                    ["+", "sum", "min", "max"].contains(&f.reducer.as_str())
                        && supported(&f.extract)
                        && f.seed.as_ref().is_none_or(supported)
                }
            };
            if !supported {
                return Err(Error::Unsupported(
                    "aggregate expression or custom function".into(),
                ));
            }
        }
    }
    let stages = agg::infer_stages(&source).map_err(Error::InvalidInput)?;
    let stage_count = stages.iter().copied().max().unwrap_or(0) + 1;
    let mut budget = MAX_CANDIDATES;
    let mut table = BTreeSet::new();
    for p in source.rules.iter().flat_map(patterns) {
        for env in assignments(
            &vars(p).into_iter().collect::<Vec<_>>(),
            &domain,
            &mut budget,
        )? {
            let atom = apply(p, &env);
            table.insert(atom.clone());
            table.insert(Pattern {
                negation: !atom.negation,
                ..atom
            });
        }
    }
    for input in inputs {
        check_literal(input)?;
        if input.predicate_args().len() > AGGREGATE_LIMITS[3] {
            return Err(Error::ResourceLimit(
                "at most 16 aggregate predicate arguments".into(),
            ));
        }
        let p = Pattern {
            name: input.name().into(),
            negation: input.negation,
            args: input
                .predicate_args()
                .iter()
                .map(agg::source::term)
                .collect::<std::result::Result<_, _>>()
                .map_err(Error::Unsupported)?,
        };
        table.insert(p.clone());
        table.insert(Pattern {
            negation: !p.negation,
            ..p
        });
    }
    let table: Vec<_> = table.into_iter().collect();
    let mut atoms: BTreeMap<_, _> = table
        .iter()
        .map(|p| {
            let l = literal(p);
            (literal_spl(&l), l)
        })
        .collect();
    let mut guards = Vec::new();
    for stage in 0..stage_count {
        let mut name = format!("__zk_snapshot_{stage}");
        while atoms.values().any(|l| l.name() == name) {
            name.push('_');
        }
        let guard = Literal::simple(name);
        atoms.insert(literal_spl(&guard), guard.clone());
        atoms.insert(literal_spl(&guard.complement()), guard.complement());
        guards.push(guard);
    }
    if atoms.len() > MAX_LITERALS {
        return Err(Error::ResourceLimit("aggregate literal universe".into()));
    }
    let literals: Vec<_> = atoms.into_values().collect();
    let ids: BTreeMap<_, _> = literals
        .iter()
        .enumerate()
        .map(|(i, l)| (literal_spl(l), i))
        .collect();
    let id = |p: &Pattern| ids[&literal_spl(&literal(p))];
    let complements: Vec<_> = literals
        .iter()
        .map(|l| ids[&literal_spl(&l.complement())])
        .collect();
    let mut inputs = inputs.to_vec();
    inputs.sort_by_key(literal_spl);
    if inputs.windows(2).any(|p| p[0] == p[1]) {
        return Err(Error::InvalidInput(
            "duplicate private input declaration".into(),
        ));
    }
    let mut b = Builder::default();
    b.push(Op::Constant(false));
    b.push(Op::Constant(true));
    let input_wires: Vec<_> = (0..inputs.len()).map(|i| b.push(Op::Input(i))).collect();
    let mut facts = vec![FALSE; literals.len()];
    for (i, l) in inputs.iter().enumerate() {
        facts[ids[&literal_spl(l)]] = input_wires[i];
    }
    let mut rules = Vec::new();
    let mut valid = TRUE;
    for stage in 0..stage_count {
        let snapshot = Program::close(&mut b, &rules, &facts, &complements, |a, d| {
            theory.is_superior(a, d)
        })?;
        let guard = ids[&literal_spl(&guards[stage])];
        rules.push(Rule {
            label: String::new(),
            kind: RuleType::Defeasible,
            head: guard,
            body: vec![],
            enabled: TRUE,
        });
        for (_, rule) in source
            .rules
            .iter()
            .enumerate()
            .filter(|(i, _)| stages[*i] == stage)
        {
            for env in assignments(&scope(rule), &domain, &mut budget)? {
                let mut enabled = TRUE;
                let mut body = Vec::new();
                for condition in &rule.body {
                    let check = match condition {
                        Condition::Logic(p) => {
                            body.push(id(&apply(p, &env)));
                            TRUE
                        }
                        Condition::Fold(f) => {
                            let seed = match &f.seed {
                                Some(e) => match expression(e, &env) {
                                    Some(n) => Some(integer::constant(n)),
                                    None => {
                                        valid = FALSE;
                                        Some(integer::constant(0))
                                    }
                                },
                                None => None,
                            };
                            let mut rows = Vec::new();
                            for row in &table {
                                if let Some(local) = matched(&f.pattern, row, &env) {
                                    let selected = snapshot[id(row)][2];
                                    let value = expression(&f.extract, &local);
                                    if value.is_none() {
                                        let absent = b.not(selected);
                                        valid = b.and(valid, absent);
                                    }
                                    rows.push((selected, integer::constant(value.unwrap_or(0))));
                                }
                            }
                            let reducer = match f.reducer.as_str() {
                                "+" | "sum" => Reducer::Sum,
                                "min" => Reducer::Min,
                                "max" => Reducer::Max,
                                _ => unreachable!(),
                            };
                            let folded = integer::fold(&mut b, reducer, seed, rows);
                            valid = b.and(valid, folded.valid);
                            let matches: Vec<_> = domain
                                .iter()
                                .filter_map(|t| match t {
                                    Value::Integer(n) => Some(equal(&mut b, &folded.value, *n)),
                                    _ => None,
                                })
                                .collect();
                            let in_domain = b.any(matches);
                            let absent = b.not(folded.present);
                            let allowed = b.or(absent, in_domain);
                            valid = b.and(valid, allowed);
                            let matches = match env.get(&f.result_var) {
                                Some(Value::Integer(n)) => equal(&mut b, &folded.value, *n),
                                _ => FALSE,
                            };
                            b.and(folded.present, matches)
                        }
                        Condition::BindValue(v, e) => {
                            let value = value(e, &env);
                            if value.is_none() {
                                valid = FALSE;
                            }
                            usize::from(
                                value
                                    .as_ref()
                                    .is_some_and(|value| env.get(v) == Some(value)),
                            )
                        }
                        Condition::Bind(v, e) => {
                            let value = expression(e, &env);
                            if value.is_none() {
                                valid = FALSE;
                            }
                            usize::from(
                                value.is_some_and(|n| env.get(v) == Some(&Value::Integer(n))),
                            )
                        }
                        Condition::Compare(op, a, c) => {
                            match (expression(a, &env), expression(c, &env)) {
                                (Some(a), Some(c)) => usize::from(match op.as_str() {
                                    "=" => a == c,
                                    "!=" => a != c,
                                    "<" => a < c,
                                    "<=" => a <= c,
                                    ">" => a > c,
                                    ">=" => a >= c,
                                    _ => unreachable!(),
                                }),
                                _ => {
                                    valid = FALSE;
                                    FALSE
                                }
                            }
                        }
                    };
                    enabled = b.and(enabled, check);
                }
                if rule.body.iter().any(|c| matches!(c, Condition::Fold(_))) {
                    body.push(guard);
                }
                for head in &rule.heads {
                    let head = id(&apply(head, &env));
                    if rule.kind == RuleType::Fact {
                        facts[head] = b.or(facts[head], enabled);
                    } else {
                        rules.push(Rule {
                            label: rule.label.clone(),
                            kind: rule.kind,
                            head,
                            body: body.clone(),
                            enabled,
                        });
                    }
                }
                if rules.len() > MAX_CANDIDATES || b.exhausted {
                    return Err(Error::ResourceLimit("aggregate circuit".into()));
                }
            }
        }
    }
    let mut outputs = Program::close(&mut b, &rules, &facts, &complements, |a, d| {
        theory.is_superior(a, d)
    })?;
    for tags in &mut outputs {
        for wire in tags {
            *wire = b.and(*wire, valid);
        }
    }
    // Closure premises are implementation details, not user-queryable claims.
    for guard in guards {
        outputs[ids[&literal_spl(&guard)]] = [FALSE; 4];
        outputs[ids[&literal_spl(&guard.complement())]] = [FALSE; 4];
    }
    if b.exhausted {
        return Err(Error::ResourceLimit("aggregate circuit".into()));
    }
    Ok(Program {
        ops: b.ops,
        outputs,
        literals,
        inputs,
        input_wires,
        valid,
    })
}
