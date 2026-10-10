# Handoff

Stage 9, written by the agent. `C5 Consolidate in your own words`. A fresh session starts from this file.

## Where the work is

Food Truck Circus is a Builder project built in one session on 2026-10-09 from `BRIEF.md` (Sam's request, word for word) and `PLAN.md`. Repository `sgilson7/food-truck-circus`, public; a push to `main` runs `./scripts/check.sh` in Chromium on GitHub Actions and deploys `dist/` to Pages only if it passes. Sam approved the repository, Pages and the deploys in advance.

## What exists

- **Core** (`crates/core/src`): `performer` (stand, bed, flight, landing, crash), `body` (particles and sticks; Verlet on a crash), `balance` (every feel number), `stage` (ground, bed, truck roof), `trick` (names and grades), `acts` (bed cloth, popcorn, chair tower, trapeze, juggling), `mission` (goals), `tour` (data, Hasse levels and layout, progress), `scene` (venue art from `props`), `crowd`, `render`, `game` (modes, clock, camera, HUD and map JSON). `fx`, `geom`, `mesh`, `rng` are Iron Vector's, with a cylinder and a cone added to `mesh`.
- **Data**: 21 venues, 26 missions, 35 trick rows, 12 costumes, the copy, the sound recipes, the body's poses.
- **Page**: `app.js` (loop and screens), `map.js` (Hasse map, brief, costumes), `hud.js`, `sound.js`, `input.js`, and Iron Vector's `gl.js` unchanged.

## What passed

`./scripts/check.sh`: fmt, clippy with warnings as errors, the core tests and 11 boundary tests, the release wasm build, the wasm import audit, and the browser check in Chromium, Firefox and WebKit (`BUILDER CHECK PASSED`). Run `cargo test -p ftc_core 2>&1 | grep "test result"` for the counts.

## Live (2026-10-09)

- Game: https://sgilson7.github.io/food-truck-circus/ (Actions run 37988796320: check and deploy passed). `BUILDER_ORIGIN=https://sgilson7.github.io/food-truck-circus tests/browser/check.py` passed in Chromium, Firefox and WebKit against the live site.
- Scholar site: https://sgilson7.github.io/projects/food-truck-circus/, commit "Add Food Truck Circus to the games" in `~/Documents/website`; `make check` (56 pages, no findings), `make packet` (44 routes clean), `make gate` and `make gate-live` passed.

## Milestone 2 (2026-10-09): the Circus Math Lab, and Sam's fixes

`PLAN-M2.md` is the plan; `SECOND-ORDER.md` rows W5 and L1–L9 the notebook. New core modules: `expr` (formulas in fixed point), `plot` (the function table and the bounce planner), `lab` (the lab mode and the plotting lab), `fly` and `labfly` (popcorn under a flight), `siteswap` and `labjuggle` (juggling patterns). New page: `lab.html` with `lab.js` and `lab.css`; tabs at `#plot`, `#popcorn`, `#juggling`. Also: the Moves glossary (`how` in `data/tricks.json`), postcards (`tools/postcards.py` → `web/art/venue/`, TikZ in `tools/postcards/`), juggling made learnable, the map-click fix, and the near plane moved for the flicker.

Live at https://sgilson7.github.io/food-truck-circus/lab.html; `./scripts/check.sh` passed in three engines, and the browser check passed against the live site in three engines. The scholar site's page describes the labs (`make gate-live` passed).

## French mode (2026-10-10)

`data/fr/` holds the French words (a whole copy file, and overlays by id for tricks, missions, costumes and venues; `crates/core/src/lang.rs`). `Game::set_french` swaps them in; the pages keep the choice in `localStorage` under `food-truck-circus.lang` and reload on the button. French postcards: `python3 tools/postcards.py --fr` into `web/art/venue/fr/`. Notebook rows F1–F4. Live and checked in three engines against the live site.

**The crowd's groans (2026-10-10).** A fall or drop calls one of `groans` in `data/tricks.json` (oh_no, darn, non, zut), each as likely. `voice_clips` in each copy file names the recording a word plays: in English every groan plays its own (Sam's OH NO and DARN, his mom's NON and ZUT), and in French only Mom's play, with OH NO reading NON. Rows F5–F6. `./scripts/check.sh`: BUILDER CHECK PASSED (chromium, firefox, webkit).

## What remains

- Sam's playtest. Every feel number in `balance.rs` is the agent's guess (SECOND-ORDER T1, T2). Tests show the shows can be passed (the first by holding Space; the roof, the trapeze and a back tuck by scripted inputs), not that they are fun.
- The routine, double back, rudy and full-in shows have not been passed by a scripted player; they rely on the same flight model as the back tuck.
- Sam's eye on the high-bounce flicker (T3) and the labs' copy (T4); a French speaker's read of `data/fr/` (T5).
- The scenes are sketches built from boxes, cones and spheres. A venue looking like itself is Sam's call.

## What the next session must not assume

- That CI checked Firefox and WebKit: it runs Chromium only, as Iron Vector's does. Run the three-engine check locally before pushing.
- That `scripts/serve.py` survives a rebuild: `build.sh` deletes `dist/`, so restart the server after building.
