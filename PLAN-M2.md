# Plan, milestone 2: the Graph Lab

Stage 2. `B2 Require step-by-step justification`. Sam asked for this on 2026-10-09 and, as with the MVP, to keep going and deploy; deploys are pre-approved (BRIEF.md).

## The request, word for word

> my peer who suggested the game helps run the natural math group ( https://naturalmath.com) and math circles around the unc area, and left me this comment: "Their act has a good amount of intrinsic mathematics in it. One could model algebra, or graphs, or calculus, and make a game mechanic out of it." so like vagrancy has the BT tool, i really want this game to be able to be usable as a math teaching tool, in line with natural math curricula and thoughts around teaching early math, with my peers focus on types of math. my first version of this is being able to use this space to draw functions / define functions, then your performer uses behavior trees and interpolative methods to figure out a set of inputs that gets them to trace out the function, like a mini fun version of desmos where the graphing pointer is a lil performer doing their routine to draw out the parabola for x^2 between -2 and +2 or something like this.

## What Natural Math asks of a tool (research)

Natural Math (Maria Droujkova; *Moebius Noodles* with Yelena McManaman) treats functions as mappings long before formulas (its baby-animal-to-adult-animal function), plays first and formalizes later, and offers young children "gentle calculus": speed as a change in distance, co-variation, grids. The lab follows that order:

1. **Draw it.** Drag the points of the function up and down on a small grid.
2. **The function machine.** The same function as a table, x in, f(x) out, with the change from one row to the next (finite differences: a parabola's changes change by the same amount each step).
3. **Write it.** A formula box (`x^2`, `2x+1`, `abs(x)`, `sin(x)`), parsed and evaluated by the core in fixed point.

## The mechanic: plotting by bouncing

The bed runs left to right along the graph's x axis. The performer faces along it and bounces from one column to the next. **The top of each bounce marks a point**: the feet's height is f(x). Between marks the body flies a parabola of its own, because gravity draws parabolas; the lab shows those arcs as a trail, so the intrinsic mathematics is visible beside the target.

A bounce's top is set by two things a player can press: how long Space pushes the bed (or C stops it), and how long the arrow key leans forward. **The performer plans with a behaviour tree**, as Vagrancy's pilots do, and returns only keys a player could press:

- *Read the machine*: the next x, its f(x), its height.
- *Imagine*: run the bounce forward in the core's own physics (a copy of the performer), for a push of so many beats, and see how high it goes.
- *Interpolate*: between two imagined pushes that bracket the target height, the target's place on the line between them picks the next push to try (regula falsi). Then the same for the lean, so the top lands over the column.
- *Too far for one bounce?* A set-up bounce in place toward the height, then try again.
- *Bounce and mark.*

The lab shows the tree with the running node lit, and a log of what was imagined and interpolated, in words. Each mark's error is in the table: quantized pushes (a beat is a sixtieth of a second) leave a few centimetres, and the lab says so.

**You try** turns the planner off: the player bounces and leans, and every top is marked.

## Where it lives

`lab.html`, beside the game, as Vagrancy's BT Lab is beside Vagrancy, linked from the tour map. The same core, the same stage and crowd; a new mode in `game.rs` and new modules `expr.rs` (parser and evaluator) and `plot.rs` (the table, the scaling, the planner). One change to the performer: a lab-only way of leaning in which each beat of the arrow key adds a little speed (the game's shows keep their own travel rule).

## Checks

- The parser reads what people type, implicit multiplication included, and refuses what it cannot read with the place it stopped.
- The planner plots x² on [-2, 2] at nine points with every mark within a stated tolerance of its target, and the test is broken once to watch it fail.
- The browser check opens the lab, types a formula, runs it, and sees the table fill.
