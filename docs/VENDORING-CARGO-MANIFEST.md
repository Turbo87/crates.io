# Vendoring `cargo-manifest` as `crates_io_cargo_toml`

This document is a reproducible runbook for how the `cargo-manifest` crate was
imported into this workspace as `crates_io_cargo_toml`, with its upstream git
history merged in.

## Background / decisions

- Upstream: <https://github.com/LukeMathWalker/cargo-manifest>, tag **`v0.19.1`**
  (commit `adc5d1e`) — the same version crates.io already depended on, so the
  behavior is unchanged.
- Use **`git subtree`** (without `--squash`) so the full upstream history
  (175 commits) is merged into the DAG rather than flattened.
- Import into `crates/cargo-manifest/` first, then rename to
  `crates/crates_io_cargo_toml/` in the final commit, so the rename is a real,
  reviewable change.
- Three commits: **(1) import → (2) integrate → (3) rename**.

Start on a feature branch with a clean working tree.

## Commit 1 — Import (subtree merge)

```bash
git remote add cargo-manifest-upstream https://github.com/LukeMathWalker/cargo-manifest.git
git fetch cargo-manifest-upstream tag v0.19.1 --no-tags

# No --squash → full history becomes ancestors of HEAD.
# Files land at crates/cargo-manifest/ with the upstream package name intact.
git subtree add --prefix=crates/cargo-manifest cargo-manifest-upstream v0.19.1

git remote remove cargo-manifest-upstream   # optional cleanup; objects stay in the repo
```

This creates a merge commit `Add 'crates/cargo-manifest/' from commit 'adc5d1e…'`
with two parents (your branch + the upstream tag). Nothing else depends on it
yet; the workspace still uses the registry `cargo-manifest`. Verify resolution
with `cargo metadata --no-deps >/dev/null`.

## Commit 2 — Integrate into the workspace (still named `cargo-manifest`)

**a. Replace `crates/cargo-manifest/Cargo.toml`** with workspace conventions
(keep the `name` and lib name for now):

```toml
[package]
name = "cargo-manifest"
version = "0.0.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/rust-lang/crates.io"
description = "Helper crate to parse and manipulate manifests - `Cargo.toml` files."
edition = "2021"

[lib]
name = "cargo_manifest"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
serde = { version = "=1.0.228", features = ["derive"] }
thiserror = "=2.0.18"
toml = { version = "=1.1.2", default-features = false, features = [
  "parse",
  "serde",          # REQUIRED: in toml 1.x, `Value`/`from_str` are gated behind `serde`
  "preserve_order",
] }

[dev-dependencies]
insta = "=1.47.2"
tempfile = "=3.27.0"
toml = { version = "=1.1.2", default-features = false, features = ["display", "serde"] }
```

> **Key gotcha:** upstream targets `toml` 0.8, but this workspace is on
> `toml` 1.x. Without the `serde` feature you get `cannot find type Value` /
> `cannot find function from_str`. Keeping `edition = "2021"` avoids needless
> edition-migration churn (and keeps the crate's own `own()` test, which asserts
> `Edition::E2021`, valid).

**b. Remove standalone-repo tooling:**

```bash
git rm -r crates/cargo-manifest/.github \
          crates/cargo-manifest/Cargo.lock \
          crates/cargo-manifest/cliff.toml \
          crates/cargo-manifest/deny.toml \
          crates/cargo-manifest/.gitignore \
          crates/cargo-manifest/CODE_OF_CONDUCT.md
```

Keep `README.md`, `CHANGELOG.md`, `LICENSE-APACHE`, `LICENSE-MIT`, `src/`,
`tests/`.

**c. Repoint the three consumers** from the registry crate to the path crate.
Change only the dependency line — the lib is still `cargo_manifest`, so no
source edits are needed yet:

- `Cargo.toml` (root):
  `cargo-manifest = "=0.19.1"` → `cargo-manifest = { path = "crates/cargo-manifest" }`
- `crates/crates_io_tarball/Cargo.toml`:
  → `cargo-manifest = { path = "../cargo-manifest" }`
- `crates/crates_io_test_utils/Cargo.toml`:
  → `cargo-manifest = { path = "../cargo-manifest" }`

**d. Regenerate the lock file and verify, then commit:**

```bash
cargo check -p crates_io_tarball -p crates_io_test_utils
grep -A1 'name = "cargo-manifest"' Cargo.lock   # version 0.0.0, no registry source
git add -A && git commit -m "Integrate cargo-manifest into the workspace"
```

## Commit 3 — Rename to `crates_io_cargo_toml`

**a. Move the directory:**

```bash
git mv crates/cargo-manifest crates/crates_io_cargo_toml
```

**b. In `crates/crates_io_cargo_toml/Cargo.toml`:** set `name = "crates_io_cargo_toml"`
and `[lib] name = "crates_io_cargo_toml"`.

**c. Update the crate's own tests** (`tests/autolib.rs`, `autotarget.rs`,
`parse.rs`, `serialize.rs`): replace `cargo_manifest` → `crates_io_cargo_toml`.
In `tests/parse.rs` also change the two `assert_eq!("cargo-manifest", package.name)`
assertions → `"crates_io_cargo_toml"` (the `own()` test reads its own
`Cargo.toml`).

**d. Update `crates/crates_io_cargo_toml/README.md`** — it is included as crate
docs via `#![doc = include_str!("../README.md")]`, so its
`use cargo_manifest::Manifest;` example is a **doctest** that must become
`use crates_io_cargo_toml::Manifest;` (also update the title and drop the
`cargo add` line). Otherwise `cargo test` fails on the doctest.

**e. Update consumer dependency keys and imports:**

| File | Change |
|---|---|
| `Cargo.toml` (root) | dep key → `crates_io_cargo_toml = { path = "crates/crates_io_cargo_toml" }`, re-sort among `crates_io_*` (between `crates_io_api_types` and `crates_io_cdn_logs`) |
| `crates/crates_io_tarball/Cargo.toml` | → `crates_io_cargo_toml = { path = "../crates_io_cargo_toml" }` |
| `crates/crates_io_test_utils/Cargo.toml` | → `{ path = "../crates_io_cargo_toml" }`, re-sort (moves below `chrono`, into the `crates_io_*` block) |
| `crates/crates_io_tarball/src/lib.rs` | `cargo_manifest` → `crates_io_cargo_toml` |
| `crates/crates_io_tarball/src/manifest.rs` | `cargo_manifest` → `crates_io_cargo_toml` |
| `crates/crates_io_test_utils/src/builders/publish.rs` | `cargo_manifest` → `crates_io_cargo_toml` |
| `src/controllers/krate/publish.rs` | `cargo_manifest` → `crates_io_cargo_toml` |

**f. Regenerate the lock, confirm nothing stale, and commit:**

```bash
cargo check -p crates_io_cargo_toml
grep -rn 'cargo_manifest\|cargo-manifest' --include='*.rs' --include='*.toml' . \
  | grep -v 'crates/crates_io_cargo_toml/CHANGELOG'   # expect no hits
git add -A && git commit -m "Rename cargo-manifest to crates_io_cargo_toml"
```

## Final verification

```bash
cargo test  -p crates_io_cargo_toml       # lib + autolib/autotarget/parse/serialize + doctest
cargo check -p crates_io --lib            # main crate consumes it via publish.rs
git log --oneline -4                      # import -> integrate -> rename on top of base
git merge-base --is-ancestor adc5d1e HEAD && git log --oneline adc5d1e | wc -l  # 175 upstream commits
```

## Notes & gotchas

- **`toml` `serde` feature** is the single most important non-obvious change
  (see commit 2, step a).
- **README doctest** is compiled into the crate's docs, so it must be renamed
  or `cargo test` fails.
- **`git log --follow`** stops at the subtree merge boundary, because the
  upstream files lived at the repository root (e.g. `src/lib.rs`). That is
  normal for subtree imports; browse the upstream history via
  `git log <import-commit>^2`. The commits are genuinely in the DAG regardless.
- **Alphabetical dependency ordering**: `cargo-manifest` and
  `crates_io_cargo_toml` sort to different positions, so the rename shifts the
  dependency line in the root and `crates_io_test_utils` manifests (the repo
  keeps `[dependencies]` sorted).
- Each commit builds independently (commit 1 resolves with both the registry
  and path crate present; commits 2 and 3 were each checked).
