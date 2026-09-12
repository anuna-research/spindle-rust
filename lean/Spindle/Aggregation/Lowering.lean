import Spindle.Aggregation.Inference
import Spindle.Aggregation.PrefixEquivalence
import Spindle.Arith.GroundRule
import Spindle.Aggregation.Primitives

/-!
# Finite-domain aggregate-schema lowering

Schemas are grounded over an explicit finite domain. Folds read completed lower
rows, never candidate heads. Integer sum/min/max, binds and comparisons are
supported; unknown functions and non-integer arithmetic fail explicitly.

A successful aggregate premise is backed by a fresh defeasible closure guard.
Strict rules retain their kind but cannot acquire +D through that guard. This
is an explicit conservative evidence policy, not a new ordinary SDL +D axiom.
Defeasible snapshot evidence is the default policy. An explicit alternative
rejects aggregate-bearing strict rules.
-/
namespace Spindle.Aggregation

inductive AggregatePolicy where
  | defeasibleEvidence
  | rejectStrict
  deriving DecidableEq, Repr

def base (p : Pattern) : Pattern := { p with atom := { p.atom with negation := false } }

/-- A finite interner avoids collisions between structured predicate arguments.
Unary names are intentionally simple reference identifiers, not an export format. -/
def encodeAtom (table : List Pattern) (p : Pattern) : Literal :=
  ⟨String.ofList ('a' :: List.replicate (table.idxOf (base p)) 'a'), p.atom.negation, none⟩

def closureGuard (stage : Nat) : Literal :=
  Literal.pos (String.ofList ('g' :: List.replicate stage 'g'))

def loweredOwner (table : List Pattern) (stages : DependencyNode → Nat) (l : Literal) : Nat :=
  if l.name.toList.head? = some 'g' then l.name.length - 1
  else match table[l.name.length - 1]? with
    | some p => stages (.predicate p.key)
    | none => 0

theorem loweredOwner_complement (table : List Pattern) (stages : DependencyNode → Nat) (l : Literal) :
    loweredOwner table stages l.complement = loweredOwner table stages l := rfl

theorem encodeAtom_complement (table : List Pattern) (p : Pattern) :
    encodeAtom table p.complement = (encodeAtom table p).complement := rfl

theorem encodeAtom_owner (table : List Pattern) (stages : DependencyNode → Nat) (p : Pattern)
    (member : base p ∈ table) : loweredOwner table stages (encodeAtom table p) = stages (.predicate p.key) := by
  simp only [loweredOwner, encodeAtom]
  simp only [String.toList_ofList, List.head?_cons, String.length_ofList,
    List.length_cons, List.length_replicate, Nat.add_sub_cancel]
  rw [List.getElem?_idxOf member]
  rfl

/-- Distinct structured atoms remain distinct in the finite encoding. -/
theorem encodeAtom_injective_on (table : List Pattern) (p q : Pattern)
    (hp : base p ∈ table) (hq : base q ∈ table)
    (encoded : encodeAtom table p = encodeAtom table q) : p = q := by
  have index : table.idxOf (base p) = table.idxOf (base q) := by
    have sizes := congrArg (fun l : Literal => l.name.length) encoded
    simpa only [encodeAtom, String.length_ofList, List.length_cons, List.length_replicate,
      Nat.add_left_inj] using sizes
  have sameBase : base p = base q := by
    have sameEntry := congrArg (fun i => table[i]?) index
    simpa only [List.getElem?_idxOf hp, List.getElem?_idxOf hq, Option.some.injEq] using sameEntry
  have negation : p.atom.negation = q.atom.negation := congrArg Literal.negated encoded
  rcases p with ⟨⟨pn, pb, pa⟩, pm, pmb⟩
  rcases q with ⟨⟨qn, qb, qa⟩, qm, qmb⟩
  simp_all [base]

theorem encodeAtom_ne_guard (table : List Pattern) (p : Pattern) (stage : Nat) :
    encodeAtom table p ≠ closureGuard stage := by
  intro equal
  have first := congrArg (fun l : Literal => l.name.toList.head?) equal
  simp [encodeAtom, closureGuard, Literal.pos] at first

private def patterns (r : SchemaRule) : List Pattern :=
  r.heads ++ r.body.filterMap (fun c => c.input.map Prod.fst)

/-- Enumerate all relational atoms over the supplied domain, including local
fold variables. Repeated proof paths do not create additional table rows. -/
def atomTable (program : AggregateProgram) (domain : Arith.Domain) : List Pattern :=
  (program.rules.flatMap fun r => (patterns r).flatMap fun p =>
    (Arith.allSubstitutions p.atom.vars.dedup domain).map fun σ => base (p.applySubst σ)).dedup

/-- Matching extends an outer environment separately for each distinct row.
Equal extracted values remain separate contributions. -/
def evalSchemaFold (domain : Arith.Domain) (σ : Arith.Substitution) (rows : List Pattern)
    (fold : FoldExpression) : Except String (Option Int) := do
  let reducer ← reducerNamed fold.reducer
  let seed ← fold.seed.mapM (evalExpr σ)
  let matched := rows.dedup.filterMap (matchRow σ fold.pattern)
  let contributions ← matched.mapM (fun localEnv => evalExpr localEnv fold.extract)
  let value := reducer.eval seed contributions
  match value with
  | none => return none
  | some n =>
    if domain.any (fun t => decide (t = .integer n)) then return some n
    else .error "aggregate result is outside the declared finite domain"

def checkCondition (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (c : Condition) : Except String Bool := do
  match c with
  | .logic _ => return true
  | .bind v expr => return decide (σ.lookup v = some (.integer (← evalExpr σ expr)))
  | .compare op a b => return compareInts op (← evalExpr σ a) (← evalExpr σ b)
  | .fold fold =>
    match ← evalSchemaFold domain σ rows fold with
    | none => return false
    | some value => return decide (σ.lookup fold.resultVar = some (.integer value))

def hasFold (r : SchemaRule) : Bool := r.body.any (fun c => match c with
  | .fold _ => true | _ => false)

def checkedAtom (table : List Pattern) (σ : Arith.Substitution) (p : Pattern) :
    Except String Literal :=
  let grounded := p.applySubst σ
  if grounded.atom.isGround ∧ base grounded ∈ table then .ok (encodeAtom table grounded)
  else .error "schema instance is ungrounded or outside the atom table"

/-- Preserve the original rule kind while making aggregate evidence an explicit
non-definite premise. Guard identifiers cannot collide with encoded user atoms. -/
def emitLoweredRules (r : SchemaRule) (stage : Nat) (heads body : List Literal) : List Rule :=
  let premises := if hasFold r then body ++ [closureGuard stage] else body
  let rules := heads.map (fun head => (⟨"u:" ++ r.label, r.kind, premises, head⟩ : Rule))
  if hasFold r then Rule.defeasible ("g:" ++ toString stage) [] (closureGuard stage) :: rules else rules

theorem emitted_definite_requires_guard (r : SchemaRule) (stage : Nat) (heads body : List Literal)
    (aggregate : hasFold r = true) (rule : Rule)
    (emitted : rule ∈ emitLoweredRules r stage heads body) (definite : rule.isDefinite = true) :
    closureGuard stage ∈ rule.body := by
  simp only [emitLoweredRules, aggregate, if_true, List.mem_cons, List.mem_map] at emitted
  rcases emitted with rfl | ⟨head, _, equal⟩
  · change false = true at definite
    cases definite
  · subst rule
    simp

/-- A closure guard is defeasible support even when the schema rule is strict. -/
def lowerInstance (policy : AggregatePolicy) (domain : Arith.Domain) (table : List Pattern)
    (stage : Nat) (rows : List Pattern) (r : SchemaRule) (σ : Arith.Substitution) :
    Except String (List Rule) := do
  if policy = .rejectStrict ∧ r.kind = .strict ∧ hasFold r then
    throw "strict aggregate rule requires an explicit closure evidence policy"
  let checks ← r.body.mapM (checkCondition domain σ rows)
  if !checks.all id then return []
  let heads ← r.heads.mapM (checkedAtom table σ)
  let body ← r.premises.mapM
    (checkedAtom table σ)
  return emitLoweredRules r stage heads body

/-- Candidate atoms only become fold rows when the completed reasoner proves them. -/
def survivingRows (table : List Pattern) (conclusions : List Conclusion) : List Pattern :=
  (table.flatMap (fun p => [p, p.complement])).filter
    (fun p => (snapshot conclusions).contains (encodeAtom table p))

private def exprSupported : Expression → Bool
  | .term _ => true
  | .call name args => (args.map exprSupported).all id &&
      (name = "+" || name = "*" || (name = "-" && args.length = 2) ||
        ((name = "min" || name = "max") && !args.isEmpty))

private def conditionSupported : Condition → Bool
  | .logic _ => true
  | .bind _ e => exprSupported e
  | .compare _ a b => exprSupported a && exprSupported b
  | .fold f => (reducerNamed f.reducer).toOption.isSome &&
      f.seed.toList.all exprSupported && exprSupported f.extract

private def sourceSupported (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) : Bool :=
  domain.all Arith.Term.isGround && program.rules.all (fun r =>
    (if r.kind = .fact then r.body.isEmpty else true) &&
    !(policy = .rejectStrict && r.kind = .strict && hasFold r) &&
    r.body.all conditionSupported &&
    (patterns r).all (fun p => p.modeName.isNone && !p.modeNegated))

/-- A lowering result retains the final ground theory for independent replay. -/
structure LoweredProgram where
  table : List Pattern
  stages : DependencyNode → Nat
  theory : Theory
  stageCount : Nat

/-- Lower this stage using the completed prior theory and check publication
ownership before extending that theory. -/
def lowerBatch (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (stage : Nat) (theory : Theory) : Except String (List Rule) :=
  let rows := survivingRows table (reason theory).conclusions
  let schemas := program.rules.filter (fun r => decide (stages (.rule r.label) = stage))
  match schemas.mapM (fun r =>
      (Arith.allSubstitutions (outerVars r) domain).mapM (lowerInstance policy domain table stage rows r)) with
  | .error message => .error message
  | .ok batches =>
    let fresh := batches.flatten.flatten
    if fresh.all (fun r => decide (loweredOwner table stages r.head = stage)) then .ok fresh
    else .error "lowering attempted to publish into a different stage"

theorem lowerBatch_owned (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (stage : Nat) (theory : Theory) (fresh : List Rule)
    (returned : lowerBatch policy program domain table stages stage theory = .ok fresh) :
    ∀ r ∈ fresh, loweredOwner table stages r.head = stage := by
  unfold lowerBatch at returned
  dsimp only at returned
  split at returned
  · cases returned
  · split at returned
    next owned =>
      cases returned
      simpa only [List.all_eq_true, decide_eq_true_eq] using owned
    · cases returned

def lowerStages (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat) :
    Nat → Nat → Theory → Except String Theory
  | 0, _, theory => .ok theory
  | count + 1, stage, theory =>
    match lowerBatch policy program domain table stages stage theory with
    | .error message => .error message
    | .ok fresh =>
      let next := { theory with rules := theory.rules ++ fresh }
      if groundScheduled (loweredOwner table stages) next then
        lowerStages policy program domain table stages count (stage + 1) next
      else .error "lowered theory violates its inferred schedule"

/-- Later lowering preserves every rule in an already completed domain, and
preserves priorities. The final theory remains ordinarily scheduled. -/
theorem lowerStages_preserves (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (count stage : Nat) (before after : Theory)
    (scheduled : groundScheduled (loweredOwner table stages) before = true)
    (returned : lowerStages policy program domain table stages count stage before = .ok after) :
    groundScheduled (loweredOwner table stages) after = true ∧
    after.superiority = before.superiority ∧
    ∀ r, loweredOwner table stages r.head < stage → (r ∈ after.rules ↔ r ∈ before.rules) := by
  induction count generalizing stage before with
  | zero =>
    cases returned
    exact ⟨scheduled, rfl, fun _ _ => Iff.rfl⟩
  | succ count ih =>
    simp only [lowerStages] at returned
    split at returned
    · cases returned
    next fresh called =>
      split at returned
      next nextScheduled =>
        obtain ⟨finalScheduled, priorities, same⟩ := ih (stage + 1) _ nextScheduled returned
        refine ⟨finalScheduled, priorities, ?_⟩
        intro r earlier
        rw [same r (by omega)]
        simp only [List.mem_append]
        constructor
        · intro member
          rcases member with old | added
          · exact old
          · have equal := lowerBatch_owned policy program domain table stages stage before fresh called r added
            omega
        · exact Or.inl
      · cases returned

/-- The actual theories used during lowering agree with the final lowered theory
on completed domains. Thus later lowering cannot change lower proof support or
lambda evidence used to compute aggregate inputs. -/
theorem lowerStages_completed_agree (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (count stage : Nat) (before after : Theory)
    (scheduled : groundScheduled (loweredOwner table stages) before = true)
    (returned : lowerStages policy program domain table stages count stage before = .ok after) :
    Agree (fun l => loweredOwner table stages l < stage) (reason before).delta (reason after).delta ∧
    Agree (fun l => loweredOwner table stages l < stage) (reason before).lambda (reason after).lambda ∧
    Agree (fun l => loweredOwner table stages l < stage) (reason before).partial_ (reason after).partial_ := by
  obtain ⟨finalScheduled, priorities, same⟩ :=
    lowerStages_preserves policy program domain table stages count stage before after scheduled returned
  have beforeOrdered := ((groundScheduled_iff _ _).mp scheduled).2.1
  have afterOrdered := ((groundScheduled_iff _ _).mp finalScheduled).2.1
  apply reason_agrees_on_closed_domains
  · intro r hr owned l member
    have bound := beforeOrdered r hr l member
    omega
  · intro r hr owned l member
    have bound := afterOrdered r hr l member
    omega
  · exact fun r hr => (same r hr).symm
  · exact priorities.symm
  · intro l hl
    simpa only [loweredOwner_complement] using hl

/-- Infer a schedule, ground and lower each stage after completing its predecessors.
Unsupported syntax and domain exhaustion are errors, not silently missing rules. -/
def lowerProgram (policy : AggregatePolicy := .defeasibleEvidence)
    (program : AggregateProgram) (domain : Arith.Domain) :
    Except String LoweredProgram :=
  if sourceSupported policy program domain then
    match inferProgram program with
    | .ok stages =>
      let table := atomTable program domain
      let stageCount := (program.rules.map (fun r => stages (.rule r.label))).foldl max 0 + 1
      let initial : Theory := ⟨[], program.priorities.map (fun (a, b) => ("u:" ++ a, "u:" ++ b))⟩
      match lowerStages policy program domain table stages stageCount 0 initial with
      | .ok theory =>
        if groundScheduled (loweredOwner table stages) theory &&
            theory.allLiterals.all (fun l => decide (loweredOwner table stages l < stageCount)) then
          .ok ⟨table, stages, theory, stageCount⟩
        else .error "final lowered theory violates its inferred schedule"
      | .error message => .error message
    | .error .unrecognizedSchemas => .error "unrecognized aggregate schemas"
    | .error .unstratifiable => .error "aggregate dependency cycle"
  else .error "unsupported schema fragment or nonground domain"

/-- Every accepted lowering has ordinary dependencies compatible with its inferred
ownership; this check also covers the inserted closure evidence guards. -/
theorem lowerProgram_scheduled (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram)
    (returned : lowerProgram policy program domain = .ok result) :
    groundScheduled (loweredOwner result.table result.stages) result.theory = true := by
  unfold lowerProgram at returned
  split at returned
  · split at returned
    · dsimp only at returned
      split at returned
      · split at returned
        next accepted =>
          cases returned
          simp only [Bool.and_eq_true] at accepted
          exact accepted.1
        · cases returned
      · cases returned
    · cases returned
    · cases returned
  · cases returned

theorem lowerProgram_covers (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram)
    (returned : lowerProgram policy program domain = .ok result) :
    ∀ l ∈ result.theory.allLiterals, loweredOwner result.table result.stages l < result.stageCount := by
  unfold lowerProgram at returned
  split at returned
  · split at returned
    · dsimp only at returned
      split at returned
      · split at returned
        next accepted =>
          cases returned
          simp only [Bool.and_eq_true] at accepted
          simpa only [List.all_eq_true, decide_eq_true_eq] using accepted.2
        · cases returned
      · cases returned
    · cases returned
    · cases returned
  · cases returned

theorem lowerProgram_inferred (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram)
    (returned : lowerProgram policy program domain = .ok result) :
    inferProgram program = .ok result.stages := by
  unfold lowerProgram at returned
  split at returned
  · split at returned
    next stages inferred =>
      dsimp only at returned
      split at returned
      · split at returned
        · cases returned; exact inferred
        · cases returned
      · cases returned
    · cases returned
    · cases returned
  · cases returned

/-- Once aggregates have been lowered against completed predecessor snapshots,
each scheduled ground prefix agrees with full replay of the final lowered theory. -/
theorem lowerProgram_prefix_equivalent (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram)
    (returned : lowerProgram policy program domain = .ok result) (stage : Nat) :
    reasonedStage (loweredOwner result.table result.stages) result.theory stage =
      (reason result.theory).conclusions.filter
        (fun c => decide (loweredOwner result.table result.stages c.literal = stage)) := by
  have scheduled := (groundScheduled_iff _ _).mp (lowerProgram_scheduled policy program domain result returned)
  exact reasonedStage_equivalent _ _ stage scheduled.2.1 (loweredOwner_complement _ _)


/-- Run the checked driver on the lowered program using the inferred stage count. -/
def evaluateLowered (result : LoweredProgram) : Except ExecutionError StageState :=
  let owner := loweredOwner result.table result.stages
  executeStages owner (groundBackend owner result.theory) result.stageCount

/-- Complete finite-domain entry point: inference, aggregate lowering, execution.
Defaults to defeasible snapshot evidence, including for strict aggregate rules. -/
def evaluateProgram (policy : AggregatePolicy := .defeasibleEvidence)
    (program : AggregateProgram) (domain : Arith.Domain) :
    Except String (LoweredProgram × StageState) :=
  match lowerProgram policy program domain with
  | .error message => .error message
  | .ok result => match evaluateLowered result with
    | .ok state => .ok (result, state)
    | .error (.backend message) => .error message
    | .error (.wrongOwner _) => .error "lowered execution rejected ownership"

private theorem reportLiteral_owned (result : ReasonResult) (l : Literal) (c : Conclusion)
    (member : c ∈ reportLiteral result l) : c.literal = l := by
  unfold reportLiteral at member
  split_ifs at member <;> simp only [List.mem_cons, List.not_mem_nil, or_false] at member
  all_goals rcases member with rfl | rfl <;> rfl

private theorem reported_in_theory (theory : Theory) (c : Conclusion)
    (member : c ∈ (reason theory).conclusions) : c.literal ∈ theory.allLiterals := by
  rw [← reason_reports] at member
  obtain ⟨l, present, reported⟩ := List.mem_flatMap.mp member
  rw [reportLiteral_owned _ l c reported]
  exact present

/-- End-to-end equivalence for successful schema lowering: executing every inferred
stage reports exactly the tags of a full run over the final lowered theory. -/
theorem lowerProgram_execute_equivalent (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram) (after : StageState)
    (lowered : lowerProgram policy program domain = .ok result)
    (executed : evaluateLowered result = .ok after) (c : Conclusion) :
    c ∈ after.conclusions ↔ c ∈ (reason result.theory).conclusions := by
  rw [executeGround_membership _ _ (lowerProgram_scheduled policy program domain result lowered)
    (loweredOwner_complement _ _) result.stageCount after executed c]
  constructor
  · exact And.left
  · intro member
    exact ⟨member, lowerProgram_covers policy program domain result lowered c.literal
      (reported_in_theory result.theory c member)⟩

theorem evaluateProgram_correct (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram) (state : StageState)
    (returned : evaluateProgram policy program domain = .ok (result, state)) :
    lowerProgram policy program domain = .ok result ∧
      ∀ c, c ∈ state.conclusions ↔ c ∈ (reason result.theory).conclusions := by
  unfold evaluateProgram at returned
  split at returned
  · cases returned
  next candidate lowered =>
    split at returned
    next after executed =>
      cases returned
      exact ⟨lowered, lowerProgram_execute_equivalent policy program domain result state lowered executed⟩
    · cases returned
    · cases returned

end Spindle.Aggregation
