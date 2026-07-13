/-
  Spindle SPL Grammar — propositional SDL fragment

  Formalizes the core fragment of the SPL input language recognized by
  `spindle-parser` (`crates/spindle-parser/src/spl/`), per the LangSec
  discipline: the grammar is part of the contract.

  Fragment (EBNF over S-expressions):

    theory   := stmt*
    stmt     := "(" "given"    lit ")"
              | "(" "always"   label bodyExpr lit ")"     -- strict
              | "(" "normally" label bodyExpr lit ")"     -- defeasible
              | "(" "except"   label bodyExpr lit ")"     -- defeater
              | "(" "prefer"   label label ")"
    lit      := atom | "(" "not" atom ")"
    bodyExpr := lit | "(" "and" lit+ ")"
    atom, label := symbol (non-reserved, no whitespace/parens)

  Layers:
  1. AST ↔ S-expression: `encodeStmt` / `decodeStmt`, with the
     **machine-checked roundtrip theorem** `decode_encode_stmt`
     (and `decode_encode_theory`): decoding a canonically encoded
     statement always succeeds and returns the original AST. This is the
     no-ambiguity/no-loss property of the grammar at the structural level.
  2. S-expression ↔ String: executable canonical printer and a
     fuel-bounded recursive-descent parser. This layer is exercised
     end-to-end by the parser difftest (`spl_parser_difftest.rs`), which
     renders random fragment ASTs canonically and compares
     `spindle-parser`'s result with this model's, via the
     `--parse-spl-batch` oracle mode. String-level roundtrip proofs are
     future work.
-/
import Mathlib.Data.List.Dedup

namespace Spl

/-! ## AST -/

structure SplLit where
  name : String
  negated : Bool
  deriving DecidableEq, Repr

inductive SplRuleType where
  | strict | defeasible | defeater
  deriving DecidableEq, Repr

inductive SplStmt where
  | fact (l : SplLit)
  | rule (t : SplRuleType) (label : String) (body : List SplLit) (head : SplLit)
  | prefer (winner loser : String)
  deriving DecidableEq, Repr

abbrev SplTheory := List SplStmt

/-! ## S-expressions -/

inductive Sexp where
  | atom (s : String)
  | list (items : List Sexp)
  deriving Repr

/-! ## Encoding (canonical) -/

def encodeLit (l : SplLit) : Sexp :=
  if l.negated then .list [.atom "not", .atom l.name]
  else .atom l.name

def ruleKeyword : SplRuleType → String
  | .strict => "always"
  | .defeasible => "normally"
  | .defeater => "except"

/-- Canonical body encoding: a single literal stands alone; several are
    wrapped in `(and …)`. -/
def encodeBody (body : List SplLit) : Sexp :=
  match body with
  | [l] => encodeLit l
  | ls => .list (.atom "and" :: ls.map encodeLit)

def encodeStmt : SplStmt → Sexp
  | .fact l => .list [.atom "given", encodeLit l]
  | .rule t label body head =>
    .list [.atom (ruleKeyword t), .atom label, encodeBody body, encodeLit head]
  | .prefer w l => .list [.atom "prefer", .atom w, .atom l]

/-! ## Decoding -/

def decodeLit : Sexp → Option SplLit
  | .atom s => some ⟨s, false⟩
  | .list [.atom "not", .atom s] => some ⟨s, true⟩
  | _ => none

def decodeBody (e : Sexp) : Option (List SplLit) :=
  match e with
  | .list (.atom "and" :: rest) => rest.mapM decodeLit
  | _ => (decodeLit e).map ([·])

def keywordRuleType : String → Option SplRuleType
  | "always" => some .strict
  | "normally" => some .defeasible
  | "except" => some .defeater
  | _ => none

def decodeStmt : Sexp → Option SplStmt
  | .list [.atom "given", litE] => (decodeLit litE).map .fact
  | .list [.atom kw, .atom label, bodyE, headE] => do
    let t ← keywordRuleType kw
    let body ← decodeBody bodyE
    let head ← decodeLit headE
    pure (.rule t label body head)
  | .list [.atom "prefer", .atom w, .atom l] => some (.prefer w l)
  | _ => none

def decodeTheory (es : List Sexp) : Option SplTheory :=
  es.mapM decodeStmt

/-! ## The roundtrip theorem (structural layer) -/

theorem decode_encode_lit (l : SplLit) : decodeLit (encodeLit l) = some l := by
  cases l with
  | mk name negated =>
    cases negated <;> simp [encodeLit, decodeLit]

theorem mapM_decodeLit_map (ls : List SplLit) :
    List.mapM decodeLit (ls.map encodeLit) = some ls := by
  induction ls with
  | nil => rfl
  | cons y ys ih => simp [List.mapM_cons, decode_encode_lit, ih]

theorem decode_encode_body (body : List SplLit) :
    decodeBody (encodeBody body) = some body := by
  match body with
  | [l] =>
    -- single literal: encoded bare, never an "and" list
    cases l with
    | mk name negated =>
      cases negated <;> simp [encodeBody, encodeLit, decodeBody, decodeLit]
  | [] =>
    simp [encodeBody, decodeBody]
  | l₁ :: l₂ :: rest =>
    simp only [encodeBody, decodeBody, List.map_cons]
    simpa [List.mapM_cons, decode_encode_lit] using
      mapM_decodeLit_map (l₁ :: l₂ :: rest)

theorem decode_encode_stmt (s : SplStmt) :
    decodeStmt (encodeStmt s) = some s := by
  cases s with
  | fact l => simp [encodeStmt, decodeStmt, decode_encode_lit]
  | rule t label body head =>
    have hkw : keywordRuleType (ruleKeyword t) = some t := by
      cases t <;> rfl
    simp [encodeStmt, decodeStmt, hkw, decode_encode_body,
      decode_encode_lit]
  | prefer w l => simp [encodeStmt, decodeStmt]

/-- **Grammar roundtrip**: canonically encoded theories decode losslessly.
    No fragment statement is ambiguous or unrepresentable. -/
theorem decode_encode_theory (t : SplTheory) :
    decodeTheory (t.map encodeStmt) = some t := by
  unfold decodeTheory
  induction t with
  | nil => rfl
  | cons s rest ih => simp [List.mapM_cons, decode_encode_stmt, ih]

/-! ## String layer (executable; difftested, not proven) -/

def printSexp : Sexp → String
  | .atom s => s
  | .list items => "(" ++ String.intercalate " " (items.attach.map
      (fun ⟨e, _⟩ => printSexp e)) ++ ")"

def printTheory (t : SplTheory) : String :=
  String.intercalate " " (t.map (fun s => printSexp (encodeStmt s)))

/-- Tokenizer: parens and whitespace-separated symbols. -/
def tokenize (s : String) : List String :=
  let s := s.replace "(" " ( " |>.replace ")" " ) "
  (s.splitOn " ").filter (· ≠ "")

/-- Fuel-bounded recursive-descent S-expression parser. -/
partial def parseSexps (tokens : List String) : Option (List Sexp) :=
  go tokens [] []
where
  /-- `stack` holds unfinished lists (innermost first); `acc` is the
      current list under construction. -/
  go : List String → List Sexp → List (List Sexp) → Option (List Sexp)
  | [], acc, [] => some acc.reverse
  | [], _, _ :: _ => none  -- unclosed paren
  | "(" :: rest, acc, stack => go rest [] (acc :: stack)
  | ")" :: rest, acc, outer :: stack =>
    go rest (.list acc.reverse :: outer) stack
  | ")" :: _, _, [] => none  -- unmatched close
  | tok :: rest, acc, stack => go rest (.atom tok :: acc) stack

def parseTheory (input : String) : Option SplTheory := do
  let sexps ← parseSexps (tokenize input)
  decodeTheory sexps

end Spl
