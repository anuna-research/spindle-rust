# How to Debug Unexpected Conclusions

This guide diagnoses missing or unexpected conclusions in an existing SPL theory.

Run these commands against `theory.spl`:

```bash
# 1. Check what was concluded
spindle reason --positive theory.spl

# 2. A literal you expected is missing -- find out why
spindle why-not flies theory.spl

# 3. A literal you did not expect is present -- inspect its proof
spindle explain "~flies" theory.spl

# 4. Pipe JSON output to other tools for further analysis
spindle explain "~flies" theory.spl --json | jq '.blocked_alternatives'
```

The [explanation API reference](explanations.md#cli-usage) describes both commands and their output formats.
