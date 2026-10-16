---
paths:
  - "crates/**"
  - "app/**"
  - "Cargo.toml"
  - "ui/src/**"
  - "ui/*.html"
  - "**/*.sh"
---
# The rules of the code

Claude Code loads this file into a session when it works with a file the
`paths:` above name, so a lane meets these rules the moment it touches code
and a seat that only merges never pays for them. The rules that bind every session are `CLAUDE.md`'s;
the role files are `roles/`. Each rule here earned its place by a failure it
prevents, and one that stops earning it is taken out.

## 1. For the reader

- **Simple to read over simple to write.** Code a reader cannot follow in
  one pass is rewritten, not commented; "clever" is an accusation.
- **A name says what the thing is or does**, fully, and no longer than that.
  The language's casing rules decide the form.
- **A comment says why**, never what: a "what" that needs a comment is code
  to simplify. A regular expression or an algorithm the reader may not know
  is the exception. A file, and a function that is not both short and
  obvious, opens with a header stating purpose, inputs, outputs and failure.
- **A unit does one thing and fits one screen**: a function past about sixty
  lines, or a condition past two levels of nesting, is split. A variable is
  declared where it is used.
- **Match the file, then the repository**: consistency outranks taste, and
  the formatter's output is the style. Where no rule decides, pick one and
  stop.

## 2. One source of truth

- **A fact lives in one place**: a path, a port, a version, a unit name is
  written once and read from there. A list that could be derived is derived,
  never kept by hand.
- **Code carries no citation out**: a comment states the fact, and the
  document cites the code.
- **A dependency is costed from the tree** before it is added, and added
  only for what is used now.

## 3. Units of change

- **One change does one thing**, with its tests in the same commit: about a
  hundred lines is usual, a thousand is refused, and a refactor never rides
  with a behaviour change. When in doubt, smaller.
- **Solve the problem that exists.** No generality, configuration or
  flexibility for a case nobody has; the next case brings its own change.
- **Take out before adding**: a fix lands at its cause, and what a change
  makes unused leaves with it. Dead code it finds is reported, not deleted.

## 4. Errors and results

- **Every result is checked**: a return value, an exit code, a `Result`. One
  that may be ignored is ignored where that is said, never silently.
- **A failure is loud and placed**: the message names what was being done
  and what was found, goes to stderr or into the error type, and never
  falls back to a weaker default on its own.
- **Input from outside the program is validated at the boundary**: a number
  is numeric, a name is on its allowlist, a path is not a root that must not
  be touched, and nothing read from a file or a person is executed.

## 5. Tests

- **A behaviour is pinned by a test that fails when the code breaks**, and
  the test names the ablation that turns it red. Its assertions are simple;
  a test is code and is held to these rules.
- **A test injects what it touches**: paths, hosts, commands and time come
  in as parameters, so the check runs on a runner with no device.
- **A test that needs a host runs on the operator's word** and is not part
  of the check.

## 6. What the check enforces

The formatter, the linter and the tests run on every pull request and every
push to `main` and `production` (`scripts/check.sh`); a session never runs
them as the verdict, and a human review spends itself on design, names and
why, not on style. A finding is relaxed on the one item it fires on, with
its reason beside it, never file-wide.

## 7. Bash

- Bash only, for a small utility or a wrapper; a script past about a hundred
  lines, or whose control flow is not straight, is a program in another
  language.
- `#!/usr/bin/env bash` and `set -euo pipefail`; a loop that must outlive a
  failing command drops `-e` on purpose and says so at the top.
- `[[ … ]]` for tests, `(( … ))` for arithmetic, `$(…)` for substitution;
  never `eval`, backticks, `let` or `expr`.
- **Quote every expansion**: `"${var}"`, `"$@"`, `"${array[@]}"`; a list is
  an array, never a space-joined string.
- `local` for every function variable, declared and assigned on separate
  lines when the value is a command substitution: `local x=$(cmd)` hides
  the exit code.
- Never pipe into `while`: a pipe is a subshell and its variables die with
  it; use process substitution or `readarray`.
- Errors go to stderr through one helper that adds what was being done; a
  script with functions ends with `main "$@"`; constants are `readonly` and
  declared at the top.
- Prefer builtins and parameter expansion to `sed`, `awk` and `echo |` for
  one value.
- `shellcheck --severity=warning` and `shfmt -i 2 -ci -d` are clean (the
  check); a directive sits on the one line it excuses, with its reason.

## 8. Rust

- `rustfmt` is the style, and `cargo clippy --workspace --all-targets --
  -D warnings` is clean; a lint is relaxed with
  `#[expect(clippy::<lint>, reason = "…")]` on the one item, so the
  suppression ends when the lint stops firing.
- A workspace splits the pure logic from the front end: the core crate
  knows no terminal, socket or screen, and everything shown comes out of it
  as data.
- Errors are a type with meaning (`C-GOOD-ERR`): one enum per crate with a
  `From` for each cause and a `Result` alias; `unwrap` and `expect` only in
  tests and on values the program itself produced.
- Names follow RFC 430 (`C-CASE`) and conversions are `as_`, `to_`,
  `into_` (`C-CONV`); public types implement `Debug` (`C-DEBUG`) and keep
  their fields private (`C-STRUCT-PRIVATE`); an argument is a custom type,
  not a bare `bool` or `Option` (`C-CUSTOM-TYPE`); a function validates its
  arguments (`C-VALIDATE`).
- A doc comment states errors, panics and safety (`C-FAILURE`); an example
  uses `?`, never `unwrap` (`C-QUESTION-MARK`).
- No `unsafe` block without a `D` number and a `// SAFETY:` comment stating
  why it is sound.

## 9. Web (Svelte, Vite)

- `package-lock.json` is committed and the check installs with `npm ci`;
  `npm run build` is clean, and `npm run check` where the project has it.
- The page is served by the project's own server and reads nothing from
  another box in the browser: the server does, and the page shows.

## 10. This repository

- `xrt-core` knows no window and no screen: configuration, the OSC
  encoding, the HTTP GET helper, the network and the heartbeat live there and
  are tested there; `xrt-app` (the Tauri shell) exposes them as commands and
  holds the engine thread; the UI calls the commands and holds no logic of
  its own.
- `xrt-app` builds in two forms, installed and portable (`--features
  portable`, the config beside the exe), and never decides at run time by a
  marker file.
- `mock-xr` is the engine's stand-in: a test that needs an engine runs
  against it (`crates/mock-xr/tests/loopback.rs`), never against the
  studio's.
- The palette is stateless: a button fires a trigger and shows nothing of
  the engine's state; the active target is the one setting it keeps.
- `ui/src/shared/tokens.css` is the one place a colour or a size is named;
  a component uses a token.
