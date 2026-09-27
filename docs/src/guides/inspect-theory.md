# How to inspect a theory

Start with `penguin.spl` from [Getting Started](../getting-started.md).

1. Show each proved literal once:

   ```bash
   spindle reason penguin.spl
   ```

2. Request structured JSON output:

   ```bash
   spindle reason --json penguin.spl
   ```

3. Check syntax without reasoning:

   ```bash
   spindle validate penguin.spl
   ```

4. Inspect theory statistics:

   ```bash
   spindle stats penguin.spl
   ```

The [CLI reference](../reference/cli.md) describes each command and its output.

For proof tags and refutations, use `spindle reason --detailed penguin.spl`.
Add `--positive` to filter that detailed view or JSON output to positive tags.
