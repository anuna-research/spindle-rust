//! SPEC-025 REQ-001/002/003: constrained Boolean execution and fact commitment.
use crate::program::{Op, Program, Wire};
use ff::Field;
use halo2_gadgets::poseidon::{
    Hash, Pow5Chip, Pow5Config,
    primitives::{self, ConstantLength, P128Pow5T3},
};
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::CircuitCost,
    pasta::{Eq, Fp},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector},
    poly::Rotation,
};

const INFERENCE_LANES: usize = 4;

#[derive(Clone, Debug)]
pub(crate) struct Config {
    lanes: [Lane; INFERENCE_LANES],
    instance: Column<Instance>,
    hash: Pow5Config<Fp, 3, 2>,
}

#[derive(Clone, Debug)]
struct Lane {
    advice: [Column<Advice>; 3],
    boolean: Selector,
    not: Selector,
    and: Selector,
    or: Selector,
}

#[derive(Clone)]
pub(crate) struct ProofCircuit {
    pub program: Program,
    pub values: Option<Vec<Fp>>,
    pub nonce: Value<Fp>,
    pub policy_id: [Fp; 2],
    pub literal: usize,
    pub tag: usize,
}

pub(crate) fn hash2(left: Fp, right: Fp) -> Fp {
    primitives::Hash::<_, P128Pow5T3, ConstantLength<2>, 3, 2>::init().hash([left, right])
}

impl ProofCircuit {
    pub fn commitment(&self, nonce: Fp, witness: &[bool]) -> Fp {
        let root = hash2(hash2(nonce, self.policy_id[0]), self.policy_id[1]);
        self.program.input_wires.iter().fold(root, |root, &wire| {
            hash2(root, Fp::from(u64::from(witness[wire])))
        })
    }

    pub fn k(&self) -> u32 {
        // SIMPLIFY: conservative upper bound for the serial floor planner;
        // replace with measured layout sizing when profiling requires it
        // (SPEC-025 ADR-002). Each two-word hash needs fewer than 128 rows.
        let rows = self.program.ops.len().div_ceil(INFERENCE_LANES)
            + 128 * (self.program.inputs.len() + 2)
            + 128;
        rows.next_power_of_two().trailing_zeros().max(8)
    }

    /// Exact byte count for this pinned no-lookup circuit configuration.
    /// This synthesizes public layout only; it does not generate curve parameters
    /// or verification keys and does not inspect private witness values.
    pub fn proof_len(&self) -> usize {
        let estimated: usize =
            CircuitCost::<Eq, Self>::measure(self.k(), &self.without_witnesses())
                .proof_size(1)
                .into();
        // Halo2 0.3.5 dev/cost.rs unconditionally includes the lookup opening
        // set [-1,0]. We have no lookups: shared Poseidon state queries
        // [-1,0,1], partial/fixed columns [0], and permutation products [0,1]
        // or their chaining set.
        // Thus exactly one extra scalar is counted. Revisit this correction
        // on any circuit/backend change. Proving checks the estimate against
        // the actual transcript; real-proof regressions exercise both paths.
        estimated - 32
    }

    /// Recognize every point/scalar encoding before parameters or keys exist.
    /// This is the pinned Halo2 0.3.5, single-proof, no-lookup transcript grammar;
    /// it does not authenticate the Fiat-Shamir challenges or proof equations.
    pub fn encoding_is_valid(&self, bytes: &[u8]) -> bool {
        use halo2_proofs::{
            pasta::EqAffine,
            transcript::{Blake2bRead, Challenge255, TranscriptRead},
        };
        if !bytes.len().is_multiple_of(32) {
            return false;
        }
        let mut cs = ConstraintSystem::default();
        Self::configure(&mut cs);
        let degree = cs.degree();
        // Equality columns: all inference lanes, public instance, dedicated
        // constant column, and Poseidon's three rc_b columns. The partial S-box
        // advice column is not an equality column.
        let advice = 3 * INFERENCE_LANES + 1;
        let permutation_columns = 3 * INFERENCE_LANES + 5;
        let permutation_products = permutation_columns.div_ceil(degree - 2);
        let prefix_points = advice + permutation_products + 1 + (degree - 1);
        // Actual opening sets: [0], [-1,0,1], [0,1], and the permutation chaining
        // set. Unlike CircuitCost, the verifier adds no unused lookup set.
        let point_sets = 4;
        let k = self.k() as usize;
        let Some(evaluations) =
            (bytes.len() / 32).checked_sub(prefix_points + point_sets + 2 * k + 4)
        else {
            return false;
        };
        let mut remaining = bytes;
        let valid = {
            let mut transcript = Blake2bRead::<_, EqAffine, Challenge255<_>>::init(&mut remaining);
            (|| -> std::io::Result<()> {
                // plonk/verifier.rs: advice, permutation, random vanishing and
                // quotient commitments, followed by all polynomial evaluations.
                for _ in 0..prefix_points {
                    transcript.read_point()?;
                }
                for _ in 0..evaluations {
                    transcript.read_scalar()?;
                }
                // poly/multiopen/verifier.rs: Q' commitment and one scalar per set.
                transcript.read_point()?;
                for _ in 0..point_sets {
                    transcript.read_scalar()?;
                }
                // poly/commitment/verifier.rs: S, k pairs of L/R, then c and f.
                for _ in 0..1 + 2 * k {
                    transcript.read_point()?;
                }
                transcript.read_scalar()?;
                transcript.read_scalar()?;
                Ok(())
            })()
            .is_ok()
        };
        valid && remaining.is_empty()
    }

    fn value(&self, wire: Wire) -> Value<Fp> {
        self.values
            .as_ref()
            .map_or(Value::unknown(), |v| Value::known(v[wire]))
    }
}

impl Circuit<Fp> for ProofCircuit {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self {
            program: self.program.clone(),
            values: None,
            nonce: Value::unknown(),
            policy_id: self.policy_id,
            literal: self.literal,
            tag: self.tag,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        let constant = meta.fixed_column();
        meta.enable_constant(constant);
        let lanes = std::array::from_fn(|_| {
            let advice = std::array::from_fn(|_| meta.advice_column());
            for col in advice {
                meta.enable_equality(col);
            }
            let boolean = meta.selector();
            let not = meta.selector();
            let and = meta.selector();
            let or = meta.selector();
            meta.create_gate("input bit", |meta| {
                let s = meta.query_selector(boolean);
                let out = meta.query_advice(advice[2], Rotation::cur());
                vec![s * out.clone() * (out - halo2_proofs::plonk::Expression::Constant(Fp::ONE))]
            });
            meta.create_gate("not", |meta| {
                let s = meta.query_selector(not);
                let a = meta.query_advice(advice[0], Rotation::cur());
                let out = meta.query_advice(advice[2], Rotation::cur());
                vec![s * (a + out - halo2_proofs::plonk::Expression::Constant(Fp::ONE))]
            });
            meta.create_gate("and", |meta| {
                let s = meta.query_selector(and);
                let a = meta.query_advice(advice[0], Rotation::cur());
                let b = meta.query_advice(advice[1], Rotation::cur());
                let out = meta.query_advice(advice[2], Rotation::cur());
                vec![s * (a * b - out)]
            });
            meta.create_gate("or", |meta| {
                let s = meta.query_selector(or);
                let a = meta.query_advice(advice[0], Rotation::cur());
                let b = meta.query_advice(advice[1], Rotation::cur());
                let out = meta.query_advice(advice[2], Rotation::cur());
                vec![s * (a.clone() + b.clone() - a * b - out)]
            });
            Lane {
                advice,
                boolean,
                not,
                and,
                or,
            }
        });
        // Inference and commitment hashing occupy separate regions. Reuse the
        // three advice columns instead of carrying three mostly empty extra
        // polynomials over the entire circuit domain. Independent selectors
        // keep each gate family inactive in the other's rows.
        let state = lanes[0].advice;
        let partial = meta.advice_column();
        let rc_a = std::array::from_fn(|_| meta.fixed_column());
        let rc_b = std::array::from_fn(|_| meta.fixed_column());
        meta.enable_constant(rc_b[0]);
        let hash = Pow5Chip::configure::<P128Pow5T3>(meta, state, partial, rc_a, rc_b);
        Config {
            lanes,
            instance,
            hash,
        }
    }

    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let (cells, nonce, public) = layouter.assign_region(
            || "compiled inference",
            |mut region| {
                let mut cells: Vec<AssignedCell<Fp, Fp>> =
                    Vec::with_capacity(self.program.ops.len());
                for (wire, op) in self.program.ops.iter().enumerate() {
                    let row = wire / config.lanes.len();
                    let lane = &config.lanes[wire % config.lanes.len()];
                    let output = match *op {
                        Op::Constant(v) => region.assign_advice_from_constant(
                            || "constant",
                            lane.advice[2],
                            row,
                            Fp::from(u64::from(v)),
                        )?,
                        _ => region.assign_advice(
                            || "result",
                            lane.advice[2],
                            row,
                            || self.value(wire),
                        )?,
                    };
                    match *op {
                        Op::Constant(_) => {}
                        Op::Input(_) => lane.boolean.enable(&mut region, row)?,
                        Op::Not(a) => {
                            lane.not.enable(&mut region, row)?;
                            cells[a].copy_advice(|| "operand", &mut region, lane.advice[0], row)?;
                        }
                        Op::And(a, b) | Op::Or(a, b) => {
                            if matches!(op, Op::And(..)) {
                                lane.and.enable(&mut region, row)?;
                            } else {
                                lane.or.enable(&mut region, row)?;
                            }
                            cells[a].copy_advice(|| "left", &mut region, lane.advice[0], row)?;
                            cells[b].copy_advice(|| "right", &mut region, lane.advice[1], row)?;
                        }
                    }
                    cells.push(output);
                }
                let selected = self.program.outputs[self.literal][self.tag];
                region.constrain_constant(cells[selected].cell(), Fp::ONE)?;
                let row = self.program.ops.len().div_ceil(config.lanes.len());
                let nonce = region.assign_advice(
                    || "private blinding",
                    config.lanes[0].advice[0],
                    row,
                    || self.nonce,
                )?;
                let mut public = Vec::new();
                for (i, value) in [
                    self.policy_id[0],
                    self.policy_id[1],
                    Fp::from(self.literal as u64),
                    Fp::from(self.tag as u64),
                ]
                .into_iter()
                .enumerate()
                {
                    public.push(region.assign_advice_from_constant(
                        || "public statement",
                        config.lanes[0].advice[2],
                        row + i,
                        value,
                    )?);
                }
                Ok((cells, nonce, public))
            },
        )?;
        for (i, cell) in public.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), config.instance, i)?;
        }
        // All words are the actual inference-input cells, not duplicate witness
        // assignments. The gadget copy constraints bind hashing to inference.
        let mut root = nonce;
        let words = public[..2]
            .iter()
            .cloned()
            .chain(self.program.input_wires.iter().map(|&w| cells[w].clone()));
        for word in words {
            let hasher = Hash::<_, _, P128Pow5T3, ConstantLength<2>, 3, 2>::init(
                Pow5Chip::construct(config.hash.clone()),
                layouter.namespace(|| "commitment init"),
            )?;
            root = hasher.hash(layouter.namespace(|| "commitment word"), [root, word])?;
        }
        layouter.constrain_instance(root.cell(), config.instance, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::parse_facts;
    use halo2_proofs::dev::MockProver;

    #[test]
    fn packed_row_boundaries_bind_same_row_operands() {
        for operations in [
            2 * INFERENCE_LANES - 1,
            2 * INFERENCE_LANES,
            2 * INFERENCE_LANES + 1,
        ] {
            let theory = spindle_parser::parse_spl("(given query)").unwrap();
            let inputs = parse_facts("(given a)").unwrap();
            let mut program = Program::compile(&theory, &inputs).unwrap();
            let literal = program
                .literals
                .iter()
                .position(|l| l.name() == "query" && !l.negation)
                .unwrap();
            program.ops = vec![
                Op::Constant(false),
                Op::Constant(true),
                Op::Input(0),
                Op::Not(2),
                Op::Not(3),
            ];
            while program.ops.len() < operations {
                program.ops.push(Op::And(program.ops.len() - 1, 2));
            }
            program.input_wires = vec![2];
            program.valid = 1;
            program.outputs[literal][0] = program.ops.len() - 1;
            let witness = program.witness(&inputs).unwrap();
            let mut circuit = ProofCircuit {
                program,
                values: Some(witness.iter().map(|&v| Fp::from(u64::from(v))).collect()),
                nonce: Value::known(Fp::from(123)),
                policy_id: [Fp::from(11), Fp::from(12)],
                literal,
                tag: 0,
            };
            let root = circuit.commitment(Fp::from(123), &witness);
            let instance = vec![
                Fp::from(11),
                Fp::from(12),
                Fp::from(literal as u64),
                Fp::ZERO,
                root,
            ];
            MockProver::run(circuit.k(), &circuit, vec![instance.clone()])
                .unwrap()
                .assert_satisfied();
            // Wire 3 consumes wire 2 in another lane of the same row.
            circuit.values.as_mut().unwrap()[3] = Fp::ONE;
            assert!(
                MockProver::run(circuit.k(), &circuit, vec![instance])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
    }

    // Synthetic backend envelope, deliberately independent of source lowering.
    fn maximum_budget_circuit() -> ProofCircuit {
        let theory = spindle_parser::parse_spl("(given query)").unwrap();
        let schema = (0..63)
            .map(|i| format!("(given p{i})"))
            .collect::<Vec<_>>()
            .join(" ");
        let inputs = parse_facts(&schema).unwrap();
        let mut program = Program::compile(&theory, &inputs).unwrap();
        let literal = program
            .literals
            .iter()
            .position(|l| l.name() == "query" && !l.negation)
            .unwrap();
        while program.ops.len() < crate::program::MAX_GATES {
            let previous = program.ops.len() - 1;
            program.ops.push(match previous % 3 {
                0 => Op::Not(previous),
                1 => Op::And(previous, program.input_wires[previous % inputs.len()]),
                _ => Op::Or(previous, program.input_wires[previous % inputs.len()]),
            });
        }
        let last = program.ops.len() - 1;
        // Keep the dependency chain while making the selected output true.
        // Do not run compiler simplification: every row is part of this fixture.
        program.ops[last] = Op::Or(last - 1, 1);
        program.outputs[literal][0] = last;
        ProofCircuit {
            program,
            values: None,
            nonce: Value::unknown(),
            policy_id: [Fp::from(11), Fp::from(12)],
            literal,
            tag: 0,
        }
    }

    #[test]
    #[ignore = "maximum gate-budget layout measurement; allocates substantial memory"]
    fn maximum_budget_layout_is_synthesizable() {
        let circuit = maximum_budget_circuit();
        assert_eq!(circuit.k(), 18);
        let start = std::time::Instant::now();
        let transcript_bytes = circuit.proof_len();
        assert!(transcript_bytes < 1_048_576);
        println!(
            "maximum_layout operations={} k={} transcript_bytes={} measure_ms={}",
            circuit.program.ops.len(),
            circuit.k(),
            transcript_bytes,
            start.elapsed().as_millis()
        );
    }

    #[test]
    #[ignore = "maximum gate-budget real proof; requires substantial memory and runtime"]
    fn maximum_budget_proof_roundtrip() {
        use halo2_proofs::{
            pasta::EqAffine,
            plonk::{SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof},
            poly::commitment::Params,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core::OsRng;
        use std::time::Instant;

        let mut circuit = maximum_budget_circuit();
        let witness = circuit.program.witness(&circuit.program.inputs).unwrap();
        assert!(witness[circuit.program.outputs[circuit.literal][circuit.tag]]);
        let nonce = Fp::random(OsRng);
        let root = circuit.commitment(nonce, &witness);
        circuit.values = Some(witness.iter().map(|&v| Fp::from(u64::from(v))).collect());
        circuit.nonce = Value::known(nonce);
        let instance = [
            Fp::from(11),
            Fp::from(12),
            Fp::from(circuit.literal as u64),
            Fp::ZERO,
            root,
        ];
        let expected = circuit.proof_len();
        let start = Instant::now();
        let params: Params<EqAffine> = Params::new(circuit.k());
        let empty = circuit.without_witnesses();
        let vk = keygen_vk(&params, &empty).unwrap();
        let pk = keygen_pk(&params, vk, &empty).unwrap();
        let keygen_ms = start.elapsed().as_millis();
        eprintln!("maximum_proof keygen_ms={keygen_ms}");
        let start = Instant::now();
        let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<_>>::init(Vec::new());
        create_proof(
            &params,
            &pk,
            &[circuit],
            &[&[&instance]],
            OsRng,
            &mut transcript,
        )
        .unwrap();
        let proof = transcript.finalize();
        assert_eq!(proof.len(), expected);
        assert!(empty.encoding_is_valid(&proof));
        let prove_ms = start.elapsed().as_millis();
        // Reconstruct the verification key independently, not from the prover's
        // key. Reuse only the transparent parameters in this backend benchmark.
        drop(pk);
        let start = Instant::now();
        let vk = keygen_vk(&params, &empty).unwrap();
        let mut transcript = Blake2bRead::<_, EqAffine, Challenge255<_>>::init(proof.as_slice());
        verify_proof(
            &params,
            &vk,
            SingleVerifier::new(&params),
            &[&[&instance]],
            &mut transcript,
        )
        .unwrap();
        println!(
            "maximum_proof keygen_ms={keygen_ms} prove_ms={prove_ms} verify_with_keygen_ms={} transcript_bytes={}",
            start.elapsed().as_millis(),
            proof.len()
        );
    }

    #[test]
    fn projected_independent_claim_rejects_forged_aggregate_validity() {
        let theory = spindle_parser::parse_spl(
            "(aggregate-domain 0 1 9223372036854775807) (given independent)
             (normally total (agg ?n sum ?v (row ?v)) (total ?n))",
        )
        .unwrap();
        let inputs = parse_facts("(given (row 1)) (given (row 9223372036854775807))").unwrap();
        let program = Program::compile(&theory, &inputs).unwrap();
        let literal = program
            .literals
            .iter()
            .position(|l| l.name() == "independent" && !l.negation)
            .unwrap();
        let program = program.for_claim(literal, 0);
        assert!(program.witness(&inputs).is_err());
        // Bypass the honest evaluator, selecting both overflowing contributions
        // and forging global validity before recomputing every consumer.
        let mut forged: Vec<bool> = Vec::new();
        for (wire, op) in program.ops.iter().enumerate() {
            let value = match *op {
                Op::Constant(v) => v,
                Op::Input(_) => true,
                Op::Not(a) => !forged[a],
                Op::And(a, b) => forged[a] && forged[b],
                Op::Or(a, b) => forged[a] || forged[b],
            };
            forged.push(if wire == program.valid { true } else { value });
        }
        assert!(forged[program.outputs[literal][0]]);
        let circuit = ProofCircuit {
            values: Some(forged.iter().map(|&v| Fp::from(u64::from(v))).collect()),
            program,
            nonce: Value::known(Fp::from(123)),
            policy_id: [Fp::from(11), Fp::from(12)],
            literal,
            tag: 0,
        };
        let root = circuit.commitment(Fp::from(123), &forged);
        let instance = vec![
            Fp::from(11),
            Fp::from(12),
            Fp::from(literal as u64),
            Fp::ZERO,
            root,
        ];
        assert!(
            MockProver::run(circuit.k(), &circuit, vec![instance])
                .unwrap()
                .verify()
                .is_err()
        );
    }

    #[test]
    fn projected_irrelevant_input_remains_bound_to_commitment() {
        let theory = spindle_parser::parse_spl("(given independent)").unwrap();
        let inputs = parse_facts("(given irrelevant)").unwrap();
        let program = Program::compile(&theory, &inputs).unwrap();
        let literal = program
            .literals
            .iter()
            .position(|l| l.name() == "independent" && !l.negation)
            .unwrap();
        let program = program.for_claim(literal, 0);
        let absent = program.witness(&[]).unwrap();
        let present = program.witness(&inputs).unwrap();
        let mut circuit = ProofCircuit {
            values: Some(absent.iter().map(|&v| Fp::from(u64::from(v))).collect()),
            program,
            nonce: Value::known(Fp::from(123)),
            policy_id: [Fp::from(11), Fp::from(12)],
            literal,
            tag: 0,
        };
        let root = circuit.commitment(Fp::from(123), &absent);
        let instance = vec![
            Fp::from(11),
            Fp::from(12),
            Fp::from(literal as u64),
            Fp::ZERO,
            root,
        ];
        MockProver::run(circuit.k(), &circuit, vec![instance.clone()])
            .unwrap()
            .assert_satisfied();
        // Same nonce, claim and old commitment; only irrelevant private input
        // changes. A fresh nonce would not isolate this binding property.
        circuit.values = Some(present.iter().map(|&v| Fp::from(u64::from(v))).collect());
        assert!(
            MockProver::run(circuit.k(), &circuit, vec![instance])
                .unwrap()
                .verify()
                .is_err()
        );
    }

    // TEST-005: change a gate's meaning and recompute all downstream wires.
    // Unlike isolated bit flips, this detects a missing AND constraint even
    // when every consumer and the committed private inputs remain consistent.
    #[test]
    fn consistently_forged_conjunction_is_rejected() {
        let theory = spindle_parser::parse_spl("(normally r (and a b) q)").unwrap();
        let inputs = parse_facts("(given a) (given b)").unwrap();
        let program = Program::compile(&theory, &inputs).unwrap();
        let facts = parse_facts("(given a)").unwrap();
        let literal = program
            .literals
            .iter()
            .position(|l| l.name() == "q" && !l.negation)
            .unwrap();
        let program = program.for_claim(literal, 2);
        let honest = program.witness(&facts).unwrap();
        assert!(!honest[program.outputs[literal][2]]);
        let mut forged: Vec<bool> = Vec::new();
        for (wire, op) in program.ops.iter().enumerate() {
            let value = match *op {
                Op::Constant(b) => b,
                Op::Input(_) => honest[wire],
                Op::Not(a) => !forged[a],
                Op::And(a, b) | Op::Or(a, b) => forged[a] || forged[b],
            };
            forged.push(value);
        }
        assert!(forged[program.outputs[literal][2]]);
        let circuit = ProofCircuit {
            values: Some(forged.iter().map(|&b| Fp::from(u64::from(b))).collect()),
            program,
            nonce: Value::known(Fp::from(123)),
            policy_id: [Fp::from(11), Fp::from(12)],
            literal,
            tag: 2,
        };
        let root = circuit.commitment(Fp::from(123), &honest);
        let instance = vec![
            Fp::from(11),
            Fp::from(12),
            Fp::from(literal as u64),
            Fp::from(2),
            root,
        ];
        assert!(
            MockProver::run(circuit.k(), &circuit, vec![instance])
                .unwrap()
                .verify()
                .is_err(),
            "accepted an OR trace for an AND policy"
        );
    }

    // TEST-002 and TEST-005: malicious witnesses bypass the honest evaluator.
    #[test]
    fn every_changed_computed_wire_is_rejected() {
        let theory =
            spindle_parser::parse_spl("(normally r a q) (normally s b (not q)) (prefer r s)")
                .unwrap();
        let inputs = parse_facts("(given a) (given b)").unwrap();
        let program = Program::compile(&theory, &inputs).unwrap();
        let literal = program
            .literals
            .iter()
            .position(|l| l.name() == "q" && !l.negation)
            .unwrap();
        let program = program.for_claim(literal, 2);
        let witness = program.witness(&inputs).unwrap();
        let circuit = ProofCircuit {
            values: Some(witness.iter().map(|&b| Fp::from(u64::from(b))).collect()),
            program,
            nonce: Value::known(Fp::from(123)),
            policy_id: [Fp::from(11), Fp::from(12)],
            literal,
            tag: 2,
        };
        let root = circuit.commitment(Fp::from(123), &witness);
        let instance = vec![
            Fp::from(11),
            Fp::from(12),
            Fp::from(literal as u64),
            Fp::from(2),
            root,
        ];
        MockProver::run(circuit.k(), &circuit, vec![instance.clone()])
            .unwrap()
            .assert_satisfied();
        for (wire, op) in circuit.program.ops.iter().enumerate() {
            if matches!(op, Op::Constant(_)) {
                continue;
            }
            let mut corrupt = circuit.clone();
            let values = corrupt.values.as_mut().unwrap();
            values[wire] = Fp::ONE - values[wire];
            assert!(
                MockProver::run(corrupt.k(), &corrupt, vec![instance.clone()])
                    .unwrap()
                    .verify()
                    .is_err(),
                "accepted corrupted wire {wire}: {op:?}"
            );
        }
        let mut non_boolean = circuit.clone();
        non_boolean.values.as_mut().unwrap()[circuit.program.input_wires[0]] = Fp::from(2);
        assert!(
            MockProver::run(non_boolean.k(), &non_boolean, vec![instance.clone()])
                .unwrap()
                .verify()
                .is_err()
        );

        // Recompute an entirely consistent trace for different private facts.
        // Inference alone accepts it, but the old public commitment must not.
        let changed = parse_facts("(given a)").unwrap();
        let changed = circuit.program.witness(&changed).unwrap();
        let mut changed_circuit = circuit.clone();
        changed_circuit.values = Some(changed.iter().map(|&b| Fp::from(u64::from(b))).collect());
        assert!(
            MockProver::run(changed_circuit.k(), &changed_circuit, vec![instance])
                .unwrap()
                .verify()
                .is_err()
        );
    }
}
