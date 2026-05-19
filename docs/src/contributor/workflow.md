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

## CI gates

`.github/workflows/rust.yml` runs `cargo fmt --check`, `cargo clippy
-- -D warnings`, `cargo build --verbose`, and `cargo test --verbose`
on every PR to `main`. Expect a sizeable pile of `dead_code` warnings
while the runtime is being assembled — those are types declared ahead
of the code that will use them. They turn into errors under
`-D warnings`, so don't add new ones casually; otherwise leave them
alone.

## What goes in the diff

- One ticket's worth of changes, no drive-bys.
- Never `git add -A` or `git add .`. Stage specific files. The working
  tree often has pre-session edits to `Cargo.lock` and similar that
  must not get pulled into a ticket commit.
- Do not commit unless asked. PRs are merged by the project owner.
