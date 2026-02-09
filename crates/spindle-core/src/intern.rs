//! String Interning for High-Performance Literal Handling
//!
//! This module provides a global string interner that allows strings to be
//! represented as cheap, copyable `SymbolId` values. This eliminates heap
//! allocations in hot paths of the reasoning engine.
//!
//! # Design
//!
//! - `SymbolId`: A 32-bit handle that is `Copy`, `Eq`, and `Hash`
//! - `Interner`: Thread-safe global registry using `RwLock`
//! - Zero-cost lookups after interning
//! - Strings are never deallocated (arena-style lifetime)
//!
//! # Usage
//!
//! ```rust
//! use spindle_core::intern::{intern, resolve, SymbolId};
//!
//! let id1 = intern("hello");
//! let id2 = intern("hello");  // Same ID returned
//! assert_eq!(id1, id2);
//!
//! assert_eq!(resolve(id1), "hello");
//! ```
//!
//! # Performance
//!
//! - `intern()`: O(1) average for existing strings, O(n) for new strings
//! - `resolve()`: O(1) always
//! - Memory: Strings stored once, IDs are 4 bytes

use rustc_hash::FxHashMap;
use std::sync::RwLock;

/// A unique identifier for an interned string.
///
/// This is a cheap, copyable handle that can be used in place of `String`
/// in performance-critical data structures. Two `SymbolId`s are equal if
/// and only if they refer to the same interned string.
///
/// # Size
///
/// `SymbolId` is 4 bytes (same as `u32`), compared to 24 bytes for `String`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct SymbolId(u32);

impl SymbolId {
    /// The invalid/empty symbol ID (represents empty string)
    pub const EMPTY: SymbolId = SymbolId(0);

    /// Create a SymbolId from a raw u32 value.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value corresponds to a valid interned string.
    #[inline]
    pub const fn from_raw(value: u32) -> Self {
        SymbolId(value)
    }

    /// Get the raw u32 value of this SymbolId.
    #[inline]
    pub const fn as_raw(self) -> u32 {
        self.0
    }

    /// Check if this is the empty symbol.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Debug for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SymbolId({}: {:?})", self.0, resolve(*self))
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", resolve(*self))
    }
}

/// Global string interner.
///
/// Uses a read-write lock to allow concurrent reads with exclusive writes.
/// Strings are stored in a `Vec` for O(1) resolution, with a `HashMap` for
/// O(1) average-case interning.
struct Interner {
    /// Map from string to its ID
    map: FxHashMap<&'static str, SymbolId>,
    /// Vec for ID -> string resolution (index = ID)
    strings: Vec<&'static str>,
}

impl Interner {
    fn new() -> Self {
        let mut interner = Interner {
            map: FxHashMap::default(),
            strings: Vec::with_capacity(1024),
        };
        // Reserve index 0 for empty string (SymbolId::EMPTY)
        interner.strings.push("");
        interner.map.insert("", SymbolId::EMPTY);
        interner
    }

    fn intern(&mut self, s: &str) -> SymbolId {
        if let Some(&id) = self.map.get(s) {
            return id;
        }

        let id = SymbolId(self.strings.len() as u32);
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        self.strings.push(leaked);
        self.map.insert(leaked, id);
        id
    }

    fn resolve(&self, id: SymbolId) -> &'static str {
        self.strings.get(id.0 as usize).copied().unwrap_or("")
    }

    fn len(&self) -> usize {
        self.strings.len()
    }
}

/// Global interner instance
static INTERNER: RwLock<Option<Interner>> = RwLock::new(None);

/// Initialize the global interner if not already initialized.
fn with_interner<F, R>(f: F) -> R
where
    F: FnOnce(&Interner) -> R,
{
    let guard = INTERNER.read().unwrap();
    if let Some(ref interner) = *guard {
        return f(interner);
    }
    drop(guard);

    // Initialize if needed
    let mut guard = INTERNER.write().unwrap();
    if guard.is_none() {
        *guard = Some(Interner::new());
    }
    f(guard.as_ref().unwrap())
}

/// Initialize the global interner if not already initialized, with mutable access.
fn with_interner_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Interner) -> R,
{
    let mut guard = INTERNER.write().unwrap();
    if guard.is_none() {
        *guard = Some(Interner::new());
    }
    f(guard.as_mut().unwrap())
}

/// Intern a string, returning its unique `SymbolId`.
///
/// If the string has been interned before, returns the existing ID.
/// Otherwise, stores the string and returns a new ID.
///
/// # Thread Safety
///
/// This function is thread-safe and can be called from multiple threads.
///
/// # Example
///
/// ```rust
/// use spindle_core::intern::intern;
///
/// let id1 = intern("hello");
/// let id2 = intern("hello");
/// assert_eq!(id1, id2);
///
/// let id3 = intern("world");
/// assert_ne!(id1, id3);
/// ```
#[inline]
pub fn intern(s: &str) -> SymbolId {
    // Fast path: check if already interned (read lock only)
    {
        let guard = INTERNER.read().unwrap();
        if let Some(ref interner) = *guard
            && let Some(&id) = interner.map.get(s)
        {
            return id;
        }
    }

    // Slow path: need to intern (write lock)
    with_interner_mut(|interner| interner.intern(s))
}

/// Resolve a `SymbolId` back to its string.
///
/// Returns the interned string, or an empty string if the ID is invalid.
///
/// # Thread Safety
///
/// This function is thread-safe and can be called from multiple threads.
///
/// # Example
///
/// ```rust
/// use spindle_core::intern::{intern, resolve};
///
/// let id = intern("hello");
/// assert_eq!(resolve(id), "hello");
/// ```
#[inline]
pub fn resolve(id: SymbolId) -> &'static str {
    with_interner(|interner| interner.resolve(id))
}

/// Get the number of interned strings.
///
/// Useful for debugging and statistics.
pub fn interned_count() -> usize {
    with_interner(|interner| interner.len())
}

/// A `SymbolId` that includes negation information.
///
/// This combines a symbol ID with a negation flag, allowing efficient
/// representation of literals without additional heap allocation.
///
/// # Layout
///
/// Uses the high bit of the u32 for negation, leaving 31 bits for the symbol ID.
/// This allows up to 2^31 unique symbols (over 2 billion).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct LiteralId(u32);

impl LiteralId {
    /// Bit mask for the negation flag (high bit)
    const NEGATION_BIT: u32 = 1 << 31;
    /// Bit mask for the symbol ID (lower 31 bits)
    const SYMBOL_MASK: u32 = !Self::NEGATION_BIT;

    /// The empty literal ID
    pub const EMPTY: LiteralId = LiteralId(0);

    /// Create a positive literal ID from a symbol.
    #[inline]
    pub const fn positive(symbol: SymbolId) -> Self {
        LiteralId(symbol.0 & Self::SYMBOL_MASK)
    }

    /// Create a negated literal ID from a symbol.
    #[inline]
    pub const fn negated(symbol: SymbolId) -> Self {
        LiteralId((symbol.0 & Self::SYMBOL_MASK) | Self::NEGATION_BIT)
    }

    /// Create a literal ID from a symbol and negation flag.
    #[inline]
    pub const fn new(symbol: SymbolId, negated: bool) -> Self {
        if negated {
            Self::negated(symbol)
        } else {
            Self::positive(symbol)
        }
    }

    /// Get the symbol ID (without negation).
    #[inline]
    pub const fn symbol(self) -> SymbolId {
        SymbolId(self.0 & Self::SYMBOL_MASK)
    }

    /// Check if this literal is negated.
    #[inline]
    pub const fn is_negated(self) -> bool {
        (self.0 & Self::NEGATION_BIT) != 0
    }

    /// Check if this literal is positive (not negated).
    #[inline]
    pub const fn is_positive(self) -> bool {
        !self.is_negated()
    }

    /// Get the complement (negation flipped) of this literal.
    #[inline]
    pub const fn complement(self) -> Self {
        LiteralId(self.0 ^ Self::NEGATION_BIT)
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn as_raw(self) -> u32 {
        self.0
    }

    /// Create from a raw u32 value.
    #[inline]
    pub const fn from_raw(value: u32) -> Self {
        LiteralId(value)
    }

    /// Check if this is the empty literal.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Debug for LiteralId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let neg = if self.is_negated() { "~" } else { "" };
        write!(f, "LiteralId({}{})", neg, resolve(self.symbol()))
    }
}

impl std::fmt::Display for LiteralId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negated() {
            write!(f, "~{}", resolve(self.symbol()))
        } else {
            write!(f, "{}", resolve(self.symbol()))
        }
    }
}

/// Intern a literal name and create a `LiteralId`.
///
/// Handles the `~` prefix for negation automatically.
///
/// # Example
///
/// ```rust
/// use spindle_core::intern::intern_literal;
///
/// let pos = intern_literal("flies");
/// assert!(!pos.is_negated());
///
/// let neg = intern_literal("~flies");
/// assert!(neg.is_negated());
///
/// // Same underlying symbol
/// assert_eq!(pos.symbol(), neg.symbol());
/// assert_eq!(pos, neg.complement());
/// ```
#[inline]
pub fn intern_literal(s: &str) -> LiteralId {
    if let Some(name) = s.strip_prefix('~') {
        LiteralId::negated(intern(name))
    } else {
        LiteralId::positive(intern(s))
    }
}

/// Resolve a `LiteralId` to its canonical string representation.
///
/// Returns the name with `~` prefix if negated.
pub fn resolve_literal(id: LiteralId) -> String {
    if id.is_negated() {
        format!("~{}", resolve(id.symbol()))
    } else {
        resolve(id.symbol()).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same_id() {
        let id1 = intern("hello");
        let id2 = intern("hello");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_intern_different_strings() {
        let id1 = intern("hello");
        let id2 = intern("world");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_resolve() {
        let id = intern("test_string");
        assert_eq!(resolve(id), "test_string");
    }

    #[test]
    fn test_empty_symbol() {
        let id = intern("");
        assert_eq!(id, SymbolId::EMPTY);
        assert!(id.is_empty());
        assert_eq!(resolve(id), "");
    }

    #[test]
    fn test_symbol_id_copy() {
        let id1 = intern("copy_test");
        let id2 = id1; // Copy
        assert_eq!(id1, id2);
        assert_eq!(resolve(id1), resolve(id2));
    }

    #[test]
    fn test_literal_id_positive() {
        let sym = intern("flies");
        let lit = LiteralId::positive(sym);

        assert!(!lit.is_negated());
        assert!(lit.is_positive());
        assert_eq!(lit.symbol(), sym);
    }

    #[test]
    fn test_literal_id_negated() {
        let sym = intern("flies");
        let lit = LiteralId::negated(sym);

        assert!(lit.is_negated());
        assert!(!lit.is_positive());
        assert_eq!(lit.symbol(), sym);
    }

    #[test]
    fn test_literal_id_complement() {
        let sym = intern("bird");
        let pos = LiteralId::positive(sym);
        let neg = pos.complement();

        assert!(!pos.is_negated());
        assert!(neg.is_negated());
        assert_eq!(pos.symbol(), neg.symbol());
        assert_eq!(pos, neg.complement());
    }

    #[test]
    fn test_intern_literal() {
        let pos = intern_literal("flies");
        let neg = intern_literal("~flies");

        assert!(!pos.is_negated());
        assert!(neg.is_negated());
        assert_eq!(pos.symbol(), neg.symbol());
    }

    #[test]
    fn test_resolve_literal() {
        let pos = intern_literal("bird");
        let neg = intern_literal("~bird");

        assert_eq!(resolve_literal(pos), "bird");
        assert_eq!(resolve_literal(neg), "~bird");
    }

    #[test]
    fn test_literal_id_equality() {
        let lit1 = intern_literal("test");
        let lit2 = intern_literal("test");
        let lit3 = intern_literal("~test");

        assert_eq!(lit1, lit2);
        assert_ne!(lit1, lit3);
        assert_eq!(lit1.complement(), lit3);
    }

    #[test]
    fn test_literal_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(intern_literal("a"));
        set.insert(intern_literal("b"));
        set.insert(intern_literal("~a"));

        assert!(set.contains(&intern_literal("a")));
        assert!(set.contains(&intern_literal("b")));
        assert!(set.contains(&intern_literal("~a")));
        assert!(!set.contains(&intern_literal("c")));
    }

    #[test]
    fn test_display() {
        let pos = intern_literal("bird");
        let neg = intern_literal("~flies");

        assert_eq!(format!("{pos}"), "bird");
        assert_eq!(format!("{neg}"), "~flies");
    }

    #[test]
    fn test_many_symbols() {
        let mut ids = Vec::new();
        for i in 0..1000 {
            ids.push(intern(&format!("symbol_{i}")));
        }

        // Verify all are unique
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 1000);

        // Verify all resolve correctly
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(resolve(*id), format!("symbol_{i}"));
        }
    }

    #[test]
    fn test_interned_count() {
        let before = interned_count();
        intern("unique_test_string_12345");
        let after = interned_count();
        assert!(after >= before); // May be equal if already interned in another test
    }

    #[test]
    fn test_symbol_id_from_raw_and_as_raw() {
        let id = intern("raw_test");
        let raw = id.as_raw();
        let restored = SymbolId::from_raw(raw);
        assert_eq!(id, restored);
        assert_eq!(resolve(restored), "raw_test");
    }

    #[test]
    fn test_symbol_id_debug() {
        let id = intern("debug_test");
        let debug_str = format!("{id:?}");
        assert!(debug_str.contains("SymbolId"));
        assert!(debug_str.contains("debug_test"));
    }

    #[test]
    fn test_symbol_id_display() {
        let id = intern("display_test");
        let display_str = format!("{id}");
        assert_eq!(display_str, "display_test");
    }

    #[test]
    fn test_literal_id_as_raw_and_from_raw() {
        let lit = intern_literal("~raw_lit_test");
        let raw = lit.as_raw();
        let restored = LiteralId::from_raw(raw);
        assert_eq!(lit, restored);
        assert!(restored.is_negated());
    }

    #[test]
    fn test_literal_id_is_empty() {
        let empty = LiteralId::EMPTY;
        assert!(empty.is_empty());

        let non_empty = intern_literal("not_empty");
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_literal_id_debug() {
        let pos = intern_literal("debug_lit");
        let neg = intern_literal("~debug_lit");

        let pos_debug = format!("{pos:?}");
        let neg_debug = format!("{neg:?}");

        assert!(pos_debug.contains("LiteralId"));
        assert!(pos_debug.contains("debug_lit"));
        assert!(neg_debug.contains("~"));
    }

    #[test]
    fn test_resolve_stable_after_growth() {
        // Intern a string, then intern many more to trigger Vec growth,
        // and verify the original pointer is unchanged (Box::leak guarantees this).
        let id = intern("stable_growth_test");
        let ptr_before = resolve(id).as_ptr();

        for i in 0..100 {
            intern(&format!("growth_filler_{i}"));
        }

        let ptr_after = resolve(id).as_ptr();
        assert_eq!(
            ptr_before, ptr_after,
            "resolve pointer should be stable after Vec growth"
        );
        assert_eq!(resolve(id), "stable_growth_test");
    }

    #[test]
    fn test_batch_resolve_correctness() {
        let ids: Vec<SymbolId> = (0..50).map(|i| intern(&format!("batch_{i}"))).collect();

        let resolved: Vec<&'static str> = ids.iter().map(|&id| resolve(id)).collect();

        for (i, s) in resolved.iter().enumerate() {
            assert_eq!(*s, format!("batch_{i}"));
        }
    }
}
