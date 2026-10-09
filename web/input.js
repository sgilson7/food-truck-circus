// Keyboard and mouse drag, turned into the action bits the core defines.
// data/controls.json names the action for each key; the core's numbers()
// gives each action its bit. Nothing here decides what an action does.

export function makeInput(canvas, controls, actions) {
  // `tapped` keeps a press released before the next frame reads it
  const state = { held: 0, tapped: 0, dx: 0, dy: 0, dragging: false };
  const bitOf = (code) => actions[controls[code]] || 0;

  window.addEventListener("keydown", (e) => {
    const bit = bitOf(e.code);
    if (!bit) return;
    state.held |= bit;
    state.tapped |= bit;
    // keeps Space off a focused button and the arrows off the page's scroll
    e.preventDefault();
  });
  window.addEventListener("keyup", (e) => {
    state.held &= ~bitOf(e.code);
  });
  canvas.addEventListener("pointerdown", (e) => {
    state.dragging = true;
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!state.dragging) return;
    state.dx += e.movementX;
    state.dy += e.movementY;
  });
  const stop = () => {
    state.dragging = false;
  };
  canvas.addEventListener("pointerup", stop);
  canvas.addEventListener("pointercancel", stop);
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("blur", () => {
    state.held = 0;
    state.dragging = false;
  });

  return {
    read() {
      const drag = state.dragging ? actions.drag : 0;
      const out = { bits: state.held | state.tapped | drag, dx: state.dx, dy: state.dy };
      state.tapped = 0;
      state.dx = 0;
      state.dy = 0;
      return out;
    },
  };
}
