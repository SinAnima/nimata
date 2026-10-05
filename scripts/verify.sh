#!/usr/bin/env bash
# Runs every check CI runs, in the same way, stopping at the first failure.
# CI calls this script too, so a local pass means the CI checks pass.
#
# Usage: scripts/verify.sh        (or: npm run verify, mise run verify)

set -euo pipefail

cd "$(dirname "$0")/.."

in_ci="${GITHUB_ACTIONS:-}"
started=$SECONDS
step_number=0

step() {
  local name="$1"
  shift
  step_number=$((step_number + 1))
  local step_started=$SECONDS
  if [ -n "$in_ci" ]; then
    echo "::group::$name"
  else
    printf '\n\033[1m[%d] %s\033[0m\n    $ %s\n' "$step_number" "$name" "$*"
  fi

  if ! "$@"; then
    [ -n "$in_ci" ] && echo "::endgroup::"
    printf '\n\033[1;31mFAILED: %s\033[0m\n' "$name"
    printf 'Rerun just this step with:\n    %s\n' "$*"
    [ -n "$in_ci" ] && echo "::error::$name failed"
    exit 1
  fi

  [ -n "$in_ci" ] && echo "::endgroup::"
  printf '    ok (%ds)\n' $((SECONDS - step_started))
}

if [ ! -d node_modules ]; then
  echo "node_modules is missing; run 'npm ci' first." >&2
  exit 1
fi

# TypeScript and Node also resolve packages from node_modules folders in
# parent directories. A dependency found there makes a check pass locally but
# fail on a clean CI checkout, so point it out.
dir="$(dirname "$PWD")"
while [ "$dir" != "/" ]; do
  if [ -d "$dir/node_modules" ]; then
    printf '\033[1;33mWarning:\033[0m %s/node_modules exists. Packages there can hide a\n' "$dir"
    printf 'missing dependency here; CI will not have them.\n'
  fi
  dir="$(dirname "$dir")"
done

# Fast checks first, so simple mistakes fail within seconds.
step "Rust formatting" cargo fmt --all --check
step "Frontend formatting" npm run --silent format:check
step "Workflow files" actionlint
step "Frontend build" npm run --silent build
step "Type check" npm run --silent check
step "Clippy" cargo clippy --workspace --all-targets -- -D warnings
step "Frontend tests with coverage" npm run --silent coverage
step "Rust tests with coverage" \
  cargo llvm-cov --workspace --fail-under-lines 90 --lcov --output-path coverage/rust.lcov

printf '\n\033[1;32mAll %d checks passed in %ds.\033[0m\n' "$step_number" $((SECONDS - started))
