//! ftc_shim: moves values between the page and `ftc_core::game`.
//!
//! Builder requirement: the shim decides nothing. Each function is one call
//! into the core and a conversion of its arguments or result. The core speaks
//! Q16.16 integers; WebGL wants `f32`, so values on the way out are divided by
//! the core's own `ONE`, and the page's float clock and mouse movement are
//! turned into the integers the core expects. `crates/core/tests/boundary.rs`
//! fails if a branch, a loop or the `?` operator appears here.

use ftc_core::{fx, game};
use wasm_bindgen::prelude::*;

const ONE: f32 = fx::ONE as f32;

fn floats(v: Vec<i32>) -> Vec<f32> {
    v.into_iter().map(|x| x as f32 / ONE).collect()
}

#[wasm_bindgen]
pub struct Game {
    inner: game::Game,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(
        body: &str,
        tricks: &str,
        venues: &str,
        missions: &str,
        costumes: &str,
        copy: &str,
        saved: &str,
    ) -> Result<Game, JsValue> {
        game::Game::new(body, tricks, venues, missions, costumes, copy, saved)
            .map(|inner| Game { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// CSS size and device pixel ratio in; drawing-buffer size out.
    pub fn resize(&mut self, css_w: u32, css_h: u32, dpr: f64) -> Vec<i32> {
        self.inner
            .resize(css_w as i32, css_h as i32, (dpr * 100.0) as i32)
            .to_vec()
    }

    /// The page's clock in milliseconds, held action bits, mouse movement in CSS pixels.
    pub fn advance(&mut self, now_ms: f64, bits: u32, dx: f64, dy: f64) {
        self.inner.advance(
            (now_ms * 1000.0) as i64,
            bits,
            (dx * 100.0) as i32,
            (dy * 100.0) as i32,
        )
    }

    pub fn build_frame(&mut self) {
        self.inner.build_frame()
    }

    pub fn view(&self) -> Vec<f32> {
        floats(self.inner.view())
    }

    pub fn instances(&self) -> Vec<f32> {
        floats(self.inner.instances())
    }

    pub fn draws(&self) -> Vec<u32> {
        self.inner.draws()
    }

    pub fn static_instances(&self) -> Vec<f32> {
        floats(self.inner.static_instances())
    }

    pub fn static_draws(&self) -> Vec<u32> {
        self.inner.static_draws()
    }

    pub fn uniforms(&self) -> Vec<f32> {
        floats(self.inner.scene_uniforms())
    }

    pub fn clear_color(&self) -> Vec<f32> {
        floats(self.inner.clear_color())
    }

    pub fn scene_version(&self) -> u32 {
        self.inner.scene_version()
    }

    pub fn hud(&self) -> String {
        self.inner.hud_json()
    }

    pub fn glossary(&self) -> String {
        self.inner.glossary_json()
    }

    pub fn map(&self) -> String {
        self.inner.map_json()
    }

    pub fn select(&mut self, mission: u32) {
        self.inner.select(mission as usize)
    }

    pub fn start(&mut self) {
        self.inner.start()
    }

    pub fn pause(&mut self) {
        self.inner.pause()
    }

    pub fn resume(&mut self) {
        self.inner.resume()
    }

    pub fn to_map(&mut self) {
        self.inner.to_map()
    }

    pub fn wear(&mut self, costume: u32) {
        self.inner.wear(costume as usize)
    }

    pub fn saved(&self) -> String {
        self.inner.saved()
    }

    pub fn reset(&mut self) {
        self.inner.reset()
    }

    pub fn open_everything(&mut self) {
        self.inner.open_everything()
    }

    // ---- the Graph Lab ----

    pub fn lab_open(&mut self) {
        self.inner.lab_open()
    }

    pub fn lab(&self) -> String {
        self.inner.lab_json()
    }

    /// What went wrong reading it, or an empty string.
    pub fn lab_formula(&mut self, src: &str) -> String {
        self.inner.lab_formula(src)
    }

    pub fn lab_domain(&mut self, from: &str, to: &str) -> String {
        self.inner.lab_domain(from, to)
    }

    pub fn lab_points(&mut self, n: u32) {
        self.inner.lab_points(n as usize)
    }

    /// A point of the small graph dragged to `y_px` of `h_px` CSS pixels from its top.
    pub fn lab_drag(&mut self, i: u32, y_px: f64, h_px: f64) {
        self.inner.lab_drag(i as usize, y_px as i32, h_px as i32)
    }

    pub fn lab_run(&mut self, auto: bool) {
        self.inner.lab_run(auto)
    }

    pub fn lab_reset(&mut self) {
        self.inner.lab_reset()
    }

    pub fn lab_speed(&mut self, scale: u32) {
        self.inner.lab_speed(scale)
    }

    pub fn lab_venue(&mut self, venue: u32) {
        self.inner.lab_venue(venue as usize)
    }
}

/// Every number the page needs, as JSON.
#[wasm_bindgen]
pub fn numbers() -> String {
    game::numbers()
}

/// `n` samples of white noise between −1 and 1.
#[wasm_bindgen]
pub fn noise(n: u32, seed: u32) -> Vec<f32> {
    floats(game::noise(n, seed))
}

/// A mesh's vertex data as floats.
#[wasm_bindgen]
pub fn mesh(id: u32) -> Vec<f32> {
    floats(game::mesh_vertices(id))
}
