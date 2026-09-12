import Spindle.Aggregation.SourceSemantics
import Spindle.Aggregation.Lowering

/-! Refinement of executable lowering by source aggregate satisfaction. -/
namespace Spindle.Aggregation

/-- Successful effectful traversal is pointwise relational evaluation. -/
theorem mapM_ok_iff {α β : Type} (f : α → Except String β) (xs : List α) (ys : List β) :
    xs.mapM f = .ok ys ↔ List.Forall₂ (fun x y => f x = .ok y) xs ys := by
  induction xs generalizing ys with
  | nil => cases ys <;> simp [pure, Except.pure]
  | cons x xs ih =>
    rw [List.mapM_cons]
    cases h : f x with
    | error msg => cases ys <;> simp [bind, Except.bind, h]
    | ok value =>
      cases ht : xs.mapM f with
      | error msg =>
        cases ys <;> simp [bind, Except.bind, ← ih, ht, h]
      | ok values =>
        cases ys <;> simp [bind, Except.bind, pure, Except.pure, ← ih, ht, h]

theorem names_iff (name : String) (r : Reducer Int) :
    reducerNamed name = .ok r ↔ Source.Names name r := by
  constructor
  · intro h
    unfold reducerNamed at h
    split at h
    next first =>
      have names : name = "+" ∨ name = "sum" := by simpa using first
      cases h
      rcases names with rfl | rfl
      · exact .add
      · exact .sum
    · split at h
      next hmin =>
        subst name
        cases h
        exact .min
      · split at h
        next hmax =>
          subst name
          cases h
          exact .max
        · cases h
  · intro h
    cases h <;> rfl

theorem seed_iff (σ : Arith.Substitution) (e : Option Expression) (n : Option Int) :
    e.mapM (evalExpr σ) = .ok n ↔ Source.Seed σ e n := by
  constructor
  · intro h
    cases e with
    | none =>
      change Except.ok none = Except.ok n at h
      cases h
      exact .absent
    | some e =>
      cases he : evalExpr σ e with
      | error msg => simp [Option.mapM, Functor.map, Except.map, he] at h
      | ok value =>
        have eq : some value = n := by
          simpa [Option.mapM, Functor.map, Except.map, he] using h
        subst n
        exact .present he
  · intro h
    cases h with
    | absent => rfl
    | present he => simp [Option.mapM, Functor.map, Except.map, he]

theorem reduces_iff (r : Reducer Int) (seed result : Option Int) (xs : List Int) :
    Source.Reduces r seed xs result ↔ r.eval seed xs = result := by
  constructor
  · intro h
    induction h with
    | empty => rfl
    | first perm _ ih => rw [r.eval_permutation _ perm]; exact ih
    | next perm _ ih => rw [r.eval_permutation _ perm]; exact ih
  · intro h
    subst result
    induction xs generalizing seed with
    | nil => exact .empty seed
    | cons x xs ih =>
      cases seed with
      | none => exact .first (.refl _) (ih (some x))
      | some seed => exact .next (.refl _) (ih (some (r.combine seed x)))

/-- Soundness and completeness of aggregate evaluation against the independent
unordered source judgment. This equivalence also excludes erroneous evaluations. -/
private theorem evalSchemaFold_canonical_iff (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (f : FoldExpression) (result : Option Int) :
    evalSchemaFold domain σ rows f = .ok result ↔
      ∃ r seed values, Source.Names f.reducer r ∧ Source.Seed σ f.seed seed ∧
        List.Forall₂ (fun env n => evalExpr env f.extract = .ok n)
          (rows.dedup.filterMap (matchRow σ f.pattern)) values ∧
        Source.Reduces r seed values result ∧
        (∀ n, result = some n → Arith.Term.integer n ∈ domain) := by
  unfold evalSchemaFold
  simp only [← names_iff, ← seed_iff, ← mapM_ok_iff, reduces_iff]
  cases hr : reducerNamed f.reducer with
  | error msg => simp [bind, Except.bind]
  | ok r =>
    cases hs : f.seed.mapM (evalExpr σ) with
    | error msg => simp [bind, Except.bind]
    | ok seed =>
      cases hv : (rows.dedup.filterMap (matchRow σ f.pattern)).mapM
          (fun env => evalExpr env f.extract) with
      | error msg => simp [bind, Except.bind]
      | ok values =>
        simp only [bind, Except.bind, pure, Except.pure, Except.ok.injEq]
        cases he : r.eval seed values with
        | none => cases result <;> simp [he]
        | some n =>
          by_cases member : Arith.Term.integer n ∈ domain
          · cases result <;> simp_all [List.any_eq_true]
            intro equal; subst n; assumption
          · cases result <;> simp_all [List.any_eq_true]
            intro equal; subst n; assumption

/-- Executable aggregate lowering is equivalent to the source judgment over an
arbitrary enumeration of the row set and arbitrary reduction order. -/
theorem evalSchemaFold_iff (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (f : FoldExpression) (result : Option Int) :
    evalSchemaFold domain σ rows f = .ok result ↔ Source.Fold domain σ rows f result := by
  rw [evalSchemaFold_canonical_iff]
  constructor
  · rintro ⟨r, seed, values, name, initial, extracts, reduced, bounded⟩
    exact ⟨r, seed, values, name, initial,
      ⟨rows.dedup, List.nodup_dedup rows, fun _ => List.mem_dedup, extracts⟩, reduced, bounded⟩
  · rintro ⟨r, seed, values, name, initial, ⟨relation, unique, covers, extracts⟩, reduced, bounded⟩
    have rowsPerm : rows.dedup.Perm relation :=
      (List.perm_ext_iff_of_nodup (List.nodup_dedup rows) unique).mpr
        (fun row => List.mem_dedup.trans (covers row).symm)
    obtain ⟨canonical, hc, perm⟩ := List.perm_comp_forall₂
      (rowsPerm.filterMap (matchRow σ f.pattern)) extracts
    refine ⟨r, seed, canonical, name, initial, hc, ?_, bounded⟩
    apply (reduces_iff r seed result canonical).mpr
    rw [r.eval_permutation seed perm]
    exact (reduces_iff r seed result values).mp reduced

/-- Static checks implement source satisfaction; logical atoms stay residual. -/
theorem checkCondition_iff (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (c : Condition) (value : Bool) :
    checkCondition domain σ rows c = .ok value ↔ Source.ConditionValue domain σ rows c value := by
  constructor
  · intro h
    cases c with
    | logic p =>
      change Except.ok true = Except.ok value at h
      cases h
      exact .logic p
    | bind v e =>
      cases he : evalExpr σ e with
      | error msg => simp [checkCondition, he, bind, Except.bind] at h
      | ok n =>
        have equal : decide (σ.lookup v = some (.integer n)) = value := by
          simpa [checkCondition, he, bind, Except.bind, pure, Except.pure] using h
        subst value
        exact .bind he
    | compare op a b =>
      cases ha : evalExpr σ a with
      | error msg => simp [checkCondition, ha, bind, Except.bind] at h
      | ok x =>
        cases hb : evalExpr σ b with
        | error msg => simp [checkCondition, ha, hb, bind, Except.bind] at h
        | ok y =>
          have equal : compareInts op x y = value := by
            simpa [checkCondition, ha, hb, bind, Except.bind, pure, Except.pure] using h
          subst value
          exact .compare ha hb
    | fold f =>
      cases hf : evalSchemaFold domain σ rows f with
      | error msg => simp [checkCondition, hf, bind, Except.bind] at h
      | ok result =>
        have meaning := (evalSchemaFold_iff domain σ rows f result).mp hf
        cases result with
        | none =>
          have equal : false = value := by
            simpa [checkCondition, hf, bind, Except.bind, pure, Except.pure] using h
          subst value
          exact .emptyFold meaning
        | some n =>
          have equal : decide (σ.lookup f.resultVar = some (.integer n)) = value := by
            simpa [checkCondition, hf, bind, Except.bind, pure, Except.pure] using h
          subst value
          exact .fold meaning
  · intro h
    cases h with
    | logic => rfl
    | bind he => simp [checkCondition, he, bind, Except.bind, pure, Except.pure]
    | compare ha hb => simp [checkCondition, ha, hb, bind, Except.bind, pure, Except.pure]
    | emptyFold hf => simp [checkCondition, ← evalSchemaFold_iff] at hf ⊢; rw [hf]; rfl
    | fold hf => simp [checkCondition, ← evalSchemaFold_iff] at hf ⊢; rw [hf]; rfl

private theorem checks_all_iff (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (cs : List Condition) (checks : List Bool)
    (returned : cs.mapM (checkCondition domain σ rows) = .ok checks) :
    checks.all id = true ↔ ∀ c ∈ cs, Source.ConditionValue domain σ rows c true := by
  have related := (mapM_ok_iff _ _ _).mp returned
  clear returned
  induction related with
  | nil => simp
  | @cons c value cs checks h _ ih =>
    simp only [List.all_cons, Bool.and_eq_true, id_eq, List.mem_cons, forall_eq_or_imp, ← ih]
    have eq : value = true ↔ Source.ConditionValue domain σ rows c true := by
      rw [← checkCondition_iff, h, Except.ok.injEq]
    rw [eq]

theorem hasFold_iff (r : SchemaRule) : hasFold r = true ↔ Source.HasAggregate r := by
  simp only [hasFold, Source.HasAggregate, List.any_eq_true]
  constructor
  · rintro ⟨c, hc, yes⟩
    cases c <;> simp only [Bool.false_eq_true] at yes
    case fold f => exact ⟨f, hc⟩
  · rintro ⟨f, hf⟩
    exact ⟨.fold f, hf, rfl⟩

/-- Grounding checks validate the encoding; they do not establish source truth. -/
theorem checkedAtom_iff (table : List Pattern) (σ : Arith.Substitution) (p : Pattern)
    (l : Literal) : checkedAtom table σ p = .ok l ↔
      (p.applySubst σ).atom.isGround = true ∧ base (p.applySubst σ) ∈ table ∧
      encodeAtom table (p.applySubst σ) = l := by
  unfold checkedAtom
  dsimp only
  split <;> simp_all

private theorem checkedAtoms_eq (table : List Pattern) (σ : Arith.Substitution)
    (ps : List Pattern) (ls : List Literal)
    (h : ps.mapM (checkedAtom table σ) = .ok ls) :
    ls = ps.map (fun p => encodeAtom table (p.applySubst σ)) := by
  have related := (mapM_ok_iff _ _ _).mp h
  clear h
  induction related with
  | nil => rfl
  | cons h _ ih => simp only [List.map_cons, ih, (checkedAtom_iff _ _ _ _).mp h |>.2.2]

/-- A source clause's representation under the explicit closure-evidence policy. -/
def encodeClause (table : List Pattern) (stage : Nat) (c : Source.Clause) : Rule :=
  ⟨"u:" ++ c.label, c.kind,
    c.body.map (encodeAtom table) ++ (if c.closure then [closureGuard stage] else []),
    encodeAtom table c.head⟩

private theorem lowerInstance_cases (policy : AggregatePolicy) (domain : Arith.Domain)
    (table : List Pattern) (stage : Nat) (rows : List Pattern) (r : SchemaRule)
    (σ : Arith.Substitution) (fresh : List Rule)
    (returned : lowerInstance policy domain table stage rows r σ = .ok fresh) :
    (Source.Enabled domain σ rows r ∧ fresh = emitLoweredRules r stage
      (r.heads.map fun p => encodeAtom table (p.applySubst σ))
      (r.premises.map
        fun p => encodeAtom table (p.applySubst σ))) ∨
    (¬Source.Enabled domain σ rows r ∧ fresh = []) := by
  unfold lowerInstance at returned
  split at returned
  · cases returned
  · cases hc : r.body.mapM (checkCondition domain σ rows) with
    | error msg => simp [hc, bind, Except.bind, pure, Except.pure] at returned
    | ok checks =>
      simp only [hc, bind, Except.bind, pure, Except.pure] at returned
      split at returned
      next no =>
        right
        have disabled : ¬Source.Enabled domain σ rows r := by
          intro enabled
          have all := (checks_all_iff domain σ rows r.body checks hc).mpr enabled
          simp [all] at no
        exact ⟨disabled, (Except.ok.inj returned).symm⟩
      next yes =>
        left
        have enabled : Source.Enabled domain σ rows r := by
          apply (checks_all_iff domain σ rows r.body checks hc).mp
          simpa using yes
        refine ⟨enabled, ?_⟩
        cases hh : r.heads.mapM (checkedAtom table σ) with
        | error msg => simp [hh] at returned
        | ok heads =>
          cases hb : r.premises.mapM
              (checkedAtom table σ) with
          | error msg => simp only [hh, hb, reduceCtorEq] at returned
          | ok body =>
            have equal : emitLoweredRules r stage heads body = fresh := by
              simpa only [hh, hb, bind, Except.bind, pure, Except.pure, Except.ok.injEq] using returned
            rw [← equal, checkedAtoms_eq _ _ _ _ hh, checkedAtoms_eq _ _ _ _ hb]

/-- Exactly the rules justified by a source instance, including its administrative
closure witness. The witness is never an encoded user atom. -/
def SourceRule (domain : Arith.Domain) (table : List Pattern) (stage : Nat)
    (rows : List Pattern) (r : SchemaRule) (σ : Arith.Substitution) (rule : Rule) : Prop :=
  (∃ c, Source.InstanceClause domain σ rows r c ∧ encodeClause table stage c = rule) ∨
  (Source.Enabled domain σ rows r ∧ Source.HasAggregate r ∧
    rule = Rule.defeasible ("g:" ++ toString stage) [] (closureGuard stage))

private theorem encoded_source_heads (domain : Arith.Domain) (table : List Pattern)
    (stage : Nat) (rows : List Pattern) (r : SchemaRule) (σ : Arith.Substitution) (rule : Rule) :
    (∃ c, Source.InstanceClause domain σ rows r c ∧ encodeClause table stage c = rule) ↔
    Source.Enabled domain σ rows r ∧ ∃ p ∈ r.heads,
      (⟨"u:" ++ r.label, r.kind,
        (r.premises.map
          fun p => encodeAtom table (p.applySubst σ)) ++
        (if hasFold r then [closureGuard stage] else []),
        encodeAtom table (p.applySubst σ)⟩ : Rule) = rule := by
  constructor
  · rintro ⟨c, ⟨enabled, p, hp, label, kind, head, body, closure⟩, eq⟩
    have hc : c.closure = hasFold r := Bool.eq_iff_iff.mpr (closure.trans (hasFold_iff r).symm)
    refine ⟨enabled, p, hp, ?_⟩
    simpa only [encodeClause, label, kind, head, body, hc, List.map_map] using eq
  · rintro ⟨enabled, p, hp, equal⟩
    refine ⟨⟨r.label, r.kind, r.premises.map (Pattern.applySubst σ), p.applySubst σ, hasFold r⟩, ?_, ?_⟩
    · exact ⟨enabled, p, hp, rfl, rfl, rfl, rfl, hasFold_iff r⟩
    · simpa only [encodeClause, List.map_map] using equal

/-- Lowering soundness and completeness: after successful checking, a ground rule
is emitted iff independently justified by source aggregate satisfaction. -/
theorem lowerInstance_correct (policy : AggregatePolicy) (domain : Arith.Domain)
    (table : List Pattern) (stage : Nat) (rows : List Pattern) (r : SchemaRule)
    (σ : Arith.Substitution) (fresh : List Rule)
    (returned : lowerInstance policy domain table stage rows r σ = .ok fresh) (rule : Rule) :
    rule ∈ fresh ↔ SourceRule domain table stage rows r σ rule := by
  rw [SourceRule, encoded_source_heads]
  rcases lowerInstance_cases policy domain table stage rows r σ fresh returned with
    ⟨enabled, rfl⟩ | ⟨disabled, rfl⟩
  · rw [← hasFold_iff]
    cases aggregate : hasFold r <;>
      simp [emitLoweredRules, aggregate, enabled, List.mem_map, eq_comm (a := rule), or_comm]
  · simp [disabled]

/-- Defined source instances cannot disappear through compiler errors when their
atoms are covered and the evidence policy permits the source rule. -/
theorem lowerInstance_complete (policy : AggregatePolicy) (domain : Arith.Domain)
    (table : List Pattern) (stage : Nat) (rows : List Pattern) (r : SchemaRule)
    (σ : Arith.Substitution)
    (enabled : Source.Enabled domain σ rows r)
    (allowed : ¬(policy = .rejectStrict ∧ r.kind = .strict ∧ Source.HasAggregate r))
    (covered : ∀ p ∈ r.heads ++ r.premises,
      (p.applySubst σ).atom.isGround = true ∧ base (p.applySubst σ) ∈ table) :
    ∃ fresh, lowerInstance policy domain table stage rows r σ = .ok fresh ∧
      ∀ rule, rule ∈ fresh ↔ SourceRule domain table stage rows r σ rule := by
  have checks : r.body.mapM (checkCondition domain σ rows) = .ok (r.body.map fun _ => true) := by
    rw [mapM_ok_iff, List.forall₂_map_right_iff, List.forall₂_same]
    exact fun c hc => (checkCondition_iff domain σ rows c true).mpr (enabled c hc)
  have atoms (ps : List Pattern) (included : ∀ p ∈ ps, p ∈ r.heads ++ r.premises) :
      ps.mapM (checkedAtom table σ) = .ok (ps.map fun p => encodeAtom table (p.applySubst σ)) := by
    rw [mapM_ok_iff, List.forall₂_map_right_iff, List.forall₂_same]
    intro p hp
    exact (checkedAtom_iff table σ p _).mpr ⟨(covered p (included p hp)).1,
      (covered p (included p hp)).2, rfl⟩
  have heads := atoms r.heads (fun _ hp => List.mem_append_left _ hp)
  have body := atoms r.premises (fun _ hp => List.mem_append_right _ hp)
  have permitted : ¬(policy = .rejectStrict ∧ r.kind = .strict ∧ hasFold r = true) := by
    simpa only [hasFold_iff] using allowed
  have all : (r.body.map fun _ => true).all id = true := by simp
  have returned : lowerInstance policy domain table stage rows r σ =
      .ok (emitLoweredRules r stage (r.heads.map fun p => encodeAtom table (p.applySubst σ))
        (r.premises.map fun p => encodeAtom table (p.applySubst σ))) := by
    simp only [lowerInstance, if_neg permitted, bind, Except.bind, pure, Except.pure, checks]
    simp only [all, Bool.not_true, Bool.false_eq_true, if_false]
    rw [heads]
    rw [body]
  exact ⟨_, returned, lowerInstance_correct policy domain table stage rows r σ _ returned⟩

/-- Relational aggregate values are unique, despite free row and reduction order. -/
theorem sourceFold_deterministic (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (f : FoldExpression) (a b : Option Int)
    (ha : Source.Fold domain σ rows f a) (hb : Source.Fold domain σ rows f b) : a = b := by
  have left := (evalSchemaFold_iff domain σ rows f a).mpr ha
  have right := (evalSchemaFold_iff domain σ rows f b).mpr hb
  exact Except.ok.inj (left.symm.trans right)

/-- The executable Cartesian product enumerates exactly finite source assignments. -/
theorem assignment_iff (domain : Arith.Domain) (vars : List String) (σ : Arith.Substitution) :
    Source.Assignment domain vars σ ↔ σ ∈ Arith.allSubstitutions vars domain := by
  constructor
  · intro h
    induction h with
    | empty => simp [Arith.allSubstitutions]
    | assign v t member _ ih =>
      simp only [Arith.allSubstitutions, List.mem_flatMap, List.mem_map]
      exact ⟨t, member, _, ih, rfl⟩
  · induction vars generalizing σ with
    | nil => intro h; simp only [Arith.allSubstitutions, List.mem_singleton] at h; subst σ; exact .empty
    | cons v vars ih =>
      simp only [Arith.allSubstitutions, List.mem_flatMap, List.mem_map]
      rintro ⟨t, ht, env, he, rfl⟩
      exact .assign v t ht (ih env he)

private theorem mapM_mem_iff {α β : Type} (f : α → Except String β) (xs : List α) (ys : List β)
    (h : xs.mapM f = .ok ys) (y : β) : y ∈ ys ↔ ∃ x ∈ xs, f x = .ok y := by
  have related := (mapM_ok_iff _ _ _).mp h
  clear h
  induction related with
  | nil => simp
  | @cons x value xs ys hx _ ih => simp [List.mem_cons, ih, hx, eq_comm (a := y)]

private theorem mapM_member {α β : Type} (f : α → Except String β) (xs : List α) (ys : List β)
    (h : xs.mapM f = .ok ys) (x : α) (member : x ∈ xs) : ∃ y ∈ ys, f x = .ok y := by
  have related := (mapM_ok_iff _ _ _).mp h
  clear h
  induction related with
  | nil => simp at member
  | @cons a b xs ys hab _ ih =>
    rcases List.mem_cons.mp member with rfl | tail
    · exact ⟨b, by simp, hab⟩
    · obtain ⟨out, hout, he⟩ := ih tail
      exact ⟨out, List.mem_cons_of_mem _ hout, he⟩

/-- Source rule membership for one stratum, interpreted over a completed predecessor
snapshot. Neither this judgment nor `SourceRule` runs aggregate lowering. -/
def SourceBatch (program : AggregateProgram) (domain : Arith.Domain) (table : List Pattern)
    (stages : DependencyNode → Nat) (stage : Nat) (before : Theory) (rule : Rule) : Prop :=
  ∃ r ∈ program.rules, stages (.rule r.label) = stage ∧
    ∃ σ, Source.Assignment domain (outerVars r) σ ∧
      SourceRule domain table stage (survivingRows table (reason before).conclusions) r σ rule

/-- Every emitted rule has a source justification, and every source-justified rule
at this stage is emitted. Includes every finite outer assignment and every head. -/
theorem lowerBatch_correct (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (stage : Nat) (before : Theory) (fresh : List Rule)
    (returned : lowerBatch policy program domain table stages stage before = .ok fresh) (rule : Rule) :
    rule ∈ fresh ↔ SourceBatch program domain table stages stage before rule := by
  unfold lowerBatch at returned
  dsimp only at returned
  split at returned
  · cases returned
  next batches evaluated =>
    split at returned
    · cases returned
      simp only [List.mem_flatten]
      constructor
      · rintro ⟨batch, ⟨instances, hi, hb⟩, member⟩
        obtain ⟨r, hr, hinstances⟩ := (mapM_mem_iff _ _ _ evaluated instances).mp hi
        obtain ⟨σ, hσ, hbatch⟩ := (mapM_mem_iff _ _ _ hinstances batch).mp hb
        have source := (lowerInstance_correct policy domain table stage _ r σ batch hbatch rule).mp member
        simp only [List.mem_filter, decide_eq_true_eq] at hr
        exact ⟨r, hr.1, hr.2, σ, (assignment_iff _ _ _).mpr hσ, source⟩
      · rintro ⟨r, hr, stageEq, σ, hσ, source⟩
        have rmem : r ∈ program.rules.filter (fun r => decide (stages (.rule r.label) = stage)) := by
          simp only [List.mem_filter, decide_eq_true_eq]; exact ⟨hr, stageEq⟩
        obtain ⟨instances, hi, hinstances⟩ := mapM_member _ _ _ evaluated r rmem
        obtain ⟨batch, hb, hbatch⟩ := mapM_member _ _ _ hinstances σ ((assignment_iff _ _ _).mp hσ)
        exact ⟨batch, ⟨instances, hi, hb⟩,
          (lowerInstance_correct policy domain table stage _ r σ batch hbatch rule).mpr source⟩
    · cases returned

/-- Declarative staged construction. Each stage adds exactly its justified source
instances against the completed predecessor theory. Priorities stay unchanged.
This relation contains no call to `lowerBatch`, `lowerStages`, or `lowerProgram`. -/
inductive SourceStages (program : AggregateProgram) (domain : Arith.Domain)
    (table : List Pattern) (stages : DependencyNode → Nat) : Nat → Nat → Theory → Theory → Prop
  | done (stage before) : SourceStages program domain table stages 0 stage before before
  | step {count stage before after} (fresh : List Rule) :
      (∀ rule, rule ∈ fresh ↔ SourceBatch program domain table stages stage before rule) →
      SourceStages program domain table stages count (stage + 1)
        { before with rules := before.rules ++ fresh } after →
      SourceStages program domain table stages (count + 1) stage before after

/-- Successful lowering constructs a theory satisfying the source stage relation. -/
theorem lowerStages_correct (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (table : List Pattern) (stages : DependencyNode → Nat)
    (count stage : Nat) (before after : Theory)
    (returned : lowerStages policy program domain table stages count stage before = .ok after) :
    SourceStages program domain table stages count stage before after := by
  induction count generalizing stage before with
  | zero => cases returned; exact .done stage _
  | succ count ih =>
    simp only [lowerStages] at returned
    split at returned
    · cases returned
    next fresh called =>
      split at returned
      · exact .step fresh (lowerBatch_correct policy program domain table stages stage before fresh called)
          (ih _ _ returned)
      · cases returned

/-- The program entry point refines independently specified aggregate satisfaction,
with the inferred schedule and original priorities. -/
theorem lowerProgram_source_correct (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram)
    (returned : lowerProgram policy program domain = .ok result) :
    inferProgram program = .ok result.stages ∧
    SourceStages program domain result.table result.stages result.stageCount 0
      ⟨[], program.priorities.map (fun (a, b) => ("u:" ++ a, "u:" ++ b))⟩ result.theory := by
  refine ⟨lowerProgram_inferred policy program domain result returned, ?_⟩
  unfold lowerProgram at returned
  split at returned
  · split at returned
    · dsimp only at returned
      split at returned
      next theory lowered =>
        split at returned
        · cases returned
          exact lowerStages_correct policy program domain _ _ _ _ _ _ lowered
        · cases returned
      · cases returned
    · cases returned
    · cases returned
  · cases returned

/-- Composition with scheduled evaluation: the constructed theory satisfies source
aggregate semantics and all four reported SDL tags agree with full replay. -/
theorem evaluateProgram_source_correct (policy : AggregatePolicy) (program : AggregateProgram)
    (domain : Arith.Domain) (result : LoweredProgram) (state : StageState)
    (returned : evaluateProgram policy program domain = .ok (result, state)) :
    SourceStages program domain result.table result.stages result.stageCount 0
      ⟨[], program.priorities.map (fun (a, b) => ("u:" ++ a, "u:" ++ b))⟩ result.theory ∧
    (∀ c, c ∈ state.conclusions ↔ c ∈ (reason result.theory).conclusions) := by
  obtain ⟨lowered, agrees⟩ := evaluateProgram_correct policy program domain result state returned
  exact ⟨(lowerProgram_source_correct policy program domain result lowered).2, agrees⟩

end Spindle.Aggregation
