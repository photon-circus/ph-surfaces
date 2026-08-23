# Releasing ph-surfaces

This document governs durable crates.io and GitHub releases. Publishing is a
maintainer-controlled action; ordinary development and pull-request CI must
never upload, create a tag, or create a GitHub Release.
Follow Cargo's official
[publishing reference](https://doc.rust-lang.org/cargo/reference/publishing.html)
for registry mechanics; this runbook adds the repository's stricter evidence
and sequencing rules.

The first intended release is ordinary SemVer `0.1.0`, tagged `v0.1.0`, with
repository lifecycle **Active**. Human- and agent-facing documentation already
describes that destination. This runbook is how the crate version, `publish`
setting, changelog heading, GitHub metadata, tag, and upload catch up. There
is no 1.0 compatibility promise.

## Roles

- The **release operator** assembles the branch, records evidence, tags, and
  performs the manual upload.
- The **evidence reviewer** independently checks the final SHA, archive,
  provenance, checksum, and complete gate result.
- A **repository administrator** handles visibility, metadata, security
  features, Actions policy, and branch protection.

The release operator and evidence reviewer should be different people whenever
practical.

## 1. Assemble the release branch

Create `release/<full-semver>` from the intended `main` commit and open a draft
merge-back pull request. Keep later development out of the release branch.

Before changing visibility or publishing, require:

- every release-blocking issue closed or explicitly dispositioned;
- public API and documentation reviewed for the release profile;
- repository-specific contribution, conduct, security, and release guidance
  present;
- a completed full-history secret, identity, integrity, and size review;
- installed GitHub Apps reviewed and restricted;
- explicit approval for every identity that will become public in Git history.

If history must be rewritten, do it only while private and restart all release
evidence from the resulting commit.

## 2. Prepare release metadata

For the release commit:

- set `version = "0.1.0"` in `Cargo.toml`;
- regenerate the matching `version` line in `Cargo.lock` (for example with
  `cargo update --workspace`); a stale lockfile fails `cargo package --locked`
  and `cargo publish --locked`;
- replace `publish = false` with `publish = ["crates-io"]`;
- do not restore the deprecated Cargo `authors` field;
- add `documentation = "https://docs.rs/ph-surfaces"`;
- pin unpackaged guide links from `blob/main/` to immutable `blob/v0.1.0/`
  URLs everywhere they appear in packaged files — `README.md`, the crate docs
  in `src/lib.rs`, and the five `examples/*.rs` headers (do not package
  `docs/`);
- move accumulated changelog entries into `## 0.1.0 - YYYY-MM-DD` (UTC),
  retaining an empty `Unreleased` section and a value statement under the
  release heading;
- update `package.version` and `package.manifest.publish` in
  `tools/xtask/config.ron`; the manifest-floor and package checks read those
  declarative expectations, and mutation fixtures load the same file.

After this step, `grep -r "0.1.0-incubating" --include="*.rs" --include="*.toml"
--include="*.ron" --include="*.lock" .` from the repository root must return
nothing.

The changelog release section must state the user value, important constraints,
known issues, and any breaking change explicitly.

## 3. Establish public repository controls

Only after the history/app review passes, make the repository public and set:

```text
Lifecycle=Active
Domain=Libraries
topics: rust, embedded, no-std, no-alloc, interpolation
```

Verifying those fields is a release-checklist action, not a code gate — no
code change can fix a wrong repository setting. The operator confirms them
with:

```sh
gh api repos/photon-circus/ph-surfaces --jq '.topics'
gh api repos/photon-circus/ph-surfaces/properties/values
```

Enable appropriate dependency, secret, and code-security features. The public
`.github/workflows/ci.yml` must keep bounded `pull_request`, `push`-to-`main`,
and manual triggers, and must call the canonical `cargo xtask ci` entry point.
Obtain one green aggregate `ci` check on the exact release commit, and protect
`main` according to the organization standard before upload. Do not enable the
automatic triggers while the repository is still private.

## 4. Run the final clean matrix

Use a fresh clone of the exact proposed release commit. Provision and record
the pinned stable toolchain, a reviewed dated nightly with `rust-src`, both
embedded targets, cargo-deny, gitleaks, and git-sizer.
The date below is the audited starting point; deliberately update it everywhere
if the release uses a later reviewed nightly.

```bash
mkdir -p target
set -euo pipefail
test -z "$(git status --porcelain)"
rustup toolchain install nightly-2026-08-08 --component rust-src
rustup target add --toolchain 1.94.0 \
  thumbv7em-none-eabi riscv32imac-unknown-none-elf
{
  rustc -Vv
  cargo -V
  rustup run nightly-2026-08-08 rustc -Vv
  cargo +nightly-2026-08-08 -V
  cargo deny --version
  gitleaks version
  git-sizer --version
  git --version
  rustup target list --toolchain 1.94.0 --installed
} | tee target/release-tool-versions.log
cargo xtask ci --profile release --nightly nightly-2026-08-08 2>&1 | tee target/release-ci.log
grep -Eq '^Summary[[:space:]]*$' target/release-ci.log
! awk '/^Summary[[:space:]]*$/ { block = ""; capture = 1 } capture { block = block $0 ORS } END { printf "%s", block }' \
    target/release-ci.log | grep -Eq '^  (FAIL|SKIP)  '
cargo test --locked --release
cargo deny check
cargo tree --locked --edges all
gitleaks git . --redact --log-opts=--all
git fsck --full
git-sizer --verbose
cargo package --locked
cargo publish --dry-run --locked
```

The strict gate must prove the host suites, five examples, all sixteen strategy
pairings, both ordinary embedded targets, both core-only sysroots, package
contents, packaged docs/doctests/examples, and a fresh downstream `#![no_std]`
consumer.

Inspect `target/package/ph-surfaces-<version>.crate` and require:

- its file list equals the reviewed allowlist;
- its MIT license and manifest metadata are correct;
- `git status --porcelain` is empty;
- `.cargo_vcs_info.json` has `git.sha1` equal to `git rev-parse HEAD`;
- `.git.dirty`, when present, is not `true` (Cargo 1.94 omits it when false);
- its recorded SHA-256 is generated from this final release artifact.

The evidence reviewer must compare the log, archive, VCS information, and
checksum to the exact commit. Any failure, skip, dirty state, or SHA movement
stops the release.

## 5. Tag before upload

Create an annotated tag on the verified commit:

```sh
git tag -a v0.1.0 -m "ph-surfaces 0.1.0"
git rev-parse 'v0.1.0^{}'
```

Require the resolved commit to equal the evidence SHA, then push the tag. Never
move a tag after it identifies an uploaded artifact.

Immediately before uploading, recheck `ph-surfaces` and the normalization-
equivalent `ph_surfaces` in the crates.io API/search and sparse index. A prior
404 is not a reservation; stop if either name is claimed or ambiguous.

## 6. Publish manually

Run one final clean dry run from the tagged commit. Then publish manually with a
short-lived token scoped as narrowly as crates.io permits:

```sh
cargo publish --locked
```

Do not expose the token in history, issue/PR text, captured command arguments,
or logs. After the first package exists, retain the named maintainer and add the
organization publisher team:

```sh
cargo owner --add github:photon-circus:crate-publishers ph-surfaces
```

Verify the resulting owner list.

## 7. Verify before creating the GitHub Release

Verify the durable crates.io artifact before announcing it:

1. Check version, license, repository, README, keywords/categories, and owners.
2. Download the `.crate`; require its hash to match the recorded local archive
   and the sparse-index checksum.
3. Reinspect its files and `.cargo_vcs_info.json`.
4. Wait for docs.rs and verify its build log, public items, version, and links.
5. Create a fresh downstream crate using the exact registry dependency
   `ph-surfaces = "=0.1.0"` with no path override.
6. Run host, ARM, RISC-V, and both core-only builds and instantiate all sixteen
   strategy pairings.

Only then create the GitHub Release from the existing tag, use the matching
changelog section as release notes, and do **not** mark it as a prerelease.
Merge the release branch back promptly, reopen `Unreleased`, and remove the
completed release branch.

## Rollback and replacement

### Before upload

Stop. If an unpublished tag was pushed and no durable artifact or GitHub
Release refers to it, coordinate its removal, fix the candidate, rerun the full
matrix, and create a newly verified tag. Never retarget an already published
tag.

### After upload

Crates.io versions are permanent records. Do not overwrite, delete, or retarget
the package or tag. Record the defect, yank the affected version so new
resolution avoids it, and publish the next version from a new commit, matrix,
tag, checksum, and GitHub Release. A patch uses `0.1.1`. A pre-1.0 breaking
change uses `0.2.0`.

The following is the documented operator command and is not a test command:

```sh
cargo yank --version 0.1.0 ph-surfaces
```

Yanking does not delete the artifact and does not break existing lockfiles.
Never run a real yank as a simulation.
