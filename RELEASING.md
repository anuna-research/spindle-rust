# Releasing

This project currently follows pre-1.0 Semantic Versioning (`0.y.z`):

- Breaking changes: bump `y` (`0.1.3` -> `0.2.0`)
- Backward-compatible changes/fixes: bump `z` (`0.1.3` -> `0.1.4`)
- `1.0.0` is reserved for intentional API stability guarantees

## Release Checklist

1. Ensure `main` is up to date.
2. Ensure `[workspace.package].version` in `Cargo.toml` matches the intended release version.
3. Run:

   ```bash
   make check
   make test
   ```

4. Merge all intended release changes to `main`.
5. Create and push an annotated tag in `vX.Y.Z` format:

   ```bash
   git checkout main
   git pull --ff-only origin main
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

6. Confirm Woodpecker tag pipeline succeeds (`.woodpecker/release.yaml`).
7. Verify artifacts exist in the versioned and `latest` cloud paths.

## Notes

- The release pipeline derives artifact version strings from tag names by stripping the `v` prefix.
- Tag names must follow `vX.Y.Z` exactly.
- If/when the project moves to stable API guarantees, switch to standard `1.x` SemVer behavior.
