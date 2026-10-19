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
    # a real pointer click: a node that moves under the pointer on press
    # loses its click, which a scripted click would hide (SECOND-ORDER W5)
    page.click(selector, timeout=5000)
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

    run_more(page, check, engine)


def run_more(page, check, engine):
    """The postcards, the glossary, juggling's first throw and the Graph Lab.

    By hand: tricks.json has 31 distinct trick ids with points (35 rows, of
    which to_feet appears three times and seat_drop twice, and to_feet scores
    nothing), so the glossary shows 31 moves. 2x+1 at x = -2 is -3. Juggling's
    hint changes from copy.juggle_start to copy.juggle_hint on the first press.
    """
    page.keyboard.press("Escape")
    page.wait_for_timeout(200)
    click(page, "#pause-map")
    page.wait_for_timeout(300)
    loaded = page.evaluate("() => { const i = document.getElementById('b-art'); return i.complete && i.naturalWidth > 0; }")
    check(loaded, "the selected show's postcard loads")

    click(page, "#brief .open-moves")
    cards = page.query_selector_all("#move-list .move")
    check(len(cards) == 31, f"the glossary lists the 31 moves ({len(cards)})")
    check(page.is_visible("#moves"), "Moves opens")
    click(page, "#moves-close")
    check(page.is_hidden("#moves"), "Moves closes")

    # juggling: the balls wait for the first press
    click(page, "#nodes .node:nth-child(6)")
    click(page, "#perform")
    page.wait_for_timeout(600)
    check(page.text_content("#hint") == COPY["juggle_start"], "juggling waits for the first throw")
    page.keyboard.press("KeyD")
    page.wait_for_timeout(300)
    check(page.text_content("#hint") == COPY["juggle_hint"], "the first press starts the juggling")
    badges = page.query_selector_all("#hands .hand")
    check(len(badges) == 2, f"a key badge under each hand ({len(badges)})")

    # the Graph Lab
    base = page.url.rsplit("/", 1)[0]
    page.goto(base + "/lab.html")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.wait_for_timeout(500)
    rows = page.query_selector_all("#machine tbody tr")
    check(len(rows) == 9, f"the lab opens on nine rows of x² ({len(rows)})")
    check(page.is_hidden("#blocked"), "a normal load of the lab shows no blocked-device message")
    page.fill("#formula", "2x+1")
    page.press("#formula", "Enter")
    page.wait_for_timeout(300)
    first = page.text_content("#machine tbody tr:nth-child(1) td:nth-child(2)")
    check(first == "-3", f"2x+1 at x = -2 is -3 ({first!r})")
    page.fill("#formula", "2x+")
    page.press("#formula", "Enter")
    page.wait_for_timeout(300)
    check(page.text_content("#error") == COPY["lab"]["expr"]["end"], "a formula that stops short says so")
    page.fill("#formula", "x^2")
    page.press("#formula", "Enter")
    click(page, "#run-auto")
    # the first point of x² is its highest: a few set-up bounces come first
    page.wait_for_timeout(18000)
    marked = [c.text_content() for c in page.query_selector_all("#machine tbody td:nth-child(5)")]
    check(sum(1 for m in marked if m) >= 1, f"the performer has marked a point after eighteen seconds ({marked})")
    lit = page.query_selector_all("#tree li.lit")
    check(len(lit) >= 1, "the tree shows what is running")

    run_labs(page, check, base)


def run_labs(page, check, base):
    """The popcorn and juggling labs.

    By hand: 441 adds to 9 over 3 beats, an average of 3. 43 adds to 7 over 2
    beats, an average of 3.5. A jump off the 6 m ledge lands on the bed and
    bounces on to the grass: two arcs.
    """
    lab = COPY["lab"]
    # the same page: switch labs with the tabs, as a person would
    click(page, "#tabs button:nth-child(3)")
    page.wait_for_timeout(600)
    want = lab["jug_ok"].replace("{digits}", "4 + 4 + 1").replace("{sum}", "9").replace("{n}", "3").replace("{balls}", "3")
    check(page.text_content("#jug-explain") == want, f"441 is three balls ({page.text_content('#jug-explain')!r})")
    page.fill("#jug-pattern", "43")
    page.press("#jug-pattern", "Enter")
    page.wait_for_timeout(300)
    want = lab["jug_average"].replace("{sum}", "7").replace("{n}", "2").replace("{avg}", "3.5")
    check(page.text_content("#jug-explain") == want, f"43 averages 3.5 balls ({page.text_content('#jug-explain')!r})")

    click(page, "#tabs button:nth-child(2)")
    page.wait_for_timeout(300)
    check(page.is_visible("#fly-jump"), "the popcorn lab's tab shows its panel")
    click(page, "#fly-jump")
    page.wait_for_timeout(9000)
    arcs = page.query_selector_all("#fly-arcs li")
    check(len(arcs) >= 2, f"the flight has two arcs, off the ledge and off the bed ({len(arcs)})")
    summary = page.text_content("#fly-summary")
    check("kernels" in summary and "m²" in summary, f"the popcorn is counted ({summary!r})")

    run_french(page, check, base)


def run_french(page, check, base):
    """French: the button on the lab page switches every word, and the
    choice carries to the game. By hand: x² at -1.5 is 2.25, written 2,25;
    the first show is "Trouve ton rebond" in data/fr/missions.json."""
    fr = json.loads((ROOT / "data" / "fr" / "copy.json").read_text())
    frm = json.loads((ROOT / "data" / "fr" / "missions.json").read_text())
    click(page, "#lang")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.wait_for_timeout(600)
    check(page.text_content("#panel h1") == fr["lab"]["title"], "the lab's title is in French")
    click(page, "#tabs button:nth-child(1)")
    page.wait_for_timeout(400)
    cell = page.text_content("#machine tbody tr:nth-child(2) td:nth-child(2)")
    check(cell == "2,25", f"numbers have decimal commas ({cell!r})")
    page.goto(base + "/index.html")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.wait_for_timeout(600)
    check(page.text_content("#map h2") == fr["map_heading"], "the game remembers French")
    click(page, "#nodes .node:nth-child(1)")
    check(page.text_content("#b-name") == frm["missions"]["m01"]["name"], "the first show's name is French")
    src = page.evaluate("() => document.getElementById('b-art').getAttribute('src')")
    check("/fr/" in src, f"the postcard is the French one ({src})")
    click(page, "#map .lang-switch")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    page.wait_for_timeout(400)
    check(page.text_content("#map h2") == COPY["map_heading"], "and back to English")


# How a school's device policy can stop the lab, imitated by init scripts:
# Chrome's DefaultJavaScriptJitSetting=block can leave WebAssembly undefined
# or leave instantiate refusing; Disable3DAPIs leaves getContext('webgl2')
# returning null.
REFUSE_WASM = (
    "WebAssembly.instantiate = () => Promise.reject(new WebAssembly.CompileError('blocked'));"
    " WebAssembly.instantiateStreaming = () => Promise.reject(new WebAssembly.CompileError('blocked'));"
)
NO_WEBGL2 = (
    "const original = HTMLCanvasElement.prototype.getContext;"
    " HTMLCanvasElement.prototype.getContext = function (kind, ...rest) {"
    " return kind === 'webgl2' ? null : original.call(this, kind, ...rest); };"
)
BLOCKS = [
    ("WebAssembly is undefined", "delete window.WebAssembly;", "blocked_wasm"),
    ("WebAssembly refuses to instantiate", REFUSE_WASM, "blocked_wasm"),
    ("WebGL 2 cannot be created", NO_WEBGL2, "blocked_webgl"),
]


def run_blocked(browser, base, check, engine):
    """A blocked device sees a plain message over the lab's stage, not a blank
    page. By hand: the message is the copy string word for word; without WebGL
    the panel still opens on the nine rows of x²."""
    for how, script, key in BLOCKS:
        context = browser.new_context()
        context.add_init_script(script)
        page = context.new_page()
        page.goto(base + "/lab.html")
        page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
        shown = page.is_visible("#blocked") and page.inner_text("#blocked") == COPY["lab"][key]
        check(shown, f"when {how}, the lab says what is blocked and who can allow it")
        if key == "blocked_webgl":
            page.wait_for_timeout(500)
            rows = len(page.query_selector_all("#machine tbody tr"))
            check(rows == 9, f"when {how}, the panel still opens on nine rows of x² ({rows})")
        context.close()
