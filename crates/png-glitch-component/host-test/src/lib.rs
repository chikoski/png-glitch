//! A minimal Rust host for the png-glitch component, and the helpers its smoke test uses.
//!
//! ```no_run
//! # fn main() -> anyhow::Result<()> {
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
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let engine = Engine::default();
        let component = Component::from_file(&engine, path)?;
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
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
    pub fn list_filters(&mut self) -> anyhow::Result<Vec<FilterInfo>> {
        self.bindings
            .chikoski_png_glitch_glitch()
            .call_list_filters(&mut self.store)
    }

    /// `validate-recipe`.
    pub fn validate_recipe(&mut self, recipe: &str) -> anyhow::Result<Result<(), GlitchError>> {
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
    ) -> anyhow::Result<Result<Vec<u8>, GlitchError>> {
        self.bindings
            .chikoski_png_glitch_glitch()
            .call_glitch(&mut self.store, image, recipe, seed)
    }
}
