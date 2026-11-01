#!/bin/bash
# Follow a Claude JSONL log in readable form.
# Usage: tasks/ralph-tail.sh [file.jsonl]
# With no argument: newest logs/*.jsonl, else the newest Claude session transcript for this project.
cd "$(dirname "$0")/.." || exit 1
command -v jq >/dev/null || { echo "jq not found: brew install jq"; exit 1; }

f="${1:-$(ls -t logs/*.jsonl 2>/dev/null | head -1)}"
[ -z "$f" ] && f=$(ls -t ~/.claude/projects/-Users-tristanbailey-code-personal-oee/*.jsonl 2>/dev/null | head -1)
[ -z "$f" ] && { echo "No log found"; exit 1; }

echo "Following $f (Ctrl-C to stop)"
tail -n +1 -F "$f" | jq -R -r --unbuffered -f tasks/ralph-log.jq
