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

## What remains

- Sam's playtest. Every feel number in `balance.rs` is the agent's guess (SECOND-ORDER T1, T2). Tests show the shows can be passed (the first by holding Space; the roof, the trapeze and a back tuck by scripted inputs), not that they are fun.
- The routine, double back, rudy and full-in shows have not been passed by a scripted player; they rely on the same flight model as the back tuck.
- The scenes are sketches built from boxes, cones and spheres. A venue looking like itself is Sam's call.

## What the next session must not assume

- That CI checked Firefox and WebKit: it runs Chromium only, as Iron Vector's does. Run the three-engine check locally before pushing.
- That `scripts/serve.py` survives a rebuild: `build.sh` deletes `dist/`, so restart the server after building.
