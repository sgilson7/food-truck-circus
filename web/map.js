// The tour map: the missions' Hasse diagram as the core lays it out, the
// selected show's brief, and the costume rack. The core places every node
// and says what is open; this file draws it and passes clicks back.

const el = (id) => document.getElementById(id);

export function makeMap(game, copy, onChange) {
  const nodes = el("nodes");
  const edges = el("edges");

  function render() {
    const map = JSON.parse(game.map());
    el("tour-count").textContent = `${map.passed} / ${map.total}`;

    const legs = el("legs");
    legs.replaceChildren();
    map.legs.forEach((leg, i) => {
      const s = document.createElement("span");
      s.textContent = leg.name;
      s.title = leg.note;
      s.style.setProperty("--c", `var(--leg${i})`);
      legs.appendChild(s);
    });

    edges.replaceChildren();
    for (const e of map.edges) {
      const line = document.createElementNS(edges.namespaceURI, "line");
      line.setAttribute("x1", e.x1);
      line.setAttribute("y1", e.y1);
      line.setAttribute("x2", e.x2);
      line.setAttribute("y2", e.y2);
      if (e.open) line.setAttribute("class", "open");
      edges.appendChild(line);
    }

    nodes.replaceChildren();
    for (const m of map.missions) {
      const b = document.createElement("button");
      b.className = "node";
      b.classList.toggle("locked", !m.open);
      b.classList.toggle("passed", m.passed);
      b.classList.toggle("selected", m.i === map.selected);
      b.style.left = `${m.x}%`;
      b.style.top = `${m.y}%`;
      b.style.setProperty("--c", `var(--leg${m.leg})`);
      b.textContent = `${m.i + 1}`;
      b.title = `${m.name} · ${m.venue}, ${m.city}`;
      const label = document.createElement("span");
      label.className = "label";
      label.textContent = m.name;
      b.appendChild(label);
      b.addEventListener("click", () => {
        game.select(m.i);
        b.blur();
        render();
        onChange();
      });
      nodes.appendChild(b);
    }

    const m = map.missions[map.selected];
    el("b-art").src = m.art;
    el("b-art").alt = `${m.venue}, ${m.city}`;
    el("b-when").textContent = m.when;
    el("b-name").textContent = m.name;
    el("b-venue").textContent = `${m.venue} · ${m.city}`;
    el("b-goal").textContent = m.goal;
    el("b-brief").textContent = m.brief;
    el("b-best").textContent = m.best ? copy.best.replace("{word}", m.best) : copy.not_yet;
    el("b-unlock").textContent = m.unlock ? copy.unlocks.replace("{costume}", m.unlock) : "";
    el("perform").disabled = !m.open;
    el("b-locked").hidden = m.open;

    const rack = el("costumes");
    rack.replaceChildren();
    for (const c of map.costumes) {
      const b = document.createElement("button");
      b.className = "costume";
      b.classList.toggle("worn", c.worn);
      b.disabled = !c.unlocked;
      b.title = c.unlocked ? c.name : copy.costume_locked.replace("{mission}", c.by);
      const sw = document.createElement("span");
      sw.className = "swatch";
      for (const colour of c.colours) {
        const i = document.createElement("i");
        i.style.background = colour;
        sw.appendChild(i);
      }
      b.appendChild(sw);
      b.appendChild(document.createTextNode(c.name));
      b.addEventListener("click", () => {
        game.wear(c.i);
        b.blur();
        render();
        onChange();
      });
      rack.appendChild(b);
    }
  }

  return { render };
}
