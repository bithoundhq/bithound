# Ticket workflow

Bithound is built ticket by ticket. One BTH ticket → one branch → one
PR. The conventions below match the project's `CLAUDE.md`.

## Branch naming

- Ticket: `bth-<n>-<short-slug>` (e.g. `bth-19-engine-handle`).
- Non-ticket fix: a descriptive slug (e.g.
  `fix-rust-yml-clippy-typo`, `docs-mdbook-scaffold`).

## Commit messages

```
BTH-N: <one-line summary>

<paragraphs of context if useful>
```

Do not include a `Co-Authored-By` trailer or any AI attribution.

## PR body

- `Closes #<ticket-number>` on the first line of the body.
- A short Summary section.
- The ticket's Acceptance Criteria checklist, with completed items
  checked.
- A Test plan.
- A "Deviations from spec" section (usually "None").

## Stacked PRs

When tickets depend on each other, stack the branches:

```bash
gh pr create --base <parent-branch>
```

GitHub auto-rebases the base when the parent merges.

**Update branch before merging.** If two PRs merge close together,
GitHub may merge the second with a stale first-parent and the changes
won't appear on `main`'s first-parent path. Click *Update branch* on
the second PR before clicking *Merge*.

## Phase bundles

Most BTH tickets ship one-per-PR. A small number of phases (Phase 10
runtime, Phase 11 rules, Phase 12 e2e + docs) bundle 2–5 tickets into
a single PR with one commit per ticket plus a `chore:` commit for
`VERSION` + `CHANGELOG`. Bundle only when the tickets are tightly
coupled and reviewing them together is easier than reviewing them
apart; document the bundling decision in the PR body's "Deviations
from spec" section.

## CI gates

`.github/workflows/rust.yml` runs `cargo fmt --check`,
`cargo clippy -- -D warnings`, `cargo build --verbose`, and
`cargo test --verbose` on every PR to `main`. All four must pass.
The 230+ unit tests run on every push; the one `#[ignore]`-gated
end-to-end integration test under `tests/e2e_tip_lag.rs` is
opt-in only — CI does not run it.

`#[allow(dead_code)]` lives crate-wide in `src/main.rs`. It started
out covering the typed domain model that landed ahead of the runtime
in V0; today most of those types are wired. Don't add new
`dead_code` casually — if you find yourself reaching for it,
chances are the code is genuinely unused and should be deleted.

## Version + CHANGELOG

Bithound uses a 4-digit `MAJOR.MINOR.PATCH.MICRO` version in
`VERSION`, separate from `Cargo.toml`'s semver-style version
(which stays at `0.1.0` for V0). Every PR that ships a feature or
fix bumps the 4-digit `VERSION` and adds a CHANGELOG entry. PR
titles are version-prefixed (e.g. `v0.0.5.0 Phase 12: ...`).

## What goes in the diff

- One ticket's worth of changes (or one phase bundle), no drive-bys.
- Never `git add -A` or `git add .`. Stage specific files. The
  working tree often has pre-session edits to `Cargo.lock` and
  similar that must not get pulled into a ticket commit. Use
  `git stash push <paths>` if needed.
- Do not commit unless asked. PRs are merged by the project owner.
