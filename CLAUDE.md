# CLAUDE.md

The rules every session here follows, subagents included. One orchestrator
seat and two lanes work here, each in its own worktree, and no session hands
over. Beside this file a session reads:

| Session | Also reads |
| --- | --- |
| the orchestrator seat | `roles/seat.md` |
| a lane | `roles/lane.md` |

## Where things live

- **`.claude/rules/code.md`** — the rules of the code: for the reader, one
  source of truth, units of change, errors, tests, the language's own. Claude
  Code loads it when a session touches the paths it names, so it needs no
  reading step. (`templates/code.md` of chun-harness is the model; a project
  keeps its languages and adds its own section.)
- **`roles/seat.md`, `roles/lane.md`** — what only that role does, read by
  the role at its start; never under `.claude/rules/`, which loads into every
  session.
- **`ARCHITECTURE.md`** — the codemap: the two crates, the Tauri
  shell, the UI's pages and components, where each runs, and what may depend
  on what.
  A map of the present: a change rewrites the cell it touches and never
  appends to it.
- **GitHub Issues** — `gh issue list` is the open work.
- **`doc/YYYY-MM-DD-<subject>-design.md` · `-plan.md` · `-measurement.md`** —
  decisions as `D` numbers, a plan's tasks with no code, and a measurement
  dated when taken, with its conditions (chun-harness
  `doc/documents-and-citations.md`).
  The designs in force are `doc/2026-07-02-xr-touch-trigger-design.md`
  (`D1` to `D14`), `doc/2026-07-09-portable-build-design.md` (`D15`) and
  `doc/2026-07-15-http-action-design.md` (`D16`); each `-plan.md` is the
  record of how it was built, and `doc/windows-checklist.md` is what is
  verified on the touchscreen PC by hand.
- **What was done** is `main`'s history and the issues and pull requests;
  nothing else records it.
- **The API docs `cargo doc` renders** are the invariant SSOT for the
  crates: a module's `//!` states what the code holds; the UI's `tokens.css`
  and the Svelte components state the look. A document cites the symbol.

## Non-negotiables

Enforced by the check (`scripts/check.sh`, run by `.github/workflows/ci.yml`
for every pull request and every push to `main` or `production`). Do not
route around them.

1. **`shellcheck` passes with no warning and `shfmt -i 2 -ci` changes nothing on every script**, in the check every change takes before it merges. No session runs the check to verify a change, and a hand run only diagnoses one (the `gates` skill).
2. **A shellcheck directive is placed at the narrowest scope, with its reason written beside it**, never file-wide.
3. **A behaviour of a script is pinned by a test** that the check runs, written with the change that adds the behaviour.
4. **`cargo fmt` runs before every commit that holds code, and `cargo clippy` passes with warnings as errors** in the check every change takes before it merges. No session runs the check to verify a change, and a hand run only diagnoses one (the `gates` skill).
5. **A lint is relaxed at the narrowest scope, with its reason written beside it** (`#[expect(lint, reason = "…")]`), never blanket-allowed.
6. **A behaviour is pinned by a test `cargo test --workspace` runs**, written with the change that adds the behaviour.
7. **`npm ci` and `npm run build` are clean** in the check every change takes before it merges.
8. **A change to the page is tried in a browser before it merges**, and the pull request says so.
9. **`package-lock.json` moves only with a dependency change** that names its reason.

## Git

- **Run `git status` before anything that writes a working tree**, and say
  what it showed. **Use the command that does one thing**: `git restore
  --staged` to unstage, `git revert` to undo a pushed commit, `git branch -f`
  to move a pointer, `git reset --hard` only when the tree should move. Never
  `git checkout`.
- **Every push goes through `scripts/publish.sh`** (the `publish` skill).
  Never force-push a branch another worktree holds.
- **A commit message ends with `Co-Authored-By`**, the model's own line,
  never a session link.

## Evidence

- **A claim is evidenced before it is made**, by a command run and its
  output read, and **checked against the tree before it is acted on**,
  whoever made it, a memory included: a file, a flag or a state a memory
  names is read in the tree first. When two trees disagree, find which one
  is behind.

## Documents and citations

- **An obligation that outlives the task at hand is a GitHub issue**, an
  unanswered `Q` and the unpaid debt of a `D` included: one unit of work
  each, a tightly-coupled follow-up its sub-issue.
- **The issue holds the state, the design holds the substance.** A decision
  is a `D` number in the design that owns its subject, filed when found as
  its own commit, and every other mention points at it. A `Q` closes with the
  pull request that closes its issue and edits the paragraph that posed it.
  **A design holds only what is in force, and nothing of the path to it**: a
  changed decision is rewritten in place to the fact as it now stands, with
  no amendment note and no earlier text kept; the earlier version, the
  question behind a change and the turns on the way are the commit's and the
  issue's. A decision that no longer holds keeps its number and one line
  naming what replaced it, so a citation still resolves.
- **A finding is written once**: what is wrong, where and what closes it in
  the issue, a few hundred characters; the evidence in the pull request and
  its commits; a decision alone in a `D` number. An issue or a pull request
  follows its template under `.github/`.
- **Name a symbol, never a line number.** A citation is
  `path/to/file::symbol`: the full path when it leaves the file, the
  basename inside it, never a bare module path.
- **A `D` number cited between documents carries its document**:
  `` `D13` of `doc/2026-09-02-<subject>-design.md` ``. Inside a plan, its own
  design's `D` numbers are bare.
- **A GitHub issue is written `Issue #N` and a pull request `PR #N`**, in a
  document, a memory, a notepad, a commit and a title; a bare `#N` is written
  only after a closing keyword; and in an issue's body a document is a link to
  it on `main`.
- **Every dated entry carries a local time**, read in the command that writes
  it: `TZ='Asia/Seoul' date '+%Y-%m-%d %H:%M KST'`.
- **Code carries no citation out**: a comment states the fact, and the
  document cites the code.

## Language

- **Never quote the operator verbatim, anywhere.** State the decision in your
  own words: a verbatim relay hands the reading to a session that heard less.
  Literal content — a config value, a log line, a file's text — is copied
  exactly. A phrase in quotes that nobody said is not a quotation.
- **A table's cells are short.** When a cell would hold more than one
  sentence, write the rows as items instead: a heading per row, then one
  labelled line per column.

## Keep the workflow light

- **Take out before adding.** A red is fixed at its cause. Before a rule, a
  test or a gate is added, find the one it replaces, and remove what no
  longer earns its place, such as a check that has caught nothing in hundreds
  of runs and guards no silent harm.
- **A change is checked by what it can affect, and by nothing else**: a
  rule, a hook, a script or a document by its own test, and a gate that runs
  for a change it cannot reach is narrowed.
- **A finding met while working an issue is fixed inside that issue**, and so
  is a new ruling on work of the same flow whose pull request has not merged:
  its issue is edited, never joined by a new one. A new issue is filed only
  for work too large to carry in the task at hand, never to defer a fix the
  change could hold. A small fix found outside it is a line in the open
  batch issue, which may land as one pull request or several.

## Subagents

- **Subagents only where the operator granted them to that session**, the
  seat's standing grant in `roles/seat.md`; a relayed grant is none. An agent
  changes nothing in the repository and returns a proposal with its
  evidence.

## Host notes

- `chunbay-x86`: the sessions' host. The worktrees are
  `~/workspaces/SBS/XR-Touch_to_OSC` (the primary, on `production`),
  `XR-Touch_to_OSC-seat`, `XR-Touch_to_OSC-lane-a` and `XR-Touch_to_OSC-lane-b` beside it; the
  session root is `~/workspaces/SBS/draft/XR-Touch_to_OSC` and the memory store
  `~/obsidian-vault/SBS/.claude/memory/XR-Touch_to_OSC/`, both named in each
  worktree's `.claude/settings.local.json` (chun-harness
  `templates/settings.local.example.json`).
- **Each worktree builds in its own `target/`** with the host's stable
  toolchain (there is no pin) and its own `ui/node_modules`; `cargo test`
  and `npm run build` are a lane's loop while it works, and the check on its
  pull request is the verdict.
- **The target is a Windows PC with a touchscreen** in the studio, where
  the palette (`xr-touch-widget`, the Tauri shell under `app/`) floats over
  the demo page and sends OSC to the XR engine. The shell compiles on
  Windows: `.github/workflows/build.yml` builds the installer on every push
  to `main`, and `release.yml` publishes a Release on a `v*` tag, both kept
  beside the check. The installer is tried on the PC by the operator against
  `doc/windows-checklist.md`; `mock-xr` stands in for the engine on this box.
- The Windows PC and the XR engine are reached only through the operator;
  nothing in this tree holds an address of theirs that is not the design's.
- `.claude/`'s `skills/`, `hooks/`, `rules/` and `settings.json` are tracked,
  `.gitignore` the source. A hook or a rule promoted on `main` reaches a
  session when its worktree's checkout next takes `main`.
- **The memory store is outside git**, in the vault so it follows the operator
  between machines: every worktree's `autoMemoryDirectory` names it, so every
  session reads the one current store, and the seat alone writes it.
  `ARCHIVE.md` there holds memories kept but not indexed: read it before
  concluding none exists. The store's rules are `roles/seat.md`'s.
- **Every session record lives under the session root**, a report under
  `reports/<session name>/`; the root counts only while it holds
  `.harness-session-root` (chun-harness `doc/sessions.md`).
