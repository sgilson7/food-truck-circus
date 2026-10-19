#!/usr/bin/env bash
# Optional audit, outside check.sh (kits/privacy): every planted leak must be
# caught. Plants each leak from scripts/plant_leak.py in a fresh copy and
# requires the boundary tests or the browser check (Chromium) to fail. The
# default idle is kept: with one second the game's first show fails on timing.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="${BUILDER_PYTHON:-$ROOT/.venv/bin/python}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/prove-privacy.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

leaks=$(python3 "$ROOT/scripts/plant_leak.py" --list) || { echo "could not list the leaks"; exit 1; }
# An empty list would pass by proving nothing.
[ -n "$leaks" ] || { echo "no leaks to plant: the proof would prove nothing"; exit 1; }

missed=0
for leak in $leaks; do
  # --delete refreshes the copy from the repository; target/ is kept between
  # leaks so only the first one compiles from scratch.
  rsync -a --delete --exclude .git --exclude .venv --exclude target --exclude dist "$ROOT/" "$WORK/copy/"
  python3 "$ROOT/scripts/plant_leak.py" "$WORK/copy" "$leak" || { echo "could not plant $leak"; missed=1; continue; }
  caught=""
  (cd "$WORK/copy" && cargo test -q --test boundary >/dev/null 2>&1) || caught="core tests"
  if (cd "$WORK/copy" && ./scripts/build.sh >/dev/null 2>&1); then
    out=$(cd "$WORK/copy" && BUILDER_ENGINES=chromium "$PY" tests/browser/check.py 2>&1)
    if ! grep -q "BROWSER CHECK PASSED" <<<"$out"; then
      caught="${caught:+$caught; }browser check ($(grep -m1 '  FAIL ' <<<"$out" | cut -c8-90))"
    fi
  else
    echo "  the build of the copy failed for $leak"
  fi
  if [ -n "$caught" ]; then echo "  caught $leak by $caught"; else echo "  MISSED $leak"; missed=1; fi
done
[ "$missed" = 0 ] && echo "PRIVACY PROOF PASSED" || { echo "PRIVACY PROOF FAILED"; exit 1; }
