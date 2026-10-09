// The heads-up display: what the core's hud() says, written into the page.
// Every string and every position comes from the core.
import { call, play } from "./sound.js";

const el = (id) => document.getElementById(id);

export function makeHud() {
  let lastCall = 0;
  let lastSfx = 0;
  let lastBubble = 0;
  const callout = el("callout");
  const word = el("c-word");
  const trick = el("c-trick");
  const bubbles = el("bubbles");
  const hands = el("hands");

  // the lab page has no mission box: a missing element is skipped
  function text(id, value) {
    const node = el(id);
    if (node && node.textContent !== value) node.textContent = value;
  }

  return {
    // Forget what was heard, so a new show's first call is not missed.
    reset(hud) {
      lastCall = hud.callouts.reduce((a, c) => (c.seq > a ? c.seq : a), lastCall);
      lastSfx = hud.sfx.reduce((a, s) => (s.seq > a ? s.seq : a), lastSfx);
    },
    update(hud, playing) {
      text("h-mission", hud.mission);
      text("h-venue", `${hud.venue} · ${hud.city}`);
      text("h-objective", hud.objective);
      text("h-progress", hud.progress);
      text("h-height", hud.height);
      text("h-peak", hud.peak);
      text("hint", hud.hint);
      const timer = el("h-timer");
      if (timer) timer.hidden = !hud.timer;
      if (hud.timer) text("h-timer", hud.timer);

      // juggling's key badges, one per hand, lit when a ball is due
      while (hands.children.length < hud.hands.length) hands.appendChild(document.createElement("div"));
      while (hands.children.length > hud.hands.length) hands.lastChild.remove();
      hud.hands.forEach((h, k) => {
        const node = hands.children[k];
        node.className = h.hot ? "hand hot" : "hand";
        node.textContent = h.key;
        node.style.left = `${h.x}%`;
        node.style.top = `${h.y}%`;
      });

      for (const s of hud.sfx) {
        if (s.seq <= lastSfx) continue;
        lastSfx = s.seq;
        if (playing) play(s.kind, s.level);
      }
      for (const c of hud.callouts) {
        if (c.seq <= lastCall) continue;
        lastCall = c.seq;
        word.textContent = c.shown;
        word.className = `word ${c.word}`;
        trick.textContent = c.text;
        trick.hidden = !c.text;
        callout.classList.remove("pop");
        // restart the animation: reading the layout makes the removal count
        void callout.offsetWidth;
        callout.classList.add("pop");
        call(c.word);
      }
      for (const b of hud.bubbles) {
        if (b.seq <= lastBubble) continue;
        const node = document.createElement("div");
        node.className = "bubble";
        node.textContent = b.text;
        node.style.left = `${b.x}%`;
        node.style.top = `${b.y}%`;
        node.addEventListener("animationend", () => node.remove());
        bubbles.appendChild(node);
      }
      for (const b of hud.bubbles) if (b.seq > lastBubble) lastBubble = b.seq;
    },
  };
}
