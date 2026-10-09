//! Every number that decides how the performer feels, in one place (after
//! Vagrancy's `sim::balance`). Lengths are metres and speeds metres a tick,
//! in Q16.16; angles are `fx::TURN` units. Values marked *(guess)* were set by
//! the agent and wait on a person playing them (SECOND-ORDER row T1).

use crate::fx::{deg, int, ratio, ONE};

/// Ticks per second. The page owns the clock; the core steps this often.
pub const TICKS_PER_SECOND: i32 = 60;

/// 9.81 m/s² at 60 ticks a second: 9.81 / 3600 m a tick², 178 raw.
pub const GRAVITY: i32 = ratio(981, 360_000);
/// No point moves faster than this, metres a tick (15 m/s).
pub const SPEED_CAP: i32 = ratio(1, 4);

// ---- the stage -----------------------------------------------------------

/// The trampoline bed: its top, and its half sizes along x and z.
pub const BED_Y: i32 = int(1);
pub const BED_HALF_X: i32 = ratio(23, 10);
pub const BED_HALF_Z: i32 = ratio(11, 10);
pub const BED_Z: i32 = ratio(4, 10);
/// The deepest the bed gives, in metres.
pub const BED_MAX_DEPTH: i32 = ratio(11, 10);
/// The bed's spring, per tick²: a half cycle of contact lasts about 18 ticks
/// (ω = π/18 a tick, ω² = 0.0305). *(guess)*
pub const BED_K: i32 = ratio(305, 10_000);
/// Pushing the bed (the drive key held while it throws you up) adds this much
/// upward acceleration, per tick². Over a nine-tick push that is about
/// 1.2 m/s a bounce. *(guess)*
pub const DRIVE_ACCEL: i32 = ratio(60, 10_000);
/// Not pushing loses this share of the upward speed each tick of the push. *(guess)*
pub const BED_DAMP: i32 = ratio(1, 80);
/// The kill (the stop key): the bed takes this share of the speed a tick.
pub const KILL_DAMP: i32 = ratio(1, 5);
/// The highest a bounce may take the centre of mass above the bed, metres.
pub const MAX_BOUNCE: i32 = int(7);

/// The food truck: centre, half sizes. Its roof is a stage too.
pub const TRUCK_X: i32 = 0;
pub const TRUCK_Z: i32 = ratio(-37, 10);
pub const TRUCK_HALF_X: i32 = ratio(36, 10);
pub const TRUCK_HALF_Z: i32 = ratio(12, 10);
pub const TRUCK_ROOF: i32 = ratio(32, 10);

// ---- flight --------------------------------------------------------------

/// Angular momentum a held flip key builds per tick of contact with the bed,
/// in turn units a tick at the layout's inertia. Held through the nine ticks
/// of a push it is about one tucked turn from a three-metre bounce; held
/// through the whole contact, about two from five metres. *(guess)*
pub const FLIP_BUILD: i32 = 28;
/// The most flip momentum one takeoff can carry.
pub const FLIP_MAX: i32 = 700;
pub const TWIST_BUILD: i32 = 14;
pub const TWIST_MAX: i32 = 400;
/// In the air a held flip or twist key still adds a little momentum: from
/// nowhere, by design, as Vagrancy's motors do (D9 there; B2 here).
pub const AIR_FLIP: i32 = 3;
pub const AIR_TWIST: i32 = 2;
/// How fast the body's shape moves toward the held pose, share per tick.
pub const SHAPE_RATE: i32 = ratio(1, 5);
/// Travel: the arrow keys give this horizontal speed at takeoff, a tick.
pub const TRAVEL: i32 = ratio(3, 100);
/// The Graph Lab's lean: speed added for each beat an arrow is held on the
/// bed, metres a tick (0.12 m/s a beat). Eighteen beats, a whole contact,
/// give 2.2 m/s.
pub const LEAN_STEP: i32 = ratio(2, 1000);
/// Walking on the ground or the roof, metres a tick (2.4 m/s).
pub const WALK: i32 = ratio(4, 100);
/// Turning on the ground, the bed or the roof, turn units a tick.
pub const TURN_RATE: i32 = deg(3);
/// A jump from the ground or the roof, metres a tick (3.6 m/s, 0.66 m).
pub const JUMP: i32 = ratio(6, 100);

// ---- landings ------------------------------------------------------------

/// Feet within this of upright land on the feet; past it, a crash.
pub const FEET_LIMIT: i32 = deg(55);
/// A drop (back or front) is a landing within this of lying flat.
pub const DROP_LIMIT: i32 = deg(35);
/// Score lost per degree off the ideal landing, and per degree of twist off
/// a half turn, in hundredths of a point.
pub const SCORE_PER_DEGREE: i32 = 110;
pub const SCORE_PER_TWIST_DEGREE: i32 = 45;
/// A bounce must rise this high (centre of mass above the bed) to count as
/// a jump trick such as a tuck jump.
pub const JUMP_TRICK_HEIGHT: i32 = ratio(8, 10);

// ---- bails ---------------------------------------------------------------

/// Verlet relaxation passes per tick (Vagrancy's ITERATIONS).
pub const ITERATIONS: usize = 8;
/// The share of speed the ragdoll loses a tick in the air.
pub const DRAG: i32 = ratio(1, 200);
/// The ground's grip on a lying point, share of sideways travel kept.
pub const GRIP: i32 = ratio(3, 10);
/// How long a crash lies before the performer gets up, and the get-up.
pub const BAIL_TICKS: i32 = 100;
pub const RISE_TICKS: i32 = 30;

// ---- the camera ----------------------------------------------------------

pub const CAM_DISTANCE: i32 = ratio(75, 10);
pub const CAM_HEIGHT: i32 = ratio(16, 10);
pub const CAM_PITCH: i32 = -deg(10);
/// The camera's yaw at the start: behind the performer, from the crowd's side
/// and a little to the left, so a flip is seen at three quarters.
pub const CAM_YAW: i32 = -deg(35);
pub const CAM_FOLLOW: i32 = ONE / 8;
