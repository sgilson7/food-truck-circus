# Plan

Stage 2. `B2 Require step-by-step justification`. Approved in advance (BRIEF.md, Autonomy).

## What the show is (research, 2026-10-09)

Cirque Kikasse is a Québec company founded in 2021. Its show *SANTÉ!* is played on and around a food truck that also serves food and drink. Presenters' listings agree on these acts:

- a trampoline beside the truck, with acrobatics up to the truck and well above it;
- a balancing act on a tower of stacked tables and chairs, given as 30 or 35 feet;
- juggling ("trampolines, balancing and juggling are the daily special", The Ringling);
- the clean-up of the truck and trampoline played as clumsy comedy, and a popcorn machine that overflows onto the audience.

No listing mentions a trapeze. The request asks for trapezes, so the game has one and the notebook says it is the game's addition (row P1).

Sources: carolinatheatre.org (event page), cirquekikasse.com (home and events list), ringling.org, PS21, Lehigh, Bucknell listings.

## The tour, as the game's map

The company's events page lists 31 stops. The game uses the US ones, in this order, starting where the peer pointed: the Carolina Theatre of Durham run (October 2026), then Richmond, New York and Fairfield, Sarasota and Miami, Tucson and Salt Lake City, San Francisco and Washington State. Every venue is a scene in `data/venues.json`: sky, ground, the buildings and plants around it, the time of the show.

## Architecture (Builder, from Iron Vector)

- `crates/core` (`ftc_core`): every rule, Q16.16 integers, no floats, clocks or I/O. Reuses Iron Vector's `fx`, `geom`, `mesh`, `rng` and the instanced-box renderer contract.
- `crates/shim` (`ftc_shim`): one call per function, no branches.
- `web/`: WebGL 2 boxes (Iron Vector's `gl.js`), HUD, the tour map, costumes, sound. No numeric literal but 0 and 1; sound recipes live in `data/sounds.json`.

## The body (from Vagrancy)

Vagrancy's body is particles joined by sticks, relaxed in a fixed order, integer fixed point, with motors that add momentum "from nowhere" by design (its D9). The performer here is the same kind of body in 3D: 15 particles (head, neck, shoulders, elbows, hands, pelvis, hips, knees, feet) and the sticks between them.

Two regimes, one body:

1. **In control.** A flight model drives the particles: centre of mass, velocity, a flip angle and a twist angle with angular momentum. The pose (layout, pike, tuck) sets the moment of inertia, and angular momentum is conserved, so tucking spins faster. The particles are placed from the pose each tick and their previous positions carry the velocity, so the switch to regime 2 is seamless.
2. **Bailed.** A bad landing hands the particles to the Verlet solver: gravity, sticks, the ground, the bed. The performer tumbles as Vagrancy's fighters do, and gets up after a moment.

Divergence from Vagrancy, recorded as row B1: direct joint motors on every limb make a trick nearly unplayable for a first-time player (Vagrancy's arms take practice by design); a trick game needs the flip to be the skill, not the limbs.

The trampoline bed is a Verlet membrane (a grid of particles and sticks, edges anchored to the frame). The trapeze ropes are Verlet chains. The chair tower is a stack of rigid segments on Verlet joints that sways.

## Tricks and landings

Core names each flight from what happened: flips (half turns rounded), twists (half turns), the pose held most, and what the performer landed on (feet, seat, back, front, the truck roof, the ground). The names are real trampoline vocabulary held in `data/tricks.json`: back tuck, front pike, barani, full, rudy, double back, seat drop, swivel hips, back drop, front drop, cradle. A landing is graded by the angle off upright and the spin left over, and the crowd calls the word: EXCELLENT, GREAT, NICE, OK, MISS (Slushline's thresholds), with Slushline's flair words 30 % of the time. A crash is OH NO or DARN, in Sam's own recordings from Slushline.

## Missions

Data-driven objectives in `data/missions.json`, each at a venue, each with prerequisites (the Hasse diagram; level = longest chain, as Iron Vector's campaign):

- `height` reach a height N times (learning to drive the bed);
- `tricks` perform named tricks;
- `routine` N tricks in a row without a stop, scoring a target;
- `roof` land on the truck roof, with or without a trick;
- `popcorn` collect popcorn in the air before time runs out;
- `tower` stack chairs and hold a handstand on top;
- `trapeze` swing, release, trick, land;
- `juggle` keep three balls going.

## Costumes

`data/costumes.json`: colours and a hat for each of the performer's parts; unlocked by missions.

## Order of work

1. Core: flight model, body, bed, trick naming, landing grade. Tests with hand-computed cases.
2. Renderer, camera, HUD, crowd and callouts; play one mission end to end.
3. The tour map, venues and the rest of the mission kinds.
4. Costumes, sound, save.
5. Check, repository, Pages, scholar site.
