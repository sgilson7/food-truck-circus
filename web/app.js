// The page draws what the core returns and decides nothing. It loads the data
// files, starts the core through the shim, and each display frame passes the
// clock, the held actions and the mouse movement in, then draws what comes
// out. The core says which mode the game is in; the page shows that screen.
import init, { Game, numbers, mesh } from "./pkg/ftc_shim.js";
import { makeRenderer } from "./gl.js";
import { makeInput } from "./input.js";
import { makeHud } from "./hud.js";
import { makeMap } from "./map.js";
import { setup as setupSound, wake, toggleMute } from "./sound.js";

const BUILD = "__BUILD__";
const SAVE_KEY = "food-truck-circus.tour";
const el = (id) => document.getElementById(id);
const file = (path) => fetch(`${path}?v=${BUILD}`).then((r) => r.text());

// Browser storage can be missing or refused; the game works without it.
const storage = {
  load() {
    try {
      return localStorage.getItem(SAVE_KEY) || "";
    } catch {
      return "";
    }
  },
  save(text) {
    try {
      localStorage.setItem(SAVE_KEY, text);
    } catch {
      // nothing to do: the tour is simply not remembered
    }
  },
};

function fillCopy(copy) {
  for (const node of document.querySelectorAll("[data-copy]")) {
    node.textContent = copy[node.dataset.copy];
  }
  document.title = copy.title;
  el("build").textContent = BUILD;
  for (const table of document.querySelectorAll("table.controls")) {
    for (const [keys, does] of copy.controls) {
      const row = table.insertRow();
      row.insertCell().textContent = keys;
      row.insertCell().textContent = does;
    }
  }
}

async function start() {
  const names = ["body", "tricks", "venues", "missions", "costumes", "copy", "controls", "sounds"];
  const texts = await Promise.all(
    names.map((n) => file(`data/${n}.json`)).concat([file("data/shaders/scene.vert"), file("data/shaders/scene.frag")]),
  );
  const [body, tricks, venues, missions, costumes, copyText, controlsText, soundsText, vertex, fragment] = texts;
  const copy = JSON.parse(copyText);
  fillCopy(copy);
  setupSound(JSON.parse(soundsText), copy.say);
  await init();
  const N = JSON.parse(numbers());

  let game;
  try {
    game = new Game(body, tricks, venues, missions, costumes, copyText, storage.load());
  } catch (e) {
    el("message").textContent = String(e);
    document.body.dataset.ready = "true";
    return;
  }

  const canvas = el("view");
  const renderer = makeRenderer(canvas, N, { vertex, fragment }, (id) => mesh(id));
  if (!renderer) el("message").textContent = copy.no_webgl;
  const save = () => storage.save(game.saved());
  const hud = makeHud();
  const input = makeInput(canvas, JSON.parse(controlsText), N.actions);
  const map = makeMap(game, copy, save);
  let sceneVersion = null;
  let shownMode = null;

  const go = (action) => () => {
    wake(BUILD);
    action();
    if (document.activeElement) document.activeElement.blur();
    hud.reset(JSON.parse(game.hud()));
    save();
    map.render();
  };
  el("perform").addEventListener("click", go(() => game.start()));
  el("resume").addEventListener("click", go(() => game.resume()));
  el("pause-map").addEventListener("click", go(() => game.to_map()));
  el("r-retry").addEventListener("click", go(() => game.start()));
  el("r-map").addEventListener("click", go(() => game.to_map()));
  el("r-next").addEventListener("click", () => {
    const next = el("r-next").dataset.next;
    go(() => {
      game.select(next);
      game.to_map();
    })();
  });
  el("everything").addEventListener("click", go(() => game.open_everything()));
  const reset = el("reset");
  reset.addEventListener("click", () => {
    if (!reset.classList.contains("armed")) {
      reset.classList.add("armed");
      reset.textContent = copy.reset_confirm;
      return;
    }
    reset.classList.remove("armed");
    reset.textContent = copy.reset;
    go(() => game.reset())();
  });
  // the glossary of moves: built once from the core, opened from the map,
  // the HUD and the pause screen; it pauses a show while it is open
  const glossary = JSON.parse(game.glossary());
  for (const m of glossary.moves) {
    const card = document.createElement("div");
    card.className = "move";
    const h = document.createElement("h4");
    h.textContent = m.name;
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = `${m.from_to} · ${m.points}`;
    const p = document.createElement("p");
    p.textContent = m.how;
    card.append(h, meta, p);
    el("move-list").appendChild(card);
  }
  el("move-roof").textContent = glossary.roof;
  for (const b of document.querySelectorAll(".open-moves")) {
    b.addEventListener("click", () => {
      game.pause();
      el("moves").hidden = false;
      b.blur();
    });
  }
  el("intro").addEventListener("animationend", () => {
    el("intro").hidden = true;
  });
  el("moves-close").addEventListener("click", () => {
    el("moves").hidden = true;
    el("moves-close").blur();
  });
  el("help-toggle").addEventListener("click", () => {
    el("help").hidden = !el("help").hidden;
    el("help-toggle").blur();
  });
  window.addEventListener("keydown", (e) => {
    if (e.code === "Escape" && !el("moves").hidden) {
      el("moves").hidden = true;
      return;
    }
    if (e.code === "Escape") {
      const mode = document.body.dataset.mode;
      if (mode === "play") game.pause();
      if (mode === "paused") go(() => game.resume())();
    }
    if (e.code === "KeyM") toggleMute();
    wake(BUILD);
  });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) game.pause();
  });

  function resize() {
    const [w, h] = game.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    if (renderer) renderer.resize(w, h);
  }
  window.addEventListener("resize", resize);
  resize();

  function showMode(mode, h) {
    document.body.dataset.mode = mode;
    el("map").hidden = mode !== "map";
    el("hud").hidden = mode === "map";
    el("paused").hidden = mode !== "paused";
    el("result").hidden = mode !== "result";
    if (mode === "map") map.render();
    // a show opens on the venue's postcard, which fades as the show starts
    if (mode === "play" && shownMode !== "paused") {
      el("intro-art").src = h.art;
      el("intro-art").alt = `${h.venue}, ${h.city}`;
      el("intro").hidden = false;
      el("intro").classList.remove("go");
      void el("intro").offsetWidth;
      el("intro").classList.add("go");
    }
    if (mode === "result" && h.result) {
      const r = h.result;
      el("r-word").textContent = r.shown;
      el("r-title").textContent = r.passed ? copy.result_pass : copy.result_fail;
      el("r-mission").textContent = `${h.mission} · ${h.venue}, ${h.city}`;
      el("r-grade").textContent = copy.result_grade.replace("{grade}", r.grade);
      el("r-unlock").textContent = r.unlocked ? copy.result_unlock.replace("{costume}", r.unlocked) : "";
      el("r-next-card").hidden = !h.next_art;
      if (h.next_art) {
        el("r-next-art").src = h.next_art;
        el("r-next-venue").textContent = h.next_venue;
      }
      const next = el("r-next");
      next.hidden = r.next === null;
      if (r.next !== null) next.dataset.next = r.next;
      save();
    }
  }

  function frame(now) {
    const { bits, dx, dy } = input.read();
    game.advance(now, bits, dx, dy);
    const h = JSON.parse(game.hud());
    if (h.mode !== shownMode) {
      showMode(h.mode, h);
      shownMode = h.mode;
    }
    hud.update(h, h.mode === "play");
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
