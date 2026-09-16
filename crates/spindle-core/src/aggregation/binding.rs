//! Bind computed values directly, then close the implicit grounding universe.
//!
//! Ordinary premises remain residual. In particular, an undecided predicate is
//! not an empty relation for the purpose of retaining ordinary attackers.
use super::*;

// Only variables introduced by ordinary predicates require grounding choices.
// Bind/aggregate outputs are evaluated, never guessed from the universe.
fn validate_bindings(rule: &SchemaRule) -> Result<()> {
    let mut bound = BTreeSet::new();
    let outer: BTreeSet<_> = scope(rule).into_iter().collect();
    for c in &rule.body {
        let (required, produced) = match c {
            Condition::Logic(p) => {
                let names = vars(p);
                (BTreeSet::new(), names)
            }
            Condition::Bind(v, e) | Condition::BindValue(v, e) => {
                (expression_vars(e), BTreeSet::from([v.clone()]))
            }
            Condition::Compare(_, a, b) => {
                let mut names = expression_vars(a);
                names.extend(expression_vars(b));
                (names, BTreeSet::new())
            }
            Condition::Fold(f) => {
                let mut names: BTreeSet<_> =
                    vars(&f.pattern).intersection(&outer).cloned().collect();
                // The output may not also be a local row variable.
                if vars(&f.pattern).contains(&f.result_var) && !bound.contains(&f.result_var) {
                    return Err(format!("unsafe aggregate output in rule '{}'", rule.label));
                }
                names.extend(f.grouping_vars.clone());
                names.extend(
                    expression_vars(&f.extract)
                        .difference(&vars(&f.pattern))
                        .cloned(),
                );
                if let Some(seed) = &f.seed {
                    names.extend(expression_vars(seed));
                }
                (names, BTreeSet::from([f.result_var.clone()]))
            }
        };
        if let Some(v) = required.difference(&bound).next() {
            return Err(format!(
                "unsafe variable {v} in rule '{}': bind it in an earlier predicate or expression",
                rule.label
            ));
        }
        bound.extend(produced);
    }
    if let Some(v) = rule
        .heads
        .iter()
        .flat_map(vars)
        .find(|v| !bound.contains(v))
    {
        return Err(format!("unsafe head variable {v} in rule '{}'", rule.label));
    }
    Ok(())
}

fn constants(e: &Expression, out: &mut BTreeSet<Term>) {
    match e {
        Expression::Term(Term::Variable(_)) => (),
        Expression::Term(t) => {
            out.insert(t.clone());
        }
        Expression::Call(_, args) | Expression::ExternalCall(_, args) => {
            for a in args {
                constants(a, out);
            }
        }
    }
}

fn bind(env: &mut Env, name: &str, value: Term) -> bool {
    match env.get(name) {
        Some(old) => old == &value,
        None => {
            env.insert(name.into(), value);
            true
        }
    }
}

fn instances(
    rule: &SchemaRule,
    candidates: &BTreeSet<Pattern>,
    rows: &[Pattern],
    registry: Option<&FunctionRegistry>,
    domain: &mut BTreeSet<Term>,
    budget: &mut usize,
) -> Result<Vec<Env>> {
    let mut envs = vec![Env::new()];
    for c in &rule.body {
        let mut next = BTreeSet::new();
        for mut env in envs {
            *budget = budget
                .checked_sub(1)
                .ok_or("aggregate grounding limit exceeded")?;
            match c {
                Condition::Logic(p) => {
                    if vars(p).iter().all(|v| env.contains_key(v)) {
                        next.insert(env);
                    } else {
                        for row in candidates.iter().filter(|row| key(row) == key(p)) {
                            let mut local = env.clone();
                            if p.args.iter().zip(&row.args).all(|(a, b)| match a {
                                Term::Variable(v) => bind(&mut local, v, b.clone()),
                                _ => a == b,
                            }) {
                                *budget = budget
                                    .checked_sub(1)
                                    .ok_or("aggregate grounding limit exceeded")?;
                                next.insert(local);
                            }
                        }
                    }
                }
                Condition::Bind(v, e) | Condition::BindValue(v, e) => {
                    let result = match c {
                        Condition::Bind(_, _) => Term::Integer(expression(e, &env, registry)?),
                        _ => value(e, &env, registry)?,
                    };
                    domain.insert(result.clone());
                    if bind(&mut env, v, result) {
                        next.insert(env);
                    }
                }
                Condition::Compare(..) => {
                    if check(c, &env, rows, &[], registry)? {
                        next.insert(env);
                    }
                }
                Condition::Fold(f) => {
                    if let Some(n) = fold(f, &env, rows, None, registry)? {
                        let result = Term::Integer(n);
                        domain.insert(result.clone());
                        if bind(&mut env, &f.result_var, result) {
                            next.insert(env);
                        }
                    }
                }
            }
        }
        envs = next.into_iter().collect();
    }
    *budget = budget
        .checked_sub(envs.len())
        .ok_or("aggregate grounding limit exceeded")?;
    Ok(envs)
}

// Ordinary dependency cycles can remain undecided without a proof seed. Seed
// their potential atoms conservatively; a positive-only Datalog grounder would
// silently discard these cycles and change the defeat/discard conditions.
fn recursive_keys(program: &Program) -> BTreeSet<Key> {
    let mut reach = BTreeSet::new();
    for rule in &program.rules {
        for head in &rule.heads {
            for c in &rule.body {
                if let Condition::Logic(p) = c {
                    reach.insert((key(p), key(head)));
                }
            }
        }
    }
    loop {
        let old = reach.clone();
        for (a, b) in &old {
            for (c, d) in &old {
                if b == c {
                    reach.insert((a.clone(), d.clone()));
                }
            }
        }
        if reach.len() == old.len() {
            break;
        }
    }
    reach
        .into_iter()
        .filter_map(|(a, b)| (a == b).then_some(a))
        .collect()
}

fn intern_atom(table: &mut Vec<Pattern>, p: &Pattern) -> Result<Literal> {
    let mut base = p.clone();
    base.negation = false;
    if !table.contains(&base) {
        table.push(base);
    }
    encoded(table, p)
}

pub(super) fn evaluate(
    program: &Program,
    registry: Option<&FunctionRegistry>,
    limit: usize,
) -> Result<Evaluation> {
    validate(program, &[], registry)?;
    for rule in &program.rules {
        validate_bindings(rule)?;
    }
    let recursive = recursive_keys(program);
    let stages = schedule(program)?;
    let stage_count = stages.iter().copied().max().unwrap_or(0) + 1;
    let mut domain = BTreeSet::new();
    for rule in &program.rules {
        for p in patterns(rule) {
            domain.extend(
                p.args
                    .iter()
                    .filter(|t| !matches!(t, Term::Variable(_)))
                    .cloned(),
            );
        }
        for c in &rule.body {
            match c {
                Condition::Logic(_) => (),
                Condition::Bind(_, e) | Condition::BindValue(_, e) => constants(e, &mut domain),
                Condition::Compare(_, a, b) => {
                    constants(a, &mut domain);
                    constants(b, &mut domain);
                }
                Condition::Fold(f) => {
                    constants(&f.extract, &mut domain);
                    if let Some(seed) = &f.seed {
                        constants(seed, &mut domain);
                    }
                }
            }
        }
    }
    let mut budget = limit;
    loop {
        let choices: Vec<_> = domain.iter().cloned().collect();
        let mut candidates = BTreeSet::new();
        for p in program.rules.iter().flat_map(patterns) {
            if vars(p).is_empty() {
                candidates.insert(p.clone());
            } else if recursive.contains(&key(p)) {
                for env in assignments(
                    &vars(p).into_iter().collect::<Vec<_>>(),
                    &choices,
                    &mut budget,
                )? {
                    candidates.insert(apply(p, &env));
                }
            }
        }
        let mut table = Vec::new();
        let mut theory = Theory::new();
        let mut labels: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for stage in 0..stage_count {
            let rows: Vec<_> = observations(&table, &theory)?
                .into_iter()
                .filter(|o| o.tag.is_positive())
                .map(|o| o.atom)
                .collect();
            let mut emitted = BTreeSet::new();
            loop {
                let before = candidates.len();
                for (i, rule) in program
                    .rules
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| stages[*i] == stage)
                {
                    for env in
                        instances(rule, &candidates, &rows, registry, &mut domain, &mut budget)?
                    {
                        if !emitted.insert((i, env.clone())) {
                            continue;
                        }
                        let j = emitted.len();
                        for head in &rule.heads {
                            candidates.insert(apply(head, &env));
                        }
                        let mut body = rule
                            .body
                            .iter()
                            .filter_map(|c| match c {
                                Condition::Logic(p) => Some(apply(p, &env)),
                                _ => None,
                            })
                            .map(|p| intern_atom(&mut table, &p))
                            .collect::<Result<Vec<_>>>()?;
                        if rule.body.iter().any(|c| matches!(c, Condition::Fold(_))) {
                            let guard = Literal::simple(format!("g{stage}"));
                            theory.add_rule(Rule::defeasible(
                                format!("g:{stage}"),
                                vec![],
                                guard.clone(),
                            ));
                            body.push(guard);
                        }
                        for (h, head) in rule.heads.iter().enumerate() {
                            let label = format!("u:{i}:{j}:{h}");
                            let head = intern_atom(&mut table, &apply(head, &env))?;
                            theory.add_rule(Rule::new(
                                label.clone(),
                                rule.kind,
                                body.clone(),
                                vec![head].into_iter().collect::<crate::rule::RuleHead>(),
                            ));
                            labels.entry(rule.label.clone()).or_default().push(label);
                        }
                    }
                }
                if candidates.len() == before {
                    break;
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
        // Replay every stratum when a computed constant extends the universe:
        // earlier ordinary cycles/attackers must also exist for the new value.
        if domain.len() == choices.len() {
            return Ok(Evaluation {
                conclusions: observations(&table, &theory)?,
                stage_count,
                theory,
                table,
            });
        }
    }
}
