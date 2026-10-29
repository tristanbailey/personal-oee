#!/bin/bash
# Smoke test: build the site and check the output is sane. Run via `composer test`.
set -u
cd "$(dirname "$0")/.."

./vendor/bin/jigsaw build production || { echo "FAIL: jigsaw build"; exit 1; }

out=build_production
fail=0

for f in index.html blog/index.html contact/index.html contact/thank-you/index.html \
         gdpr/what-is-gdpr/index.html sitemap.xml blog/feed.rss; do
    [ -s "$out/$f" ] || { echo "FAIL: missing or empty $out/$f"; fail=1; }
done

# PHP errors leaked into generated pages
if grep -rlE 'Undefined (variable|index|array key|offset)|Fatal error:|Parse error:|Stack trace:' "$out" --include='*.html' | head -5 | grep .; then
    echo "FAIL: PHP errors in the files above"
    fail=1
fi

# Broken local images are reported but do not fail the run (profile photos are known to be missing)
broken=0
while read -r p; do
    [ -e "source$p" ] || broken=$((broken + 1))
done < <(grep -rhoE '/img/[A-Za-z0-9_./-]+\.(png|jpe?g|gif|svg)' "$out" --include='*.html' | sort -u)
echo "note: $broken referenced local images are missing from source/img"

[ "$fail" -eq 0 ] && echo "OK: smoke test passed"
exit "$fail"
