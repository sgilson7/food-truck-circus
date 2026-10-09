"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. These drive the game the way a person would and assert what the page
shows. Expected values are worked out by hand from the data files (C4), never
copied from the game's own output:

- missions.json lists 26 missions; only m01 has an empty `after`, so a new
  player sees 26 nodes and exactly one open.
- m01 is at the venue "rtp", whose name in venues.json is "RTP 800 Building".
- costumes.json has 12 costumes; 11 are named in some mission's `unlock`
  (all but "cook"), so a new player can wear one and 11 are disabled.
- m01 asks for 3 m three times. The bed's drive adds about 1.2 m/s a bounce
  (balance.rs: 0.006 m/tick² over a nine-tick push), so holding Space passes
  it in well under fifteen seconds.
- m02, m03, m04 and m06 each come only after m01: passing m01 leaves five
  shows open (m01 itself and those four). The first draft of this check said
  three, from counting only m02 and m03 (SECOND-ORDER row W3).
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
COPY = json.loads((ROOT / "data" / "copy.json").read_text())


def click(page, selector):
    # a click from the page's own script: Playwright's pointer click scrolls
    # the overlay mid-click in some engines (SECOND-ORDER row W2)
    page.evaluate("s => document.querySelector(s).click()", selector)
    page.wait_for_timeout(200)


def run(page, check, engine):
    check(page.text_content("#map h1") == COPY["title"], "the title from data/copy.json is drawn")
    message = page.inner_text("#message")
    check(message in ("", COPY["no_webgl"]), f"no error message is shown ({message!r})")
    check(page.is_visible("#map"), "the game opens on the tour map")

    nodes = page.query_selector_all("#nodes .node")
    open_nodes = page.query_selector_all("#nodes .node:not(.locked)")
    check(len(nodes) == 26, f"the map shows the 26 missions ({len(nodes)})")
    check(len(open_nodes) == 1, f"a new tour has one show open ({len(open_nodes)})")
    check("RTP 800 Building" in page.inner_text("#b-venue"), "the first show is at the RTP 800 Building")
    costumes = page.query_selector_all("#costumes .costume")
    locked = page.query_selector_all("#costumes .costume:disabled")
    check(len(costumes) == 12 and len(locked) == 11, f"12 costumes, 11 still to earn ({len(costumes)}, {len(locked)})")

    click(page, "#nodes .node:nth-child(2)")
    check(page.is_disabled("#perform"), "a locked show cannot be started")
    check(page.is_visible("#b-locked"), "a locked show says why")
    click(page, "#nodes .node:nth-child(1)")
    check(page.is_enabled("#perform"), "the first show can be started")

    # the first show: hold Space until the result comes up
    click(page, "#perform")
    page.wait_for_timeout(300)
    check(page.is_hidden("#map") and page.is_visible("#hud"), "performing hides the map and shows the HUD")
    page.keyboard.down("Space")
    passed = False
    for _ in range(40):
        page.wait_for_timeout(500)
        if page.is_visible("#result"):
            passed = True
            break
    page.keyboard.up("Space")
    check(passed, "holding Space passes the first show")
    if passed:
        check(page.text_content("#r-title") == COPY["result_pass"], "the result says the show is done")

    # back to the map: three shows open now
    click(page, "#r-map")
    page.wait_for_timeout(300)
    open_nodes = page.query_selector_all("#nodes .node:not(.locked)")
    check(len(open_nodes) == 5, f"passing the first show opens the four after it ({len(open_nodes)})")

    # the tour survives a reload
    page.reload()
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.wait_for_timeout(500)
    open_nodes = page.query_selector_all("#nodes .node:not(.locked)")
    check(len(open_nodes) == 5, f"the tour is kept across a reload ({len(open_nodes)})")

    # pause and resume
    click(page, "#nodes .node:nth-child(3)")
    click(page, "#perform")
    page.wait_for_timeout(300)
    page.keyboard.press("Escape")
    page.wait_for_timeout(200)
    check(page.is_visible("#paused"), "Escape pauses the show")
    click(page, "#resume")
    check(page.is_hidden("#paused"), "the show resumes")
