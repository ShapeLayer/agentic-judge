#!/bin/sh
set -eu
PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$PROJECT_ROOT"
for tool in cargo npm node python3 clang++; do
    command -v "$tool" >/dev/null 2>&1 || { echo "필요한 프로그램이 없습니다: $tool" >&2; exit 1; }
done
if [ ! -f ui/node_modules/typescript/bin/tsc ]; then
    npm ci --prefix ui
fi
cargo build --locked
mkdir -p solutions
target/debug/agentic-judge doctor --config config/study.toml
python3 scripts/seed-study.py
exec target/debug/agentic-judge serve --config config/study.toml "$@"
