#!/usr/bin/env bash
#
# The project's check: every gate over the whole tree, red on the first that
# fails. The runner runs it for every pull request and every push to main or
# production (.github/workflows/ci.yml); a hand run only diagnoses. Run it
# after staging: it reads tracked files. Read its exit code directly, never
# through a pipe.
#
#   scripts/check.sh
#
# Gates, in order, the cheap ones first:
#   lint, format, exec-bits, hooks   the scripts and the hook cases
#   fmt                              cargo fmt over the workspace, the Tauri shell included
#   clippy, test                     xrt-core and mock-xr; the shell is Windows' (build.yml)
#   ui                               ui/: npm ci, npm run build
#
# It writes no gate record: scripts/publish.sh says so on one line when it
# moves production, and publishes on reachability alone.
set -euo pipefail
cd "$(dirname "$0")/.."

# A script is a tracked file whose first line is a bash, sh or dash shebang;
# zsh is not shellcheck's, and a shebang quoted inside a document does not
# count.
scripts=()
while IFS= read -r -d '' f; do
  if [[ $(head -n 1 -- "$f" 2>/dev/null | tr -d "\0") =~ ^#!(/[^[:space:]]*/|[[:space:]]*/usr/bin/env[[:space:]]+)(ba|da)?sh([[:space:]]|$) ]]; then scripts+=("$f"); fi
done < <(git ls-files -z | sort -z)
echo "shellcheck: ${#scripts[@]} scripts"
if [ "${#scripts[@]}" -gt 0 ]; then
  shellcheck --severity=warning "${scripts[@]}"
  echo "shfmt"
  shfmt -i 2 -ci -d "${scripts[@]}"
fi

echo "exec-bits"
python3 scripts/check-script-exec-bits.py

echo "hooks"
python3 scripts/check-hooks.py

echo "fmt"
cargo fmt --all --check

# xrt-app, the Tauri shell, needs webkit2gtk to compile on Linux and carries
# no tests: the Windows job of .github/workflows/build.yml compiles it.
echo "clippy"
cargo clippy -p xrt-core -p mock-xr --all-targets -- -D warnings

echo "test"
cargo test -p xrt-core -p mock-xr

echo "ui"
(cd ui && npm ci --silent && npm run build --silent)

echo "green"
