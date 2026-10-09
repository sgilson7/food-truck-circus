// The Graph Lab page. Like the game's page it draws what the core sends and
// decides nothing: the small graph's every position, the table, the tree
// and the planner's thinking come from the core's lab() as JSON, and every
// word is a copy string. Edits go back to the core as typed or dragged.
import init, { Game, numbers, mesh } from "./pkg/ftc_shim.js";
import { makeRenderer } from "./gl.js";
import { makeInput } from "./input.js";
import { makeHud } from "./hud.js";
import { setup as setupSound, wake, toggleMute } from "./sound.js";

const BUILD = "__BUILD__";
const el = (id) => document.getElementById(id);
const file = (path) => fetch(`${path}?v=${BUILD}`).then((r) => r.text());
const at = (copy, path) => path.split(".").reduce((o, k) => (o == null ? o : o[k]), copy);

function make(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

async function start() {
  const names = ["body", "tricks", "venues", "missions", "costumes", "copy", "controls", "sounds"];
  const texts = await Promise.all(
    names.map((n) => file(`data/${n}.json`)).concat([file("data/shaders/scene.vert"), file("data/shaders/scene.frag")]),
  );
  const [body, tricks, venues, missions, costumes, copyText, controlsText, soundsText, vertex, fragment] = texts;
  const copy = JSON.parse(copyText);
  for (const node of document.querySelectorAll("[data-copy]")) node.textContent = at(copy, node.dataset.copy);
  el("build").textContent = BUILD;
  setupSound(JSON.parse(soundsText), copy.say);
  await init();
  const N = JSON.parse(numbers());

  let game;
  try {
    // the lab keeps no tour: it starts from an empty save every time
    game = new Game(body, tricks, venues, missions, costumes, copyText, "");
  } catch (e) {
    el("message").textContent = String(e);
    document.body.dataset.ready = "true";
    return;
  }
  game.lab_open();

  const canvas = el("view");
  const renderer = makeRenderer(canvas, N, { vertex, fragment }, (id) => mesh(id));
  if (!renderer) el("message").textContent = copy.no_webgl;
  const hud = makeHud();
  const input = makeInput(canvas, JSON.parse(controlsText), N.actions);

  // ---- the controls the panel holds ----
  const lab0 = JSON.parse(game.lab());
  for (const [scale, label] of copy.lab.speeds) {
    const o = make("option", null, label);
    o.value = scale;
    el("speed").appendChild(o);
  }
  lab0.venues.forEach((name, i) => {
    const o = make("option", null, name);
    o.value = i;
    el("venue").appendChild(o);
  });
  el("venue").value = lab0.venue;
  for (const n of lab0.counts) {
    const o = make("option", null, n);
    o.value = n;
    el("points").appendChild(o);
  }
  for (const idea of copy.lab.ideas) el("ideas").appendChild(make("li", null, idea));

  let shown = -1;
  const edited = (message) => {
    el("error").textContent = message || "";
    shown = -1;
  };
  for (const p of copy.lab.presets) {
    const b = make("button", null, p);
    b.type = "button";
    b.addEventListener("click", () => {
      el("formula").value = p;
      edited(game.lab_formula(p));
    });
    el("presets").appendChild(b);
  }
  el("formula-form").addEventListener("submit", (e) => {
    e.preventDefault();
    edited(game.lab_formula(el("formula").value));
  });
  el("formula").addEventListener("change", () => edited(game.lab_formula(el("formula").value)));
  const domain = () => edited(game.lab_domain(el("from").value, el("to").value));
  el("from").addEventListener("change", domain);
  el("to").addEventListener("change", domain);
  el("points").addEventListener("change", () => {
    game.lab_points(el("points").value);
    edited("");
  });
  el("speed").addEventListener("change", () => game.lab_speed(el("speed").value));
  el("venue").addEventListener("change", () => game.lab_venue(el("venue").value));
  const run = (auto) => () => {
    wake(BUILD);
    game.lab_run(auto);
    el("you-hint").hidden = auto;
    document.activeElement.blur();
    edited("");
  };
  el("run-auto").addEventListener("click", run(true));
  el("run-you").addEventListener("click", run(false));
  el("stop").addEventListener("click", () => {
    game.lab_reset();
    edited("");
  });
  window.addEventListener("keydown", (e) => {
    if (e.code === "KeyM") toggleMute();
  });

  // ---- the small graph: dots you can drag ----
  const box = el("graph-box");
  const svg = el("graph");
  const labels = el("graph-labels");
  let dragging = null;
  box.addEventListener("pointermove", (e) => {
    if (dragging === null) return;
    const r = box.getBoundingClientRect();
    game.lab_drag(dragging, e.clientY - r.top, r.height);
    el("formula").value = copy.lab.drawn_formula;
    shown = -1;
  });
  const drop = () => {
    dragging = null;
  };
  box.addEventListener("pointerup", drop);
  box.addEventListener("pointercancel", drop);

  function line(cls, x1, y1, x2, y2) {
    const l = document.createElementNS(svg.namespaceURI, "line");
    l.setAttribute("class", cls);
    l.setAttribute("x1", x1);
    l.setAttribute("y1", y1);
    l.setAttribute("x2", x2);
    l.setAttribute("y2", y2);
    svg.appendChild(l);
  }

  let pen = null;
  function drawPanel(lab) {
    const g = lab.graph;
    svg.replaceChildren();
    labels.replaceChildren();
    for (const t of g.ticks_x) {
      line("grid", t.at, "0", t.at, "100");
      const s = make("span", null, t.label);
      s.style.left = `${t.at}%`;
      s.style.top = `${g.x_axis}%`;
      labels.appendChild(s);
    }
    for (const t of g.ticks_y) {
      line("grid", "0", t.at, "100", t.at);
      const s = make("span", "y", t.label);
      s.style.left = `${g.y_axis || g.ticks_x[0].at}%`;
      s.style.top = `${t.at}%`;
      labels.appendChild(s);
    }
    line("axis", "0", g.x_axis, "100", g.x_axis);
    if (g.y_axis) line("axis", g.y_axis, "0", g.y_axis, "100");
    for (const pts of g.curve) {
      const p = document.createElementNS(svg.namespaceURI, "polyline");
      p.setAttribute("class", "curve");
      p.setAttribute("points", pts);
      svg.appendChild(p);
    }
    for (const s of g.samples) {
      const d = make("div", "dot");
      d.title = s.label;
      d.style.left = `${s.x}%`;
      d.style.top = `${s.y}%`;
      d.addEventListener("pointerdown", (e) => {
        dragging = s.i;
        box.setPointerCapture(e.pointerId);
        e.preventDefault();
      });
      labels.appendChild(d);
    }
    // the marks on top of the dots they were aiming for
    for (const m of g.marks) {
      const d = make("div", "mark");
      d.style.left = `${m.x}%`;
      d.style.top = `${m.y}%`;
      labels.appendChild(d);
    }
    pen = make("div", "pen");
    labels.appendChild(pen);

    const tbody = el("machine").tBodies[0];
    tbody.replaceChildren();
    for (const r of lab.rows) {
      const tr = make("tr", r.next ? "next" : null);
      for (const v of [r.x, r.y, r.change, r.change2, r.marked, r.off]) tr.appendChild(make("td", null, v));
      tbody.appendChild(tr);
    }
    const tree = el("tree");
    tree.replaceChildren();
    for (const n of lab.tree) {
      const li = make("li", `d${n.depth}${n.lit ? " lit" : ""}`);
      li.appendChild(make("span", "kind", n.kind));
      li.appendChild(document.createTextNode(n.label));
      tree.appendChild(li);
    }
    const log = el("log");
    log.replaceChildren();
    for (const t of lab.log) log.appendChild(make("li", null, t));
    log.scrollTop = log.scrollHeight;

    if (document.activeElement !== el("formula")) el("formula").value = lab.drawn ? copy.lab.drawn_formula : lab.formula;
    if (document.activeElement !== el("from")) el("from").value = lab.from;
    if (document.activeElement !== el("to")) el("to").value = lab.to;
    el("points").value = lab.n;
    el("lab-summary").hidden = !lab.summary;
    el("lab-summary").textContent = lab.summary || "";
  }

  function resize() {
    const [w, h] = game.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    if (renderer) renderer.resize(w, h);
  }
  window.addEventListener("resize", resize);
  resize();

  let sceneVersion = null;
  let lastKey = "";
  function frame(now) {
    const { bits, dx, dy } = input.read();
    game.advance(now, bits, dx, dy);
    const lab = JSON.parse(game.lab());
    const key = `${lab.version} ${lab.running} ${lab.finished} ${lab.formula} ${lab.n}`;
    if (key !== lastKey || shown < 0) {
      lastKey = key;
      shown = lab.version;
      drawPanel(lab);
    }
    if (pen) {
      pen.style.left = `${lab.graph.pen.x}%`;
      pen.style.top = `${lab.graph.pen.y}%`;
    }
    hud.update(JSON.parse(game.hud()), true);
    if (renderer) {
      if (game.scene_version() !== sceneVersion) {
        sceneVersion = game.scene_version();
        renderer.setScene(game.static_instances(), game.uniforms(), game.clear_color());
      }
      game.build_frame();
      renderer.frame(game.view(), game.instances(), game.draws());
    }
    requestAnimationFrame(frame);
  }
  hud.reset(JSON.parse(game.hud()));
  document.body.dataset.ready = "true";
  requestAnimationFrame(frame);
}

start();
