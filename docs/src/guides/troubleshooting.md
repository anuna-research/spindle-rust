# Troubleshooting

Common issues and how to resolve them.

## Parse Errors

### "Could not parse" Error

```
Error at line 5: could not parse: bird flies
```

**Cause**: Missing arrow operator.

**Fix**: Add the correct arrow:
```lisp
r1: bird => flies
```

### "Unknown keyword" Error (SPL)

```
SPL parse error: Unknown keyword: defeasible
```

**Cause**: Wrong keyword.

**Fix**: Use correct SPL keywords:
- `given` (not `fact`)
- `normally` (not `defeasible`)
- `always` (not `strict`)
- `except` (not `defeater`)

### Missing Label (SPL)

```
Error at line 3: could not parse: >> bird
```

**Cause**: SPL rules require labels.

**Fix**: Add a label:
```lisp
f1: >> bird
```

## Unexpected Conclusions

### Expected Conclusion Missing

**Symptom**: A literal you expected to be `+d` is `-d`.

**Debugging steps**:

1. Check if the rule exists:
   ```bash
   spindle stats theory.spl
   ```

2. Check if the body is satisfied:
   ```lisp
   # Is 'bird' actually proven?
   r1: bird => flies
   ```

3. Check for conflicts:
   ```bash
   # Look for rules proving the complement
   grep "~flies\|-flies" theory.spl
   ```

4. Check superiority:
   ```bash
   # Is there a superior rule blocking?
   grep ">" theory.spl
   ```

### Unexpected Conclusion Present

**Symptom**: A literal is `+d` when it shouldn't be.

**Debugging steps**:

1. Find which rule proves it:
   ```bash
   grep "=> literal\|-> literal" theory.spl
   ```

2. Check if a defeater is missing:
   ```lisp
   # Add a defeater to block
   d1: exception ~> unexpected_literal
   ```

### Both Literals Unprovable (Ambiguity)

**Symptom**: Neither `q` nor `~q` is `+d`.

**Cause**: Conflicting rules without superiority.

**Fix**: Add superiority:
```lisp
r1: a => q
r2: b => ~q
r1 > r2    # or r2 > r1
```

## Superiority Issues

### Superiority Not Working

**Symptom**: Declared `r1 > r2` but r2 still wins.

**Check**:

1. Rule labels match exactly:
   ```lisp
   r1: bird => flies      # Label is "r1"
   r2: penguin => ~flies  # Label is "r2"
   r1 > r2                # Must match exactly
   ```

2. Rule type compatibility:
   - Superiority only affects defeasible rules and defeaters
   - Strict rules always win regardless of superiority

3. Both rules actually fire:
   ```bash
   # Both bodies must be satisfied
   spindle reason --positive theory.spl | grep "bird\|penguin"
   ```

### Circular Superiority

```lisp
# BAD: creates undefined behavior
r1 > r2
r2 > r1
```

**Fix**: Remove one declaration or restructure rules.

## Grounding Issues

### Variables Not Matching

**Symptom**: Rules with variables don't produce expected results.

**Check**:

1. Facts use predicates:
   ```lisp
   ; Wrong: not a predicate
   (given parent-alice-bob)

   ; Right: predicate with arguments
   (given (parent alice bob))
   ```

2. Variable positions match:
   ```lisp
   ; Fact: (parent alice bob)
   ;              ?x    ?y

   ; Rule must match positions
   (normally r1 (parent ?x ?y) (ancestor ?x ?y))
   ```

3. All head variables appear in body:
   ```lisp
   ; INVALID: ?z not in body
   (normally r1 (parent ?x ?y) (triple ?x ?y ?z))
   ```

### Grounding Explosion

**Symptom**: Memory exhaustion or very slow reasoning.

**Cause**: Too many variable combinations.

**Fix**:
- Add constraints to rule bodies
- Break large joins into smaller rules
- Add explicit superiority where conflicts are expected

## File Format Issues

### Wrong Format Detection

**Symptom**: File parses incorrectly.

**Fix**: Use correct extension:
- `.spl` for SPL format
- `.spl` for SPL format

### Encoding Issues

**Symptom**: Special characters cause errors.

**Fix**: Use UTF-8 encoding. The negation symbol `¬` is supported.

## Conflict Expectations

### Conflicting Defeasible Rules Both Show As Provable

**Symptom**: You see both `+d p` and `+d ~p`.

**Cause**: No superiority relation resolves the conflict.

**Fix**:
1. Add an explicit superiority declaration between the conflicting rules.
2. Re-run reasoning and verify only the preferred side remains `+d`.

## Performance Issues

### Slow Reasoning

**Check**:
1. Theory size: `spindle stats theory.spl`
2. Conflict graph: add superiority to resolve high-conflict hotspots
3. Grounding: check for variable explosion
4. Conflicts: add superiority to reduce ambiguity

### Memory Exhaustion

**Causes**:
- Too many ground rules (variable explosion)
- Very long inference chains
- Large number of conflicts

**Fixes**:
- Reduce variable combinations
- Restructure theory

## Debugging Tips

### Validate First

Always validate before reasoning:
```bash
spindle validate theory.spl && spindle reason theory.spl
```

### Minimal Reproduction

Create a minimal theory that reproduces the issue:
```lisp
# Start with just the failing rules
f1: >> bird
r1: bird => flies
# Add rules until issue appears
```

### Use Positive Output

Focus on what IS proven:
```bash
spindle reason --positive theory.spl
```

### Check Statistics

```bash
spindle stats theory.spl
```

Look for unexpected counts.

### Enable Logging

```bash
SPINDLE_LOG=debug spindle reason theory.spl 2>&1 | less
```

## Getting Help

If you can't resolve an issue:

1. Create a minimal reproduction
2. Include the theory file
3. Show expected vs actual output
4. Report at: https://codeberg.org/anuna/spindle-rust/issues
