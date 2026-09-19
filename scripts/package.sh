#!/bin/sh
# Build a self-contained archive; run from any directory.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"
npm ci --prefix ui --ignore-scripts
cargo build --release --locked
version=$(./target/release/agentic-judge --version | awk '{print $2}')
platform="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
name="agentic-judge-$version-$platform"
mkdir -p dist
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT HUP INT TERM
mkdir "$staging/$name"
cp target/release/agentic-judge scripts/install.sh "$staging/$name/"
cp vendor/testlib/LICENSE "$staging/$name/TESTLIB-LICENSE"
cp README.md "$staging/$name/README.md"
tar -czf "dist/$name.tar.gz" -C "$staging" "$name"
shasum -a 256 "dist/$name.tar.gz" > "dist/$name.tar.gz.sha256"
printf 'Created dist/%s.tar.gz\n' "$name"
