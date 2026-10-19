#!/usr/bin/env python3
"""Rule 6 as a script: break each behaviour a test covers, run that test,
require it to fail, and put the code back. A test that still passes with its
behaviour broken does not cover it. (The Builder template's script, with
browser breaks added for the privacy kit.)

    python3 scripts/break_tests.py              every break in tests/breaks.json
    python3 scripts/break_tests.py blocked      the breaks whose names hold these words

tests/breaks.json is a list of objects:

    {"name": "the strict CSP keeps font-src 'none'",
     "file": "web/index.html",
     "find": "font-src 'none'; ",
     "replace": "",
     "test": "every_private_page_sets_the_strict_privacy_policy"}

A break with "test": "browser" rebuilds dist/ and runs the browser check in
Chromium; it is caught only when the check fails on a line holding "expect",
so a break that fails some other check does not count. The file is restored
after each break, whatever happens, and dist/ is rebuilt from the restored
source at the end. The script exits non-zero when a break leaves its test
passing, when its text is no longer in its file (the break has rotted), or
when the list is empty (a proof with nothing in it proves nothing).
"""
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
LIST = ROOT / "tests" / "breaks.json"
PY = os.environ.get("BUILDER_PYTHON") or str(ROOT / ".venv" / "bin" / "python")


def run_browser(expect):
    build = subprocess.run(["./scripts/build.sh"], cwd=ROOT, capture_output=True, text=True)
    if build.returncode != 0:
        return False, "the build failed, so the browser check never ran"
    env = {**os.environ, "BUILDER_ENGINES": "chromium"}
    run = subprocess.run([PY, "tests/browser/check.py"], cwd=ROOT, capture_output=True, text=True, env=env)
    fails = [l for l in run.stdout.splitlines() if l.startswith("  FAIL ")]
    hit = [l for l in fails if expect in l]
    if run.returncode != 0 and hit:
        return True, hit[0].strip()[:160]
    return False, f"exit {run.returncode}; failed lines: {fails[:3]}"


def main(words):
    breaks = json.loads(LIST.read_text())
    if words:
        breaks = [b for b in breaks if any(w in b["name"] for w in words)]
    if not breaks:
        sys.exit("no breaks to run: the proof would prove nothing")
    bad = []
    browser = False
    for b in breaks:
        path = ROOT / b["file"]
        original = path.read_text()
        if original.count(b["find"]) != 1:
            print(f"  ROTTED  {b['name']}: the text to break appears {original.count(b['find'])} times in {b['file']}")
            bad.append(b["name"])
            continue
        try:
            path.write_text(original.replace(b["find"], b["replace"]))
            if b["test"] == "browser":
                browser = True
                caught, why = run_browser(b["expect"])
            else:
                run = subprocess.run(
                    ["cargo", "test", "--workspace", "--quiet", b["test"]],
                    cwd=ROOT, capture_output=True, text=True,
                )
                caught, why = run.returncode != 0, ""
        finally:
            path.write_text(original)
        if caught:
            print(f"  caught  {b['name']}" + (f": {why}" if why else ""))
        else:
            print(f"  MISSED  {b['name']}: {b['test']} still passes with the behaviour broken {why}")
            bad.append(b["name"])
    if browser:
        subprocess.run(["./scripts/build.sh"], cwd=ROOT, capture_output=True, text=True)
    print(f"\n{len(breaks) - len(bad)} of {len(breaks)} breaks caught")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main(sys.argv[1:])
