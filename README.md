# Food Truck Circus

A 3D trampoline game in the browser. You are the performer on a food truck circus, and each show is a trick or a run of tricks at a real stop on a real tour.

**Play:** https://sgilson7.github.io/food-truck-circus/

It is a fan game after Cirque Kikasse's *SANTÉ!*, the Québec circus played on and around a food truck, made for the show's Durham run in October 2026. It is not their show and uses nothing of theirs but the idea: no logo, no likeness, no copy.

- **The tour.** 26 shows at 21 venues from the company's 2025–26 tour, starting at the RTP 800 Building in Durham on October 8, 2026 and ending in Port Angeles, Washington. The map is a Hasse diagram of the shows by what each needs first.
- **The trampoline.** Hold Space to push the bed as it throws you up, and each bounce goes higher. Hold S or W while it throws you to set a back or front flip turning, A or D to twist, J to tuck and spin faster, K to pike and land on your seat. The game names what you did in trampoline terms (seat drop, swivel hips, cradle, back tuck, barani, rudy, back full, double back, full-in) and grades the landing.
- **The other acts.** A tower of chairs on the truck's roof to about thirty-five feet, then a handstand on top. Juggling two and three balls. The popcorn machine overflowing over the bed. A trapeze over the bed, which is the game's own addition: no listing of the show mentions one.
- **The crowd** sits on blankets and lawn chairs, as the listings say to come, and shouts the grade: EXCELLENT, GREAT, NICE, OK, with Slushline's flair words. A fall is OH NO or DARN, in Sam's own voice.
- **Costumes.** Twelve, earned on the tour.
- **Postcards.** Every stop has a postcard, drawn as TikZ in the style of `Sam_Comm_final.png` by `tools/postcards.py` from the venue's own data, and shown on the map, as each show opens, and as the next stop when a show ends.

## The Circus Math Lab

**Open it:** https://sgilson7.github.io/food-truck-circus/lab.html

Three math-circle toys after Natural Math's way in: play first, formalize later, functions as machines before formulas, and "gentle calculus".

- **Plot a function** (`lab.html`). Draw a function by dragging its points, read it as a function machine (x in, f(x) out, the change from row to row and the change of that change), or type a formula (`x^2`, `2x+1`, `|x|`, `sin(x)`). The performer plots it on the trampoline: **the top of each bounce marks one point**, with back tucks, back pikes and tuck jumps on the way. A behaviour tree chooses how many beats to hold Space and the arrow, and when to somersault, by imagining bounces in the game's own physics and interpolating between two that bracket the target; the lab shows the tree and its thinking in words. The marks are joined with straight lines or a smooth curve, drawn beside f with the gap shaded and the largest and average error stated.
- **Popcorn under a flight** (`lab.html#popcorn`). The performer jumps off a tall ledge, lands on the bed and bounces on to the grass. Every arc of the flight is a parabola, and the lab writes its equation in two forms. In each strip of ground the performer drops a column of popcorn as tall as they are high, so the piles are a Riemann sum and counting kernels counts area; the lab compares the popcorn, the Riemann sum and the area worked out finely. Strip width and where in the strip the popcorn drops are yours to change. "Match a curve" finds the jump whose flight best fits a typed curve, by successive parabolic interpolation: a parabola through three misfits finds the best parabola.
- **Juggling patterns** (`lab.html#juggling`). Patterns written as numbers (siteswap): each digit is how many beats a ball stays up, so a bigger number is a higher throw. The lab checks a pattern (the number of balls is the average of the digits; no two balls may come down on the same beat), says why one fails, draws the beat diagram, and the performer juggles it.

## Moves

The **Moves** button (on the map, in a show and on the pause screen) lists every trick the crowd can name and how to do it.

## Controls

| Input | Does |
| --- | --- |
| Space (hold) | Push the bed. Also: let go of the trapeze, bring up a chair, start the handstand |
| C | Stop bouncing |
| S / W | Back flip / front flip, held while the bed throws you up |
| A / D | Twist left / right; in juggling, the hand on that side of the screen |
| J or Left Shift | Tuck |
| K | Pike (land on your seat) |
| Arrow keys | Travel toward or away from the truck, turn; walk on the ground |
| Mouse drag | Look around |
| R · Esc · M | Start the show again · Pause · Sound off |

## How it is built

Food Truck Circus is a [Builder](https://github.com/sgilson7/builder-setting) project, and its two parents are Sam's other games.

- **From Iron Vector:** the 3D engine. Q16.16 fixed-point maths with no floats in the core, one instanced draw per mesh, the core computing every matrix, the camera and the HUD, and the page's `gl.js` reused unchanged.
- **From Vagrancy:** the body. The performer is particles joined by sticks, relaxed in a fixed order in integer maths. In control, a flight model places the particles: a centre of mass, a flip and a twist, and the angular momentum behind each, with the inertia read off the body's shape, so a tuck spins faster than a layout with nothing else said. On a bad landing the same particles go limp into the Verlet solver and tumble. As in Vagrancy, a held key adds a little momentum from nowhere.
- **The lab's formulas** are read and evaluated by the core in fixed point too (`crates/core/src/expr.rs`), and its planner (`plot.rs`) imagines on a copy of the same performer the player drives.
- **From Slushline:** the grade words, their thresholds and their flair, and the two recorded voice lines.

The layers:

- **`crates/core`** holds every rule: the bounce, the flips, trick names, grades, the tower, the trapeze, juggling, popcorn, the missions, the tour, the scenes and the crowd. No input or output, no floats, no clock.
- **`crates/shim`** moves values across the WebAssembly boundary and decides nothing.
- **`web/`** is a static page with a WebGL 2 canvas. It draws what the core returns.
- **`data/`** holds every venue, mission, trick, costume, string, sound recipe and shader.

```bash
./scripts/setup.sh   # once: Playwright and three browser engines
./scripts/check.sh   # fmt, clippy, the core tests, the wasm build and audit, the browser check
python3 scripts/serve.py 8000
```

A push to `main` runs the same check in Chromium and deploys `dist/` to GitHub Pages if it passes.

## Credits

Made by Sam Gilson. The voice clips are Sam's recordings (LICENSES.md). Venue names, cities and dates are from Cirque Kikasse's events page and the presenters' listings; the scenes are the game's own sketches of those places.
