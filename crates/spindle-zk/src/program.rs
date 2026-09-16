use crate::{Error, Result};
use spindle_core::{
    body::BodyLiteral, conclusion::ConclusionType, intern::resolve, literal::Literal,
    rule::RuleType, term::Term, theory::Theory,
};
use std::collections::{BTreeMap, HashMap};

mod aggregate;
mod integer;

pub(crate) type Wire = usize;
pub(crate) const FALSE: Wire = 0;
pub(crate) const TRUE: Wire = 1;
pub(crate) const MAX_LITERALS: usize = 128;
pub(crate) const MAX_RULES: usize = 256;
pub(crate) const MAX_GATES: usize = 1_000_000;
// Domain size, candidate assignments, variables per scope, predicate arity.
pub(crate) const AGGREGATE_LIMITS: [usize; 4] = [64, 4096, 16, 16];

/// Four tags in the order used by the operational reference.
pub(crate) const TAGS: [ConclusionType; 4] = [
    ConclusionType::DefinitelyProvable,
    ConclusionType::DefinitelyNotProvable,
    ConclusionType::DefeasiblyProvable,
    ConclusionType::DefeasiblyNotProvable,
];

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Op {
    Constant(bool),
    Input(usize),
    Not(Wire),
    And(Wire, Wire),
    Or(Wire, Wire),
}

#[derive(Default)]
struct Builder {
    ops: Vec<Op>,
    intern: HashMap<Op, Wire>,
    exhausted: bool,
}

impl Builder {
    fn push(&mut self, op: Op) -> Wire {
        if let Some(&wire) = self.intern.get(&op) {
            return wire;
        }
        if self.ops.len() >= MAX_GATES {
            self.exhausted = true;
            return FALSE;
        }
        let wire = self.ops.len();
        self.ops.push(op);
        self.intern.insert(op, wire);
        wire
    }

    fn not(&mut self, a: Wire) -> Wire {
        match self.ops[a] {
            Op::Constant(v) => usize::from(!v),
            Op::Not(b) => b,
            _ => self.push(Op::Not(a)),
        }
    }

    fn and(&mut self, a: Wire, b: Wire) -> Wire {
        if a == FALSE || b == FALSE {
            return FALSE;
        }
        if a == TRUE || a == b {
            return b;
        }
        if b == TRUE {
            return a;
        }
        self.push(Op::And(a.min(b), a.max(b)))
    }

    fn or(&mut self, a: Wire, b: Wire) -> Wire {
        if a == TRUE || b == TRUE {
            return TRUE;
        }
        if a == FALSE || a == b {
            return b;
        }
        if b == FALSE {
            return a;
        }
        self.push(Op::Or(a.min(b), a.max(b)))
    }

    fn all(&mut self, wires: impl IntoIterator<Item = Wire>) -> Wire {
        wires.into_iter().fold(TRUE, |acc, w| self.and(acc, w))
    }

    fn any(&mut self, wires: impl IntoIterator<Item = Wire>) -> Wire {
        wires.into_iter().fold(FALSE, |acc, w| self.or(acc, w))
    }
}

struct Rule {
    label: String,
    kind: RuleType,
    head: usize,
    body: Vec<usize>,
    // A disabled lowered instance is absent, not an undecided ordinary rule.
    enabled: Wire,
}

/// Injective internal literal key, using SPL's quoting convention rather than
/// JSON escapes (SPL preserves the character after a backslash literally).
pub(crate) fn literal_spl(literal: &Literal) -> String {
    let name = Literal::simple(literal.name()).to_spl();
    let mut parts = vec![name[1..name.len() - 1].to_string()];
    parts.extend(literal.predicate_args().iter().map(|term| match term {
        Term::Symbol(id) => format!(
            "\"{}\"",
            resolve(*id).replace('\\', "\\\\").replace('"', "\\\"")
        ),
        other => other.to_string(),
    }));
    // These names are ordinary predicates as bare atoms, but dispatch to
    // special forms in list position. Keep their zero-arity spelling bare.
    let atom = if literal.predicate_args().is_empty()
        && matches!(
            literal.name(),
            "not" | "must" | "may" | "forbidden" | "during"
        ) {
        parts[0].clone()
    } else {
        format!("({})", parts.join(" "))
    };
    if literal.negation {
        format!("(not {atom})")
    } else {
        atom
    }
}

/// A deterministic inference program, separate from private fact values.
///
/// Supports ground nonmodal rules and a bounded finite-domain aggregate profile.
/// Aggregate programs require an explicit public `aggregate-domain` bound.
#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) ops: Vec<Op>,
    pub(crate) outputs: Vec<[Wire; 4]>,
    pub(crate) inputs: Vec<Literal>,
    pub(crate) literals: Vec<Literal>,
    pub(crate) input_wires: Vec<Wire>,
    pub(crate) valid: Wire,
}

pub(crate) fn check_literal(literal: &Literal) -> Result<()> {
    let variable = |name: &str| name.starts_with('?') || name == "_";
    if !literal.mode.is_empty()
        || !literal.temporal.is_empty()
        || literal.temporal_expr.is_some()
        || literal.interval_var.is_some()
        || variable(literal.name())
        || literal.predicate_args().iter().any(|t| match t {
            Term::Symbol(s) => variable(resolve(*s)),
            Term::Integer(_) => false,
            Term::Decimal(_) | Term::Float(_) => true,
        })
    {
        return Err(Error::Unsupported(
            "ground nonmodal integer/symbol literal required".into(),
        ));
    }
    Ok(())
}

impl Program {
    /// Retain the public claim's dependency cone and the complete commitment
    /// input vector. This is a circuit projection, not a reusable reasoner:
    /// outputs other than the selected claim are deliberately unavailable.
    pub(crate) fn for_claim(&self, literal: usize, tag: usize) -> Self {
        let selected = self.outputs[literal][tag];
        let mut live = vec![false; self.ops.len()];
        for wire in [FALSE, TRUE, selected, self.valid]
            .into_iter()
            .chain(self.input_wires.iter().copied())
        {
            live[wire] = true;
        }
        // The compiler emits operands before consumers, so one reverse pass
        // computes the transitive dependency closure without recursive stacks.
        for wire in (0..self.ops.len()).rev() {
            if live[wire] {
                match self.ops[wire] {
                    Op::Not(a) => live[a] = true,
                    Op::And(a, b) | Op::Or(a, b) => {
                        live[a] = true;
                        live[b] = true;
                    }
                    Op::Constant(_) | Op::Input(_) => {}
                }
            }
        }
        let mut remap = vec![FALSE; self.ops.len()];
        let mut ops = Vec::new();
        for (wire, op) in self.ops.iter().enumerate() {
            if !live[wire] {
                continue;
            }
            remap[wire] = ops.len();
            ops.push(match *op {
                Op::Not(a) => Op::Not(remap[a]),
                Op::And(a, b) => Op::And(remap[a], remap[b]),
                Op::Or(a, b) => Op::Or(remap[a], remap[b]),
                Op::Constant(_) | Op::Input(_) => *op,
            });
        }
        let mut outputs = vec![[FALSE; 4]; self.outputs.len()];
        outputs[literal][tag] = remap[selected];
        Self {
            ops,
            outputs,
            inputs: self.inputs.clone(),
            literals: self.literals.clone(),
            input_wires: self.input_wires.iter().map(|&w| remap[w]).collect(),
            valid: remap[self.valid],
        }
    }

    /// Compile a ground policy with a declared public universe of private facts.
    ///
    /// Tags grow monotonically from the empty state. At most four times the
    /// literal count productive rounds are possible. Unrolling that bound
    /// computes the least fixed point, not an arbitrary self-supporting model.
    pub fn compile(theory: &Theory, inputs: &[Literal]) -> Result<Self> {
        if theory.rule_count() > MAX_RULES || inputs.len() > MAX_LITERALS {
            return Err(Error::ResourceLimit("rule or input count".into()));
        }
        if theory.has_circular_superiority() {
            return Err(Error::InvalidInput("cyclic superiority".into()));
        }
        for pair in theory.superiorities() {
            if theory.get_rule(&pair.superior).is_none()
                || theory.get_rule(&pair.inferior).is_none()
            {
                return Err(Error::InvalidInput(
                    "superiority references an unknown rule".into(),
                ));
            }
        }
        let trust = theory.trust_policy();
        if !trust.trust_map.is_empty()
            || !trust.thresholds.is_empty()
            || !trust.decay_map.is_empty()
        {
            return Err(Error::Unsupported("trust-weighted reasoning".into()));
        }
        if spindle_core::aggregation::source::has_folds(theory) {
            return aggregate::compile(theory, inputs);
        }
        let mut atoms = BTreeMap::new();
        let mut insert = |lit: &Literal| -> Result<()> {
            check_literal(lit)?;
            atoms.insert(literal_spl(lit), lit.clone());
            let complement = lit.complement();
            atoms.insert(literal_spl(&complement), complement);
            Ok(())
        };
        for lit in inputs {
            insert(lit)?;
        }
        for rule in theory.rules() {
            if !rule.mode.is_empty()
                || !rule.temporal.is_empty()
                || !rule.constraints.is_empty()
                || !rule.state_queries.is_empty()
                || rule.head.len() != 1
            {
                return Err(Error::Unsupported(format!(
                    "rule {} requires lowering",
                    rule.label
                )));
            }
            if rule.rule_type == RuleType::Fact && !rule.body.is_empty() {
                return Err(Error::InvalidInput("a fact cannot have premises".into()));
            }
            insert(&rule.head[0])?;
            for body in &rule.body {
                match body {
                    BodyLiteral::Logic(lit) if !lit.has_arith_args() => insert(&lit.to_literal())?,
                    _ => {
                        return Err(Error::Unsupported(format!(
                            "arithmetic or aggregate premise in {}",
                            rule.label
                        )));
                    }
                }
            }
        }
        if atoms.len() > MAX_LITERALS {
            return Err(Error::ResourceLimit(format!(
                "at most {MAX_LITERALS} literals including complements"
            )));
        }
        let literals: Vec<_> = atoms.into_values().collect();
        let ids: BTreeMap<_, _> = literals
            .iter()
            .enumerate()
            .map(|(i, l)| (literal_spl(l), i))
            .collect();
        let mut inputs = inputs.to_vec();
        inputs.sort_by_key(literal_spl);
        if inputs.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::InvalidInput(
                "duplicate private input declaration".into(),
            ));
        }
        let mut b = Builder::default();
        b.push(Op::Constant(false));
        b.push(Op::Constant(true));
        let input_wires: Vec<_> = (0..inputs.len()).map(|i| b.push(Op::Input(i))).collect();
        let mut facts = vec![FALSE; literals.len()];
        for (i, lit) in inputs.iter().enumerate() {
            facts[ids[&literal_spl(lit)]] = input_wires[i];
        }
        let mut rules = Vec::new();
        let mut source_rules: Vec<_> = theory.rules().collect();
        source_rules.sort_by(|a, b| a.label.cmp(&b.label));
        for rule in source_rules {
            let head = ids[&literal_spl(&rule.head[0])];
            if rule.rule_type == RuleType::Fact {
                facts[head] = TRUE;
                continue;
            }
            let body = rule
                .body
                .iter()
                .map(|lit| match lit {
                    BodyLiteral::Logic(lit) => ids[&literal_spl(&lit.to_literal())],
                    _ => unreachable!("profile checked above"),
                })
                .collect();
            rules.push(Rule {
                label: rule.label.clone(),
                kind: rule.rule_type,
                head,
                body,
                enabled: TRUE,
            });
        }
        let complements: Vec<_> = literals
            .iter()
            .map(|l| ids[&literal_spl(&l.complement())])
            .collect();
        let state = Self::close(&mut b, &rules, &facts, &complements, |a, d| {
            theory.is_superior(a, d)
        })?;
        Ok(Self {
            ops: b.ops,
            outputs: state,
            inputs,
            literals,
            input_wires,
            valid: TRUE,
        })
    }

    /// Reusable closure for aggregate stages. Activation is a circuit wire:
    /// an absent instance cannot support, attack, or obstruct negative proofs.
    fn close(
        b: &mut Builder,
        rules: &[Rule],
        facts: &[Wire],
        complements: &[usize],
        superior: impl Fn(&str, &str) -> bool,
    ) -> Result<Vec<[Wire; 4]>> {
        let n = facts.len();
        let mut heads = vec![Vec::new(); n];
        for (i, rule) in rules.iter().enumerate() {
            heads[rule.head].push(i);
        }
        let mut state = vec![[FALSE; 4]; n];
        for _ in 0..4 * n {
            let applicable: Vec<[Wire; 4]> = rules
                .iter()
                .map(|r| {
                    let absent = b.not(r.enabled);
                    let definite = b.all(r.body.iter().map(|&l| state[l][0]));
                    let not_definite = b.any(r.body.iter().map(|&l| state[l][1]));
                    let defeasible = b.all(r.body.iter().map(|&l| state[l][2]));
                    let not_defeasible = b.any(r.body.iter().map(|&l| state[l][3]));
                    [
                        b.and(r.enabled, definite),
                        b.or(absent, not_definite),
                        b.and(r.enabled, defeasible),
                        b.or(absent, not_defeasible),
                    ]
                })
                .collect();
            let mut next = state.clone();
            for q in 0..n {
                let contrary = complements[q];
                let own = &heads[q];
                let productive: Vec<_> = own
                    .iter()
                    .copied()
                    .filter(|&r| rules[r].kind != RuleType::Defeater)
                    .collect();
                let strict: Vec<_> = own
                    .iter()
                    .copied()
                    .filter(|&r| rules[r].kind == RuleType::Strict)
                    .collect();
                let strict_support = b.any(strict.iter().map(|&r| applicable[r][0]));
                let definite = b.or(facts[q], strict_support);
                let strict_discard = b.all(strict.iter().map(|&r| applicable[r][1]));
                let not_fact = b.not(facts[q]);
                let negative_definite = b.and(not_fact, strict_discard);

                let support = b.any(productive.iter().map(|&r| applicable[r][2]));
                let mut safe_attacks = Vec::new();
                let mut winning_attacks = Vec::new();
                for &a in &heads[contrary] {
                    // Direct superiority only: SPEC semantics explicitly forbids
                    // inventing a transitive closure of the named relation.
                    let defenders: Vec<_> = productive
                        .iter()
                        .copied()
                        .filter(|&d| superior(&rules[d].label, &rules[a].label))
                        .collect();
                    let defended = b.any(defenders.iter().map(|&d| applicable[d][2]));
                    safe_attacks.push(b.or(applicable[a][3], defended));
                    let discarded_defenders = b.all(defenders.iter().map(|&d| applicable[d][3]));
                    winning_attacks.push(b.and(applicable[a][2], discarded_defenders));
                }
                let attacks_safe = b.all(safe_attacks);
                let positive = b.all([support, state[contrary][1], attacks_safe]);
                let positive = b.or(state[q][0], positive);
                let all_discarded = b.all(productive.iter().map(|&r| applicable[r][3]));
                let winning = b.any(winning_attacks);
                let negative = b.any([all_discarded, state[contrary][0], winning]);
                let negative = b.and(state[q][1], negative);
                for (tag, value) in [definite, negative_definite, positive, negative]
                    .into_iter()
                    .enumerate()
                {
                    next[q][tag] = b.or(state[q][tag], value);
                }
            }
            if b.exhausted {
                return Err(Error::ResourceLimit(format!(
                    "at most {MAX_GATES} Boolean operations"
                )));
            }
            if next == state {
                break;
            }
            state = next;
        }
        Ok(state)
    }

    /// Number of operations in the witness-independent constraint program.
    pub fn operation_count(&self) -> usize {
        self.ops.len()
    }

    /// Evaluate the compiled relation for selected private facts.
    pub fn evaluate(&self, facts: &[Literal]) -> Result<Vec<(Literal, ConclusionType)>> {
        let witness = self.witness(facts)?;
        Ok(self
            .literals
            .iter()
            .zip(&self.outputs)
            .flat_map(|(lit, wires)| {
                wires
                    .iter()
                    .enumerate()
                    .filter_map(|(tag, &wire)| witness[wire].then_some((lit.clone(), TAGS[tag])))
            })
            .collect())
    }

    pub(crate) fn witness(&self, facts: &[Literal]) -> Result<Vec<bool>> {
        let mut selected = vec![false; self.inputs.len()];
        for fact in facts {
            check_literal(fact)?;
            let Some(index) = self.inputs.iter().position(|input| input == fact) else {
                return Err(Error::InvalidInput(
                    "private fact is not declared by the policy".into(),
                ));
            };
            if selected[index] {
                return Err(Error::InvalidInput("duplicate private fact".into()));
            }
            selected[index] = true;
        }
        let mut values: Vec<bool> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let value = match *op {
                Op::Constant(v) => v,
                Op::Input(i) => selected[i],
                Op::Not(a) => !values[a],
                Op::And(a, b) => values[a] && values[b],
                Op::Or(a, b) => values[a] || values[b],
            };
            values.push(value);
        }
        if !values[self.valid] {
            return Err(Error::InvalidInput(
                "aggregate arithmetic error or result outside the declared finite domain".into(),
            ));
        }
        Ok(values)
    }
}

#[cfg(test)]
mod activation_tests {
    use super::*;

    #[test]
    fn claim_projection_preserves_tags_inputs_and_global_validity() {
        for source in [
            "(normally r a q) (normally s b (not q)) (prefer r s)",
            "(always r a q) (always cycle q a) (normally s b (not q))",
            "(aggregate-domain 0 1 2) (normally r (agg ?n count ?x (row ?x)) (total ?n))",
            "(aggregate-domain 0 1) (given independent) (normally r (agg ?n count ?x (row ?x)) (total ?n))",
        ] {
            let schema = if source.contains("aggregate-domain") {
                "(given (row 0)) (given (row 1))"
            } else {
                "(given a) (given b)"
            };
            let inputs = crate::parse_facts(schema).unwrap();
            let program =
                Program::compile(&spindle_parser::parse_spl(source).unwrap(), &inputs).unwrap();
            for literal in 0..program.literals.len() {
                for tag in 0..4 {
                    let projected = program.for_claim(literal, tag);
                    assert!(projected.ops.len() <= program.ops.len());
                    assert_eq!(projected.inputs, program.inputs);
                    for mask in 0..4 {
                        let facts: Vec<_> = inputs
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| mask & (1 << i) != 0)
                            .map(|(_, fact)| fact.clone())
                            .collect();
                        let full = program.witness(&facts);
                        let reduced = projected.witness(&facts);
                        assert_eq!(full.is_ok(), reduced.is_ok());
                        if let (Ok(full), Ok(reduced)) = (full, reduced) {
                            assert_eq!(
                                full[program.outputs[literal][tag]],
                                reduced[projected.outputs[literal][tag]]
                            );
                            for (&before, &after) in
                                program.input_wires.iter().zip(&projected.input_wires)
                            {
                                assert_eq!(full[before], reduced[after]);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gate_allocator_stops_at_the_declared_budget() {
        let start = std::time::Instant::now();
        let mut b = Builder::default();
        b.push(Op::Constant(false));
        b.push(Op::Constant(true));
        for previous in 0..MAX_GATES - 2 {
            b.push(Op::Not(previous));
        }
        assert_eq!(b.ops.len(), MAX_GATES);
        assert!(!b.exhausted);
        assert_eq!(b.push(Op::Input(0)), FALSE);
        assert!(b.exhausted);
        for i in 1..1024 {
            b.push(Op::Input(i));
        }
        assert_eq!(b.ops.len(), MAX_GATES);
        println!(
            "gate_budget={MAX_GATES} allocation_ms={}",
            start.elapsed().as_millis()
        );
    }

    #[test]
    fn conditional_instances_match_physically_removed_rules() {
        let inputs: Vec<_> = (0..3)
            .map(|i| Literal::simple(format!("gate{i}")))
            .collect();
        for sources in [
            [
                "(normally r0 (and) q)",
                "(normally r1 (and) (not q))",
                "(except r2 q (not q))",
            ],
            [
                "(always r0 q q)",
                "(normally r1 (and) q)",
                "(always r2 (not q) (not q))",
            ],
            [
                "(always r0 (and) q)",
                "(always r1 (and) (not q))",
                "(normally r2 q p)",
            ],
        ] {
            let theory = spindle_parser::parse_spl(&sources.join(" ")).unwrap();
            let baseline = Program::compile(&theory, &inputs).unwrap();
            let ids: BTreeMap<_, _> = baseline
                .literals
                .iter()
                .enumerate()
                .map(|(i, l)| (literal_spl(l), i))
                .collect();
            let mut b = Builder::default();
            b.push(Op::Constant(false));
            b.push(Op::Constant(true));
            let input_wires: Vec<_> = (0..3).map(|i| b.push(Op::Input(i))).collect();
            let mut facts = vec![FALSE; baseline.literals.len()];
            for (i, input) in inputs.iter().enumerate() {
                facts[ids[&literal_spl(input)]] = input_wires[i];
            }
            let rules: Vec<_> = (0..3)
                .map(|i| {
                    let rule = theory.get_rule(&format!("r{i}")).unwrap();
                    Rule {
                        label: rule.label.clone(),
                        kind: rule.rule_type,
                        head: ids[&literal_spl(&rule.head[0])],
                        body: rule
                            .body
                            .iter()
                            .map(|lit| match lit {
                                BodyLiteral::Logic(lit) => ids[&literal_spl(&lit.to_literal())],
                                _ => unreachable!(),
                            })
                            .collect(),
                        enabled: input_wires[i],
                    }
                })
                .collect();
            let complements: Vec<_> = baseline
                .literals
                .iter()
                .map(|l| ids[&literal_spl(&l.complement())])
                .collect();
            let outputs = Program::close(&mut b, &rules, &facts, &complements, |a, d| {
                a == "r0" && d == "r1"
            })
            .unwrap();
            let program = Program {
                ops: b.ops,
                outputs,
                inputs: inputs.clone(),
                literals: baseline.literals,
                input_wires,
                valid: TRUE,
            };
            for mask in 0..8 {
                let selected: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, l)| l.clone())
                    .collect();
                let mut source = sources
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, s)| *s)
                    .collect::<Vec<_>>()
                    .join(" ");
                if mask & 3 == 3 {
                    source.push_str(" (prefer r0 r1)");
                }
                let reduced = spindle_parser::parse_spl(&source).unwrap();
                // Keep the query universe fixed even when all its rules disappear.
                let reduced = Program::compile(&reduced, &program.literals).unwrap();
                let expected = reduced.evaluate(&selected).unwrap();
                let actual = program.evaluate(&selected).unwrap();
                assert_eq!(actual, expected, "activation mask {mask}: {source}");
            }
        }
    }
}
