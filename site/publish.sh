#!/usr/bin/env bash
# Bygger sajten och publicerar den på GitHub Pages (grenen gh-pages).
#
#   bash site/publish.sh
#
# Kräver node + npm och push-rätt till repot.
set -euo pipefail
cd "$(dirname "$0")"
root="$(cd .. && pwd)"
remote="$(git -C "$root" remote get-url origin)"
name="$(git -C "$root" config user.name || echo sonix)"
mail="$(git -C "$root" config user.email || echo sonix@localhost)"

npm install --no-audit --no-fund
npx next build

# Pages ska servera filerna som de är, inte köra dem genom Jekyll.
touch out/.nojekyll

tmp="$(mktemp -d)"
cp -r out/. "$tmp/"
git -C "$tmp" init -q
git -C "$tmp" checkout -q -b gh-pages
git -C "$tmp" add -A
git -C "$tmp" -c user.name="$name" -c user.email="$mail" commit -q -m "site: publicera GitHub Pages"
git -C "$tmp" push -q --force "$remote" gh-pages
rm -rf "$tmp"
echo "Publicerad till https://alexwest1981.github.io/sonix/"
