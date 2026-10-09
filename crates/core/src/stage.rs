//! The stage every venue shares: the ground, the trampoline bed and the food
//! truck. Where a thing can stand is decided here and only here.

use crate::balance::{
    BED_HALF_X, BED_HALF_Z, BED_Y, BED_Z, TRUCK_HALF_X, TRUCK_HALF_Z, TRUCK_ROOF, TRUCK_X, TRUCK_Z,
};
use crate::geom::V3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Ground,
    Bed,
    Roof,
}

pub fn over_bed(x: i32, z: i32) -> bool {
    x.abs() <= BED_HALF_X && (z - BED_Z).abs() <= BED_HALF_Z
}

pub fn over_roof(x: i32, z: i32) -> bool {
    (x - TRUCK_X).abs() <= TRUCK_HALF_X && (z - TRUCK_Z).abs() <= TRUCK_HALF_Z
}

/// The surface under a point that was at height `from` a moment ago.
pub fn surface_under(x: i32, z: i32, from: i32) -> Surface {
    if over_roof(x, z) && from >= TRUCK_ROOF - TRUCK_ROOF / 8 {
        Surface::Roof
    } else if over_bed(x, z) && from >= BED_Y - BED_Y / 2 {
        Surface::Bed
    } else {
        Surface::Ground
    }
}

pub fn height(s: Surface) -> i32 {
    match s {
        Surface::Ground => 0,
        Surface::Bed => BED_Y,
        Surface::Roof => TRUCK_ROOF,
    }
}

pub fn floor_under(x: i32, z: i32, from: i32) -> i32 {
    height(surface_under(x, z, from))
}

/// A point inside the truck's body, pushed out through the nearest side;
/// `None` when it is outside or above the roof.
pub fn out_of_truck(p: V3, r: i32) -> Option<V3> {
    let (dx, dz) = (p.x - TRUCK_X, p.z - TRUCK_Z);
    let (hx, hz) = (TRUCK_HALF_X + r, TRUCK_HALF_Z + r);
    if dx.abs() >= hx || dz.abs() >= hz || p.y >= TRUCK_ROOF {
        return None;
    }
    let mut out = p;
    if hx - dx.abs() < hz - dz.abs() {
        out.x = TRUCK_X + if dx < 0 { -hx } else { hx };
    } else {
        out.z = TRUCK_Z + if dz < 0 { -hz } else { hz };
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::geom::v3;

    #[test]
    fn the_bed_is_under_its_centre_and_the_roof_under_the_truck() {
        assert_eq!(surface_under(0, BED_Z, int(3)), Surface::Bed);
        assert_eq!(surface_under(TRUCK_X, TRUCK_Z, int(5)), Surface::Roof);
        // below the roof line, the truck is a wall, not a floor
        assert_eq!(surface_under(TRUCK_X, TRUCK_Z, int(1)), Surface::Ground);
        assert_eq!(surface_under(int(9), int(9), int(3)), Surface::Ground);
    }

    #[test]
    fn a_point_inside_the_truck_leaves_by_the_nearest_side() {
        // just inside the side that faces the bed (+z)
        let p = v3(0, int(1), TRUCK_Z + TRUCK_HALF_Z - int(1) / 10);
        let out = out_of_truck(p, 0).expect("inside");
        assert_eq!(out.z, TRUCK_Z + TRUCK_HALF_Z);
        assert_eq!(out.x, 0);
        assert!(out_of_truck(v3(0, int(1), int(5)), 0).is_none());
    }
}
