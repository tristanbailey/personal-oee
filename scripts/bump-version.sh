#!/bin/sh
# Raise the patch version by 0.0.1 (NFR-003): Cargo.toml (every crate, the app
# and `office-oee version`) and the app bundle's version in bundle.sh.
# Minor and major releases are chosen by the owner and edited by hand.
set -e
cd "$(dirname "$0")/.."
CURRENT=$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)"$/\1/p' Cargo.toml | head -1)
[ -n "$CURRENT" ] || { echo "No version found in Cargo.toml" >&2; exit 1; }
MAJOR=${CURRENT%%.*}; REST=${CURRENT#*.}; MINOR=${REST%%.*}; PATCH=${REST#*.}
NEXT="$MAJOR.$MINOR.$((PATCH + 1))"
sed -i '' "s/^version = \"$CURRENT\"\$/version = \"$NEXT\"/" Cargo.toml
sed -i '' "s|<string>$CURRENT</string>|<string>$NEXT</string>|g" scripts/bundle.sh
cargo update --workspace --offline >/dev/null 2>&1 || cargo build --workspace >/dev/null
echo "$CURRENT -> $NEXT"
