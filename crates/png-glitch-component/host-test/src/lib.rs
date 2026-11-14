//! A minimal Rust host for the png-glitch component, and the helpers its smoke test uses.
//!
//! ```no_run
//! # fn main() -> wasmtime::Result<()> {
//! let mut host = png_glitch_component_host_test::Host::load(
//!     "target/wasm32-wasip2/release/png_glitch_component.wasm",
//! )?;
//! let png = std::fs::read("in.png")?;
//! let out = host.glitch(&png, "filters:\n  - type: invert\n", 42)?;
//! std::fs::write("out.png", out?)?;
//! # Ok(()) }
//! ```

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

wasmtime::component::bindgen!({
    world: "png-glitch",
    path: "../wit",
});

pub use exports::chikoski::png_glitch::glitch::{FilterInfo, Format, GlitchError, ParamKind};
pub use exports::chikoski::png_glitch::live::{
    ColorType, Encoding, FilterStrategy, FilterType, SeedMode,
};
use wasmtime::component::ResourceAny;

/// A `session` resource of a [`Host`].
pub struct Session(ResourceAny);

struct State {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

/// An instantiated component.
pub struct Host {
    store: Store<State>,
    bindings: PngGlitch,
}

impl Host {
    /// Loads and instantiates the component at `path`.
    pub fn load(path: &str) -> wasmtime::Result<Self> {
        let engine = Engine::default();
        let component = Component::from_file(&engine, path)?;
        let mut linker = Linker::new(&engine);
        let mut options = wasmtime_wasi::p2::bindings::sync::LinkOptions::default();
        options.cli_exit_with_code(true);
        wasmtime_wasi::p2::add_to_linker_with_options_sync(&mut linker, &options)?;
        let mut store = Store::new(
            &engine,
            State {
                ctx: WasiCtxBuilder::new().build(),
                table: ResourceTable::new(),
            },
        );
        let bindings = PngGlitch::instantiate(&mut store, &component, &linker)?;
        Ok(Self { store, bindings })
    }

    /// `list-filters`.
    pub fn list_filters(&mut self) -> wasmtime::Result<Vec<FilterInfo>> {
        self.bindings
            .chikoski_png_glitch_glitch()
            .call_list_filters(&mut self.store)
    }

    /// `validate-recipe`.
    pub fn validate_recipe(&mut self, recipe: &str) -> wasmtime::Result<Result<(), GlitchError>> {
        self.bindings
            .chikoski_png_glitch_glitch()
            .call_validate_recipe(&mut self.store, recipe)
    }

    /// `glitch`.
    pub fn glitch(
        &mut self,
        image: &[u8],
        recipe: &str,
        seed: u64,
    ) -> wasmtime::Result<Result<Vec<u8>, GlitchError>> {
        self.bindings
            .chikoski_png_glitch_glitch()
            .call_glitch(&mut self.store, image, recipe, seed)
    }

    /// `session.from-pixels`.
    pub fn session_from_pixels(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        enc: &Encoding,
    ) -> wasmtime::Result<Result<Session, GlitchError>> {
        let r = self
            .bindings
            .chikoski_png_glitch_live()
            .session()
            .call_from_pixels(&mut self.store, width, height, rgba, enc)?;
        Ok(r.map(Session))
    }

    /// `session.from-png`.
    pub fn session_from_png(
        &mut self,
        png: &[u8],
    ) -> wasmtime::Result<Result<Session, GlitchError>> {
        let r = self
            .bindings
            .chikoski_png_glitch_live()
            .session()
            .call_from_png(&mut self.store, png)?;
        Ok(r.map(Session))
    }

    /// `session.reload-pixels`.
    pub fn reload_pixels(
        &mut self,
        s: &Session,
        rgba: &[u8],
    ) -> wasmtime::Result<Result<(), GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_reload_pixels(&mut self.store, s.0, rgba)
    }

    /// `session.set-recipe`.
    pub fn set_recipe(
        &mut self,
        s: &Session,
        recipe: &str,
        seed: u64,
        mode: SeedMode,
    ) -> wasmtime::Result<Result<(), GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_set_recipe(&mut self.store, s.0, recipe, seed, mode)
    }

    /// `session.apply`.
    pub fn apply(&mut self, s: &Session) -> wasmtime::Result<Result<(), GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_apply(&mut self.store, s.0)
    }

    /// `session.reset`.
    pub fn reset(&mut self, s: &Session) -> wasmtime::Result<Result<(), GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_reset(&mut self.store, s.0)
    }

    /// `session.render-rgba`.
    pub fn render_rgba(&mut self, s: &Session) -> wasmtime::Result<Result<Vec<u8>, GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_render_rgba(&mut self.store, s.0)
    }

    /// `session.snapshot`.
    pub fn snapshot(&mut self, s: &Session) -> wasmtime::Result<Result<Vec<u8>, GlitchError>> {
        self.bindings
            .chikoski_png_glitch_live()
            .session()
            .call_snapshot(&mut self.store, s.0)
    }
}
