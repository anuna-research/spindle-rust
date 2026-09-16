//! Signed i64 operations expressed entirely in the constrained Boolean DAG.
//! No arithmetic result is supplied as an unconstrained host-computed witness.

use super::{Builder, FALSE, TRUE, Wire};

pub(super) type Integer = [Wire; 64];

pub(super) fn constant(value: i64) -> Integer {
    std::array::from_fn(|i| usize::from((value as u64 >> i) & 1 != 0))
}

fn xor(b: &mut Builder, x: Wire, y: Wire) -> Wire {
    let either = b.or(x, y);
    let both = b.and(x, y);
    let not_both = b.not(both);
    b.and(either, not_both)
}

fn choose(b: &mut Builder, condition: Wire, yes: Wire, no: Wire) -> Wire {
    let yes = b.and(condition, yes);
    let absent = b.not(condition);
    let no = b.and(absent, no);
    b.or(yes, no)
}

pub(super) fn select(b: &mut Builder, condition: Wire, yes: &Integer, no: &Integer) -> Integer {
    std::array::from_fn(|i| choose(b, condition, yes[i], no[i]))
}

/// Return the two's-complement sum and a signed-overflow flag.
pub(super) fn add(b: &mut Builder, x: &Integer, y: &Integer) -> (Integer, Wire) {
    let mut carry = FALSE;
    let sum = std::array::from_fn(|i| {
        let differing = xor(b, x[i], y[i]);
        let bit = xor(b, differing, carry);
        let generated = b.and(x[i], y[i]);
        let propagated = b.and(differing, carry);
        carry = b.or(generated, propagated);
        bit
    });
    let different_signs = xor(b, x[63], y[63]);
    let same_sign = b.not(different_signs);
    let changed_sign = xor(b, x[63], sum[63]);
    (sum, b.and(same_sign, changed_sign))
}

pub(super) fn less(b: &mut Builder, x: &Integer, y: &Integer) -> Wire {
    let mut result = FALSE;
    for i in 0..64 {
        let differing = xor(b, x[i], y[i]);
        // Invert the significance of the sign bit for signed ordering.
        let smaller = if i == 63 { x[i] } else { y[i] };
        result = choose(b, differing, smaller, result);
    }
    result
}

#[derive(Clone, Copy)]
pub(super) enum Reducer {
    Sum,
    Min,
    Max,
}

pub(super) struct Fold {
    pub value: Integer,
    pub present: Wire,
    pub valid: Wire,
}

/// Fold already-distinct, canonically ordered rows from a completed snapshot.
/// `selected` must be the row's constrained +d evidence. Count uses Sum with
/// contribution 1. An unseeded empty fold has present=false; arithmetic errors
/// are tracked separately so they cannot masquerade as an absent result.
pub(super) fn fold(
    b: &mut Builder,
    reducer: Reducer,
    seed: Option<Integer>,
    rows: impl IntoIterator<Item = (Wire, Integer)>,
) -> Fold {
    let mut result = Fold {
        value: seed.unwrap_or_else(|| constant(0)),
        present: if seed.is_some() { TRUE } else { FALSE },
        valid: TRUE,
    };
    for (selected, value) in rows {
        let (combined, overflow) = match reducer {
            Reducer::Sum => add(b, &result.value, &value),
            Reducer::Min => {
                let smaller = less(b, &value, &result.value);
                (select(b, smaller, &value, &result.value), FALSE)
            }
            Reducer::Max => {
                let smaller = less(b, &result.value, &value);
                (select(b, smaller, &value, &result.value), FALSE)
            }
        };
        let combined = select(b, result.present, &combined, &value);
        result.value = select(b, selected, &combined, &result.value);
        let overflow = b.all([selected, result.present, overflow]);
        let no_overflow = b.not(overflow);
        result.valid = b.and(result.valid, no_overflow);
        result.present = b.or(result.present, selected);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::Op;
    use proptest::prelude::*;

    fn builder() -> Builder {
        let mut b = Builder::default();
        b.push(Op::Constant(false));
        b.push(Op::Constant(true));
        b
    }

    fn input(b: &mut Builder, offset: usize) -> Integer {
        std::array::from_fn(|i| b.push(Op::Input(offset + i)))
    }

    fn evaluate(b: &Builder, inputs: &[bool]) -> Vec<bool> {
        assert!(!b.exhausted);
        let mut values: Vec<bool> = Vec::new();
        for op in &b.ops {
            values.push(match *op {
                Op::Constant(v) => v,
                Op::Input(i) => inputs[i],
                Op::Not(a) => !values[a],
                Op::And(a, c) => values[a] && values[c],
                Op::Or(a, c) => values[a] || values[c],
            });
        }
        values
    }

    fn bits(x: i64) -> Vec<bool> {
        (0..64).map(|i| (x as u64 >> i) & 1 != 0).collect()
    }

    fn number(x: &Integer, values: &[bool]) -> i64 {
        x.iter()
            .enumerate()
            .fold(0u64, |acc, (i, &w)| acc | (u64::from(values[w]) << i)) as i64
    }

    fn check_arithmetic(x: i64, y: i64) {
        let mut b = builder();
        let a = input(&mut b, 0);
        let c = input(&mut b, 64);
        let (sum, overflow) = add(&mut b, &a, &c);
        let lt = less(&mut b, &a, &c);
        let values = evaluate(&b, &[bits(x), bits(y)].concat());
        assert_eq!(number(&sum, &values), x.wrapping_add(y));
        assert_eq!(values[overflow], x.checked_add(y).is_none());
        assert_eq!(values[lt], x < y);
    }

    #[test]
    fn signed_boundaries() {
        for x in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            for y in [i64::MIN, -1, 0, 1, i64::MAX] {
                check_arithmetic(x, y);
            }
        }
    }

    #[test]
    fn circuit_rejects_overflow_even_with_forged_validity() {
        use crate::{circuit::ProofCircuit, program::Program};
        use halo2_proofs::{circuit::Value, dev::MockProver, pasta::Fp};
        use spindle_core::Literal;

        let mut b = builder();
        let first = b.push(Op::Input(0));
        let second = b.push(Op::Input(1));
        let result = fold(
            &mut b,
            Reducer::Sum,
            Some(constant(0)),
            [(first, constant(i64::MAX)), (second, constant(1))],
        );
        let program = Program {
            ops: b.ops.clone(),
            outputs: vec![[result.valid; 4]],
            inputs: vec![Literal::simple("a"), Literal::simple("b")],
            literals: vec![Literal::simple("valid")],
            input_wires: vec![first, second],
            valid: result.valid,
        };
        for (selected, accepted) in [([true, false], true), ([true, true], false)] {
            let witness = evaluate(&b, &selected);
            let mut circuit = ProofCircuit {
                program: program.clone(),
                values: Some(witness.iter().map(|&v| Fp::from(u64::from(v))).collect()),
                nonce: Value::known(Fp::from(7)),
                policy_id: [Fp::from(8), Fp::from(9)],
                literal: 0,
                tag: 0,
            };
            let instance = vec![
                Fp::from(8),
                Fp::from(9),
                Fp::from(0),
                Fp::from(0),
                circuit.commitment(Fp::from(7), &witness),
            ];
            let verified = MockProver::run(circuit.k(), &circuit, vec![instance.clone()])
                .unwrap()
                .verify()
                .is_ok();
            assert_eq!(verified, accepted);
            if !accepted {
                circuit.values.as_mut().unwrap()[result.valid] = Fp::from(1);
                assert!(
                    MockProver::run(circuit.k(), &circuit, vec![instance])
                        .unwrap()
                        .verify()
                        .is_err(),
                    "forged overflow validity accepted"
                );
            }
        }
    }

    proptest! {
        #[test]
        fn arithmetic_matches_checked_i64(x in any::<i64>(), y in any::<i64>()) {
            check_arithmetic(x, y);
        }

        #[test]
        fn selected_folds_match_checked_i64(
            x in any::<i64>(), y in any::<i64>(), seed in proptest::option::of(any::<i64>()),
            first in any::<bool>(), second in any::<bool>(),
        ) {
            for reducer in [Reducer::Sum, Reducer::Min, Reducer::Max] {
                let mut b = builder();
                let a = input(&mut b, 0);
                let c = input(&mut b, 64);
                let select_a = b.push(Op::Input(128));
                let select_c = b.push(Op::Input(129));
                let result = fold(&mut b, reducer, seed.map(constant), [(select_a, a), (select_c, c)]);
                let values = evaluate(&b, &[bits(x), bits(y), vec![first, second]].concat());
                let mut expected = seed;
                let mut valid = true;
                for value in [first.then_some(x), second.then_some(y)].into_iter().flatten() {
                    expected = match expected {
                        None => Some(value),
                        Some(old) => match reducer {
                            Reducer::Sum => {
                                valid &= old.checked_add(value).is_some();
                                Some(old.wrapping_add(value))
                            },
                            Reducer::Min => Some(old.min(value)),
                            Reducer::Max => Some(old.max(value)),
                        }
                    };
                }
                prop_assert_eq!(values[result.valid], valid);
                prop_assert_eq!(values[result.present], expected.is_some());
                if let Some(expected) = expected {
                    prop_assert_eq!(number(&result.value, &values), expected);
                }
            }
        }
    }
}
