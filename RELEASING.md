# Releasing aetherd

Releases are tag-driven. Pushing a `vX.Y.Z` tag triggers
`.github/workflows/release.yml`, which verifies the tree, builds the binary,
publishes a GitHub Release, and pushes a container image.

## Versioning

The project follows [Semantic Versioning](https://semver.org/).

- `0.y.z` while the public API can still change between minor versions. Adding
  fields is fine at any time; breaking `/v1` requires a new API version.
- `1.0.0` only once the API is considered stable and the planned AI provider
  surface has shipped. Do not cut `1.0.0` while `/v1/usage` and provider support
  are still missing.
- Patch releases (`0.y.Z`) are for fixes that do not change the API.

`Cargo.toml` is the source of truth for the version. `/health`, the OpenAPI
document, and the binary's `--version` all derive from it, so only one place
needs editing.

## Checklist

1. Ensure `main` is green and the working tree is clean.
2. Decide the next version from the changes since the last tag.
3. Edit `Cargo.toml` `version`. Run any `cargo` command that updates
   `Cargo.lock`, then confirm the lockfile change is only the crate version.
4. Move the `Unreleased` entries in `CHANGELOG.md` into a new
   `## [X.Y.Z] - YYYY-MM-DD` section and update the compare links at the bottom.
5. Commit as `chore(release): vX.Y.Z`.
6. Create an annotated tag and push it:

   ```bash
   git tag -a vX.Y.Z -m "aetherd vX.Y.Z"
   git push origin main
   git push origin vX.Y.Z
   ```

7. Watch the release workflow and confirm:
   - the GitHub Release exists with the binary archive and `SHA256SUMS`;
   - `ghcr.io/julestblt/aetherd:X.Y.Z` and `:latest` are published;
   - a smoke test of the published binary passes.

## Verification performed by CI

The `verify` job fails the release before anything is published when:

- formatting, clippy, or tests fail;
- `Cargo.toml`'s version does not match the tag;
- `CHANGELOG.md` has no section for the tag's version.

This makes a mismatched release impossible: the tag either produces a coherent
release, or it produces nothing.

## Post-release

- Keep the released section in `CHANGELOG.md` for history; new work goes back
  under `Unreleased`.
- If a release is wrong and nothing should remain, delete the GitHub Release and
  the tag, fix the cause, and cut a new patch version. Do not move a published
  tag that downstream users may have fetched.
