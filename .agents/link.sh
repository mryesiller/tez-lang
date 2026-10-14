#!/usr/bin/env bash
# Wires `.agents/` into tools that look in tool-specific locations.
# `.agents/` stays the single source of truth; everything created here is a symlink
# (and is git-ignored), so nothing is duplicated.
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p .claude
for d in agents skills commands; do
  ln -sfn "../.agents/$d" ".claude/$d"
done
ln -sfn ".agents/mcp.json" ".mcp.json"
echo "linked: .claude/{agents,skills,commands} and .mcp.json -> .agents/"
