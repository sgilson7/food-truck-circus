#!/usr/bin/env python3
"""Browser check: the built page in Chromium, Firefox and WebKit.

For each engine it serves dist/ on 127.0.0.1, opens index.html, waits for the
page to set <body data-ready="true">, lets it idle, and then runs the project's
own interactions from tests/browser/interactions.py. The run fails on:

- an uncaught page error;
- a console error;
- any request to another origin, during load, idle or the interactions;
- a Content-Security-Policy violation (a blocked attempt is still an attempt);
- a WebSocket, a popup, a worker or a service worker opened by the page;
- a failed check inside the interactions;
- (privacy kit, D7 tightened) any request after the page reported ready,
  same-origin included (blob: and data: stay in the browser and are allowed);
- (privacy kit, D7 tightened) a cookie, sessionStorage, IndexedDB, or any
  localStorage key other than the ones the brief declares (STORAGE_KEYS).

What a pass establishes is what happened on that path, in those engines, on
that run. It does not show that no feature can ever reach the network: a
feature the interactions never start is a feature this check never saw.

    python3 tests/browser/check.py

Environment:
    BUILDER_ENGINES   space-separated engines (default "chromium firefox webkit")
    BUILDER_IDLE      seconds to idle after load (default 10)
    BUILDER_ORIGIN    check a deployed origin instead of serving dist/, e.g.
                      https://example.github.io/my-tool  (no trailing slash)
"""
import functools
import http.server
import importlib.util
import os
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
ENGINES = os.environ.get("BUILDER_ENGINES", "chromium firefox webkit").split()
IDLE_MS = int(float(os.environ.get("BUILDER_IDLE", "10")) * 1000)

failures = []

# The privacy kit's storage rule, tightened for this project (SECOND-ORDER
# row K1, BUILDER-SPEC change table): both pages keep the language choice
# (row F1), and the game page keeps the tour (PLAN), so storage is allowed for
# exactly these keys and nothing else. This is the one place they are listed.
STORAGE_KEYS = {
    "food-truck-circus.lang",  # the language, chosen on either page
    "food-truck-circus.tour",  # the game page's progress; the lab never writes it
}

# Pages the after-ready rule covers: the kit is applied to the Circus Math
# Lab, the page students use in class (TRIAGE T1). The game is unchanged.
PRIVATE_PAGES = {"lab.html"}

# Reports, as a console.debug line, the moment a document sets
# <body data-ready="true">, so a page that reloads itself starts a new
# before-ready phase. (A Playwright binding here made pointer clicks hang in
# Chromium; a console line is a plain event.)
READY_MARK = "builder: the page reported ready"
READY_SCRIPT = (
    "new MutationObserver(() => {"
    " if (!window.__builderReadySent && document.body && document.body.dataset.ready === 'true') {"
    " window.__builderReadySent = true; console.debug('" + READY_MARK + "'); } })"
    ".observe(document, {subtree: true, attributes: true, attributeFilter: ['data-ready']});"
)

STORAGE_SCRIPT = """async () => {
    const keys = (s) => { const out = []; for (let i = 0; i < s.length; i++) out.push(s.key(i)); return out; };
    const idb = indexedDB.databases ? (await indexedDB.databases()).map((d) => d.name) : [];
    return {cookie: document.cookie, local: keys(localStorage), session: keys(sessionStorage), idb};
}"""


def page_name(url):
    path = url.split("#", 1)[0].split("?", 1)[0]
    name = path.rsplit("/", 1)[-1]
    return name or "index.html"


def private(url):
    return PRIVATE_PAGES is None or page_name(url) in PRIVATE_PAGES


def watch_after_ready(context, page):
    """Records every request a private page makes after it reported ready.

    A navigation the test starts (goto, reload) opens a new before-ready
    phase. A navigation the page starts itself is allowed only as a plain
    reload of the same address with no query string, which carries no data;
    any other is recorded."""
    state = {"ready": False, "doc": "", "test_nav": False}
    after = []

    def on_ready(message):
        if message.type == "debug" and message.text == READY_MARK:
            state["ready"] = True

    page.on("console", on_ready)
    context.add_init_script(READY_SCRIPT)

    def on_request(r):
        url = r.url
        if url.startswith(("blob:", "data:")):
            return
        main_nav = r.is_navigation_request() and r.frame == page.main_frame
        if main_nav:
            plain_reload = "?" not in url and url.split("#", 1)[0] == state["doc"]
            if state["test_nav"] or not state["ready"] or plain_reload:
                state.update(ready=False, doc=url.split("#", 1)[0], test_nav=False)
                return
        if state["ready"] and private(state["doc"]):
            after.append(url)

    context.on("request", on_request)
    for name in ("goto", "reload", "go_back", "go_forward"):
        original = getattr(page, name)

        def wrapped(*args, _original=original, **kwargs):
            state["test_nav"] = True
            return _original(*args, **kwargs)

        setattr(page, name, wrapped)
    return after


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def log_message(self, *args):
        pass


def serve():
    handler = functools.partial(Handler, directory=str(DIST))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def load_interactions():
    path = pathlib.Path(__file__).with_name("interactions.py")
    spec = importlib.util.spec_from_file_location("interactions", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_engine(p, engine, base, interactions):
    print(f"\n[{engine}] {base}/")
    scheme_host = base.split("/", 3)
    origin = "/".join(scheme_host[:3])
    seen = {"foreign": [], "console": [], "errors": [], "sockets": [], "popups": [], "workers": []}

    def same_origin(url):
        return url.startswith(origin + "/") or url.startswith("blob:" + origin) or url.startswith("data:")

    browser = getattr(p, engine).launch()
    context = browser.new_context(accept_downloads=True)
    context.on("request", lambda r: None if same_origin(r.url) else seen["foreign"].append(r.url))

    def route(r):
        if same_origin(r.request.url):
            r.continue_()
        else:
            r.abort()

    context.route("**/*", route)
    # A request the page's CSP blocks never becomes a request event, so the
    # violation itself is recorded and reported.
    context.add_init_script(
        "window.__builderCsp = [];"
        "document.addEventListener('securitypolicyviolation',"
        " e => window.__builderCsp.push(e.violatedDirective + ' ' + e.blockedURI));"
    )
    page = context.new_page()
    after_ready = watch_after_ready(context, page)
    page.on("console", lambda m: seen["console"].append(m.text) if m.type == "error" else None)
    page.on("pageerror", lambda e: seen["errors"].append(str(e)))
    page.on("websocket", lambda ws: seen["sockets"].append(ws.url))
    page.on("worker", lambda w: seen["workers"].append(w.url))
    page.on("popup", lambda pg: seen["popups"].append(pg.url))

    page.goto(base + "/index.html")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    check(True, "page reported ready")
    page.wait_for_timeout(IDLE_MS)
    check(not seen["foreign"], f"no off-origin request during load and {IDLE_MS // 1000}s idle {seen['foreign']}")

    interactions.run(page, check, engine)

    csp = page.evaluate("() => window.__builderCsp || []")
    check(not csp, f"no Content-Security-Policy violation {csp}")
    sw = page.evaluate("() => navigator.serviceWorker ? navigator.serviceWorker.getRegistrations().then(r => r.length) : 0")
    check(not seen["errors"], f"no uncaught page error {seen['errors']}")
    check(not seen["console"], f"no console error {seen['console']}")
    check(not seen["foreign"], f"no off-origin request at any point {seen['foreign']}")
    check(not seen["sockets"], f"no WebSocket {seen['sockets']}")
    check(not seen["popups"], f"no popup {seen['popups']}")
    check(not seen["workers"] and sw == 0, f"no worker or service worker {seen['workers']} sw={sw}")
    check(not after_ready, f"no request of any kind after the page reported ready {after_ready}")
    stored = page.evaluate(STORAGE_SCRIPT)
    extra = sorted(set(stored["local"]) - STORAGE_KEYS)
    check(
        stored["cookie"] == "" and not stored["session"] and not stored["idb"] and not extra,
        f"nothing kept in the browser but the declared keys {sorted(STORAGE_KEYS)} {stored}",
    )
    if hasattr(interactions, "run_blocked"):
        interactions.run_blocked(browser, base, check, engine)
    browser.close()


def main():
    origin = os.environ.get("BUILDER_ORIGIN", "").rstrip("/")
    server = None
    if not origin:
        if not (DIST / "index.html").exists():
            sys.exit("dist/ is missing: run scripts/build.sh first")
        server = serve()
        host, port = server.server_address[:2]
        origin = f"http://{host}:{port}"
    interactions = load_interactions()
    with sync_playwright() as p:
        for engine in ENGINES:
            run_engine(p, engine, origin, interactions)
    if server:
        server.shutdown()
    if failures:
        print(f"\nBROWSER CHECK FAILED: {len(failures)} problem(s)")
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print(f"\nBROWSER CHECK PASSED in {', '.join(ENGINES)}")


if __name__ == "__main__":
    main()
