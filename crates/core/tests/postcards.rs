//! Every venue on the tour has its postcard (tools/postcards.py) and its
//! TikZ source, so a venue added to data/ without one fails here.

use std::path::Path;

#[test]
fn every_venue_has_a_postcard_and_the_tikz_it_was_drawn_from() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let venues: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("data/venues.json")).unwrap()).unwrap();
    for v in venues["venues"].as_array().unwrap() {
        let id = v["id"].as_str().unwrap();
        let png = root.join(format!("web/{}", ftc_core::game::postcard(id)));
        assert!(png.exists(), "{id}: no postcard at {}", png.display());
        assert!(
            root.join(format!("tools/postcards/{id}.tex")).exists(),
            "{id}: no TikZ source"
        );
    }
}
