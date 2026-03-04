//! Extension function registry for callable functions in `bind` expressions.
//!
//! Provides a principled mechanism for registering external pure functions
//! that can be called from arithmetic expressions during grounding.
//!
//! # Architecture
//!
//! - [`ExtensionFunction`] — trait for pure, deterministic functions
//! - [`FunctionRegistry`] — maps names to function implementations
//! - [`FunctionSignature`] — metadata (name, arity, description)
//! - [`EvalContext`] — shared context threaded through evaluation

use std::collections::HashMap;
use std::fmt;

use crate::intern::{SymbolId, resolve};
use crate::term::Term;

// ---------------------------------------------------------------------------
// Arity
// ---------------------------------------------------------------------------

/// Function arity specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arity {
    /// Exactly N arguments.
    Fixed(usize),
    /// Between min and max arguments (inclusive).
    Range(usize, usize),
}

impl Arity {
    /// Check whether a given argument count satisfies this arity.
    pub fn accepts(&self, n: usize) -> bool {
        match self {
            Arity::Fixed(expected) => n == *expected,
            Arity::Range(min, max) => n >= *min && n <= *max,
        }
    }
}

impl fmt::Display for Arity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arity::Fixed(n) => write!(f, "{n}"),
            Arity::Range(min, max) => write!(f, "{min}..{max}"),
        }
    }
}

// ---------------------------------------------------------------------------
// FunctionSignature
// ---------------------------------------------------------------------------

/// Metadata about an extension function, declared upfront.
#[derive(Clone, Debug)]
pub struct FunctionSignature {
    /// The interned function name.
    pub name: SymbolId,
    /// Accepted arity.
    pub arity: Arity,
    /// Human-readable description for diagnostics.
    pub description: &'static str,
}

// ---------------------------------------------------------------------------
// EvalError
// ---------------------------------------------------------------------------

/// Error from extension function evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// Argument has wrong type.
    TypeError(String),
    /// Evaluation failed for a domain-specific reason.
    EvalFailed(String),
    /// Wraps an [`ArithError`](crate::arith::ArithError) from builtin arithmetic functions
    /// so the original error variant is preserved through the Call dispatch path.
    ArithError(crate::arith::ArithError),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::TypeError(msg) => write!(f, "type error: {msg}"),
            EvalError::EvalFailed(msg) => write!(f, "eval failed: {msg}"),
            EvalError::ArithError(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<crate::arith::ArithError> for EvalError {
    fn from(e: crate::arith::ArithError) -> Self {
        EvalError::ArithError(e)
    }
}

// ---------------------------------------------------------------------------
// ExtensionFunction trait
// ---------------------------------------------------------------------------

/// A pure, deterministic function callable from `bind` expressions.
///
/// Implementations must be:
/// - **Pure**: no side effects, no I/O
/// - **Deterministic**: same inputs always produce the same output
/// - **Send + Sync**: safe to share across threads
pub trait ExtensionFunction: Send + Sync {
    /// The function's signature (name, arity, description).
    fn signature(&self) -> &FunctionSignature;

    /// Evaluate the function with the given arguments.
    ///
    /// Arguments are fully evaluated [`Term`] values. The function should
    /// return a single [`Term`] result, or an [`EvalError`] on failure.
    fn eval(&self, args: &[Term]) -> Result<Term, EvalError>;
}

// ---------------------------------------------------------------------------
// FunctionRegistry
// ---------------------------------------------------------------------------

/// Registry of named extension functions.
///
/// Maps interned function names to their implementations. Used by the
/// validation stage to check function existence and arity, and by the
/// evaluation stage to dispatch calls.
pub struct FunctionRegistry {
    functions: HashMap<SymbolId, Box<dyn ExtensionFunction>>,
}

impl FunctionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Register an extension function.
    ///
    /// If a function with the same name is already registered, it is replaced.
    pub fn register(&mut self, f: Box<dyn ExtensionFunction>) {
        let name = f.signature().name;
        self.functions.insert(name, f);
    }

    /// Create a registry pre-loaded with all built-in arithmetic functions.
    pub fn with_prelude() -> Self {
        let mut reg = Self::new();
        crate::builtins::register_builtins(&mut reg);
        reg
    }

    /// Merge another registry into this one. Entries from `other` override
    /// existing entries with the same name.
    pub fn merge(&mut self, other: FunctionRegistry) {
        for (name, func) in other.functions {
            self.functions.insert(name, func);
        }
    }

    /// Look up a function by its interned name.
    pub fn get(&self, name: SymbolId) -> Option<&dyn ExtensionFunction> {
        self.functions.get(&name).map(|b| b.as_ref())
    }

    /// Check whether a function with the given name is registered.
    pub fn contains(&self, name: SymbolId) -> bool {
        self.functions.contains_key(&name)
    }

    /// Iterate over all registered function names.
    pub fn names(&self) -> impl Iterator<Item = SymbolId> + '_ {
        self.functions.keys().copied()
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.functions.keys().map(|id| resolve(*id)).collect();
        f.debug_struct("FunctionRegistry")
            .field("functions", &names)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EvalContext
// ---------------------------------------------------------------------------

/// Shared context threaded through evaluation and grounding.
///
/// Bundles the function registry (and future shared state) into a single
/// struct so that only one extra parameter needs to be threaded through
/// the grounding call chain.
pub struct EvalContext<'a> {
    /// Optional function registry for extension function dispatch.
    pub registry: Option<&'a FunctionRegistry>,
}

impl<'a> EvalContext<'a> {
    /// Create an empty context (no registry).
    ///
    /// Equivalent to current behaviour — programs without extension
    /// functions behave identically.
    pub fn empty() -> Self {
        Self { registry: None }
    }

    /// Create a context with a function registry.
    pub fn with_registry(registry: &'a FunctionRegistry) -> Self {
        Self {
            registry: Some(registry),
        }
    }
}

impl fmt::Debug for EvalContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvalContext")
            .field("has_registry", &self.registry.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;

    /// A simple test function that doubles an integer.
    struct DoubleFunction {
        sig: FunctionSignature,
    }

    impl DoubleFunction {
        fn new() -> Self {
            Self {
                sig: FunctionSignature {
                    name: intern("double"),
                    arity: Arity::Fixed(1),
                    description: "doubles an integer",
                },
            }
        }
    }

    impl ExtensionFunction for DoubleFunction {
        fn signature(&self) -> &FunctionSignature {
            &self.sig
        }

        fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
            match &args[0] {
                Term::Integer(n) => Ok(Term::Integer(n * 2)),
                other => Err(EvalError::TypeError(format!(
                    "expected integer, got {other:?}"
                ))),
            }
        }
    }

    #[test]
    fn arity_fixed_accepts() {
        let a = Arity::Fixed(2);
        assert!(!a.accepts(0));
        assert!(!a.accepts(1));
        assert!(a.accepts(2));
        assert!(!a.accepts(3));
    }

    #[test]
    fn arity_range_accepts() {
        let a = Arity::Range(1, 3);
        assert!(!a.accepts(0));
        assert!(a.accepts(1));
        assert!(a.accepts(2));
        assert!(a.accepts(3));
        assert!(!a.accepts(4));
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = FunctionRegistry::new();
        let double = DoubleFunction::new();
        let name = double.sig.name;
        reg.register(Box::new(double));

        assert!(reg.contains(name));
        assert!(reg.get(name).is_some());

        let missing = intern("missing");
        assert!(!reg.contains(missing));
        assert!(reg.get(missing).is_none());
    }

    #[test]
    fn registry_names() {
        let mut reg = FunctionRegistry::new();
        reg.register(Box::new(DoubleFunction::new()));
        let names: Vec<SymbolId> = reg.names().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn registry_eval_function() {
        let mut reg = FunctionRegistry::new();
        reg.register(Box::new(DoubleFunction::new()));
        let name = intern("double");
        let func = reg.get(name).unwrap();
        let result = func.eval(&[Term::Integer(5)]).unwrap();
        assert_eq!(result, Term::Integer(10));
    }

    #[test]
    fn registry_eval_type_error() {
        let mut reg = FunctionRegistry::new();
        reg.register(Box::new(DoubleFunction::new()));
        let name = intern("double");
        let func = reg.get(name).unwrap();
        let result = func.eval(&[Term::Symbol(intern("foo"))]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::TypeError(_)));
    }

    #[test]
    fn eval_context_empty() {
        let ctx = EvalContext::empty();
        assert!(ctx.registry.is_none());
    }

    #[test]
    fn eval_context_with_registry() {
        let reg = FunctionRegistry::new();
        let ctx = EvalContext::with_registry(&reg);
        assert!(ctx.registry.is_some());
    }

    #[test]
    fn registry_default_is_empty() {
        let reg = FunctionRegistry::default();
        assert_eq!(reg.names().count(), 0);
    }

    #[test]
    fn registry_debug_format() {
        let mut reg = FunctionRegistry::new();
        reg.register(Box::new(DoubleFunction::new()));
        let debug = format!("{reg:?}");
        assert!(debug.contains("FunctionRegistry"));
        assert!(debug.contains("double"));
    }

    /// A test function that triples an integer (same name as DoubleFunction
    /// for override testing).
    struct TripleFunction {
        sig: FunctionSignature,
    }

    impl TripleFunction {
        fn new() -> Self {
            Self {
                sig: FunctionSignature {
                    name: intern("double"), // intentionally same name
                    arity: Arity::Fixed(1),
                    description: "triples an integer",
                },
            }
        }
    }

    impl ExtensionFunction for TripleFunction {
        fn signature(&self) -> &FunctionSignature {
            &self.sig
        }

        fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
            match &args[0] {
                Term::Integer(n) => Ok(Term::Integer(n * 3)),
                other => Err(EvalError::TypeError(format!(
                    "expected integer, got {other:?}"
                ))),
            }
        }
    }

    /// A function with a distinct name for merge disjoint-key testing.
    struct IncrementFunction {
        sig: FunctionSignature,
    }

    impl IncrementFunction {
        fn new() -> Self {
            Self {
                sig: FunctionSignature {
                    name: intern("increment"),
                    arity: Arity::Fixed(1),
                    description: "adds one",
                },
            }
        }
    }

    impl ExtensionFunction for IncrementFunction {
        fn signature(&self) -> &FunctionSignature {
            &self.sig
        }

        fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
            match &args[0] {
                Term::Integer(n) => Ok(Term::Integer(n + 1)),
                other => Err(EvalError::TypeError(format!(
                    "expected integer, got {other:?}"
                ))),
            }
        }
    }

    #[test]
    fn registry_merge_disjoint() {
        let mut reg_a = FunctionRegistry::new();
        reg_a.register(Box::new(DoubleFunction::new()));

        let mut reg_b = FunctionRegistry::new();
        reg_b.register(Box::new(IncrementFunction::new()));

        reg_a.merge(reg_b);
        assert_eq!(reg_a.names().count(), 2);
        assert!(reg_a.contains(intern("double")));
        assert!(reg_a.contains(intern("increment")));
    }

    #[test]
    fn registry_merge_override_on_collision() {
        let mut reg_a = FunctionRegistry::new();
        reg_a.register(Box::new(DoubleFunction::new()));

        let mut reg_b = FunctionRegistry::new();
        reg_b.register(Box::new(TripleFunction::new()));

        // Before merge: double(5) = 10
        let func_before = reg_a.get(intern("double")).unwrap();
        assert_eq!(func_before.eval(&[Term::Integer(5)]).unwrap(), Term::Integer(10));

        reg_a.merge(reg_b);

        // After merge: "double" is overridden by TripleFunction, so double(5) = 15
        assert_eq!(reg_a.names().count(), 1);
        let func_after = reg_a.get(intern("double")).unwrap();
        assert_eq!(func_after.eval(&[Term::Integer(5)]).unwrap(), Term::Integer(15));
    }
}
