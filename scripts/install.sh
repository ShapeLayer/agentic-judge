#!/bin/sh
# Install from an extracted release archive. No repository or Rust toolchain needed.
set -eu
prefix="${HOME}/.local"
if [ "${1:-}" = "--prefix" ] && [ "$#" -eq 2 ]; then
  prefix=$2
elif [ "$#" -ne 0 ]; then
  echo "Usage: $0 [--prefix /absolute/install/path]" >&2
  exit 2
fi
case "$prefix" in /*) ;; *) echo 'Install prefix must be absolute' >&2; exit 2;; esac
release_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
test -f "$release_dir/agentic-judge" || { echo 'Run install.sh from an extracted release archive' >&2; exit 1; }
mkdir -p "$prefix/bin" "$prefix/share/agentic-judge"
# Rename permits upgrades while the previous executable is running.
temporary="$prefix/bin/.agentic-judge-$$"
trap 'rm -f "$temporary"' EXIT HUP INT TERM
cp "$release_dir/agentic-judge" "$temporary"
chmod 755 "$temporary"
mv -f "$temporary" "$prefix/bin/agentic-judge"
cp "$release_dir/TESTLIB-LICENSE" "$prefix/share/agentic-judge/TESTLIB-LICENSE"
printf 'Installed %s/bin/agentic-judge\nAdd %s/bin to PATH, then run agentic-judge.\n' "$prefix" "$prefix"
