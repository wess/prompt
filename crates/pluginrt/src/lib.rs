//! Plugin runtime — a `wasmtime` component-model host for WASM plugins (and,
//! later, a warm-process native-tier adapter). Owns the engine, the WIT world
//! bindings, capability-gated linking, and one resident instance per plugin.
//! Defines the [`AppHost`] trait the app implements, keeping `wasmtime` out of
//! the `app` crate's own dependency surface.

use std::collections::HashSet;

use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

/// The generated host/guest bindings for `wit/plugin.wit`.
mod bindings {
    wasmtime::component::bindgen!({
        world: "plugin",
        path: "wit",
    });
}

use bindings::prompt::plugin::types;
/// Wire types shared with the app (mirrors the WIT `types` interface).
pub use bindings::prompt::plugin::types::{CommandTarget, HttpRequest, HttpResponse, LogLevel};

/// The host operations a plugin can call, implemented by the app. Each method
/// mirrors a WIT `host-*` interface function; a gated one is only reachable by a
/// plugin that declared (and was granted) the matching capability, because the
/// host links that interface only then.
pub trait AppHost: Send {
    fn log(&mut self, level: LogLevel, message: String);
    fn storage_get(&mut self, key: String) -> Option<String>;
    fn storage_set(&mut self, key: String, value: String);
    fn run_command(&mut self, text: String, target: CommandTarget) -> Result<(), String>;
    fn send_input(&mut self, bytes: Vec<u8>) -> Result<(), String>;
    fn read_screen(&mut self, lines: u32) -> Result<String, String>;
    fn selection(&mut self) -> Option<String>;
    fn fetch(&mut self, request: HttpRequest) -> Result<HttpResponse, String>;
    fn read_file(&mut self, path: String) -> Result<Vec<u8>, String>;
    fn write_file(&mut self, path: String, data: Vec<u8>) -> Result<(), String>;
    fn clipboard_read(&mut self) -> Result<String, String>;
    fn clipboard_write(&mut self, text: String) -> Result<(), String>;
    fn notify(&mut self, title: String, body: String);
}

/// Store data for one plugin instance: the app host it delegates to, plus a
/// locked-down WASI context (the baseline system interface the guest's `std`
/// needs — no preopened dirs, no sockets, no env; real privileged operations go
/// through the gated `host-*` interfaces, not WASI).
struct State {
    host: Box<dyn AppHost>,
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for State {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

impl types::Host for State {}

impl bindings::prompt::plugin::host_core::Host for State {
    fn log(&mut self, level: LogLevel, message: String) {
        self.host.log(level, message)
    }
    fn storage_get(&mut self, key: String) -> Option<String> {
        self.host.storage_get(key)
    }
    fn storage_set(&mut self, key: String, value: String) {
        self.host.storage_set(key, value)
    }
}

impl bindings::prompt::plugin::host_commands::Host for State {
    fn run_command(&mut self, text: String, target: CommandTarget) -> Result<(), String> {
        self.host.run_command(text, target)
    }
    fn send_input(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.host.send_input(bytes)
    }
}

impl bindings::prompt::plugin::host_screen::Host for State {
    fn read_screen(&mut self, lines: u32) -> Result<String, String> {
        self.host.read_screen(lines)
    }
    fn selection(&mut self) -> Option<String> {
        self.host.selection()
    }
}

impl bindings::prompt::plugin::host_net::Host for State {
    fn fetch(&mut self, request: HttpRequest) -> Result<HttpResponse, String> {
        self.host.fetch(request)
    }
}

impl bindings::prompt::plugin::host_fs::Host for State {
    fn read_file(&mut self, path: String) -> Result<Vec<u8>, String> {
        self.host.read_file(path)
    }
    fn write_file(&mut self, path: String, data: Vec<u8>) -> Result<(), String> {
        self.host.write_file(path, data)
    }
}

impl bindings::prompt::plugin::host_clipboard::Host for State {
    fn read(&mut self) -> Result<String, String> {
        self.host.clipboard_read()
    }
    fn write(&mut self, text: String) -> Result<(), String> {
        self.host.clipboard_write(text)
    }
}

impl bindings::prompt::plugin::host_notify::Host for State {
    fn notify(&mut self, title: String, body: String) {
        self.host.notify(title, body)
    }
}

/// Build the shared component-model engine.
///
/// Resource-bounding a runaway guest (fuel / epoch-interruption with a ticker
/// thread, the analogue of the process runtime's SIGKILL) is a hardening pass on
/// top of this — enabling epoch interruption here without a ticker + per-call
/// deadline just traps every guest immediately.
pub fn engine() -> Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    Engine::new(&config)
}

/// A resident WASM plugin instance: a `Store` plus the instantiated component.
pub struct PluginInstance {
    store: Store<State>,
    world: bindings::Plugin,
}

impl PluginInstance {
    /// Instantiate `wasm` with only the host interfaces its `capabilities` grant
    /// linked. A guest that imports an ungranted interface fails here — that is
    /// the enforced capability boundary. Runs the guest's `init` once.
    pub fn new(
        engine: &Engine,
        wasm: &[u8],
        capabilities: &[String],
        host: Box<dyn AppHost>,
    ) -> Result<Self> {
        let component = Component::new(engine, wasm).context("load plugin component")?;
        let caps: HashSet<&str> = capabilities.iter().map(String::as_str).collect();
        let mut linker: Linker<State> = Linker::new(engine);

        // Baseline WASI (clocks, random, io) so the guest's std initializes. The
        // context is empty — no filesystem, sockets, or env are exposed.
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;

        // Always linked: log + per-plugin storage.
        bindings::prompt::plugin::host_core::add_to_linker(&mut linker, |s: &mut State| s)?;
        if caps.contains("commands") {
            bindings::prompt::plugin::host_commands::add_to_linker(&mut linker, |s: &mut State| s)?;
        }
        if caps.contains("screen") {
            bindings::prompt::plugin::host_screen::add_to_linker(&mut linker, |s: &mut State| s)?;
        }
        if caps.contains("network") {
            bindings::prompt::plugin::host_net::add_to_linker(&mut linker, |s: &mut State| s)?;
        }
        if caps.contains("filesystem") {
            bindings::prompt::plugin::host_fs::add_to_linker(&mut linker, |s: &mut State| s)?;
        }
        if caps.contains("clipboard") {
            bindings::prompt::plugin::host_clipboard::add_to_linker(&mut linker, |s: &mut State| s)?;
        }
        if caps.contains("notify") {
            bindings::prompt::plugin::host_notify::add_to_linker(&mut linker, |s: &mut State| s)?;
        }

        let wasi = WasiCtxBuilder::new().build();
        let mut store = Store::new(
            engine,
            State {
                host,
                wasi,
                table: ResourceTable::new(),
            },
        );
        let world = bindings::Plugin::instantiate(&mut store, &component, &linker)
            .context("instantiate plugin component")?;
        world
            .prompt_plugin_guest()
            .call_init(&mut store)
            .context("plugin init")?;
        Ok(Self { store, world })
    }

    /// Call a tool the plugin exports. Returns the plugin's JSON result, or its
    /// error string. The outer `Result` is a host-side trap (a crashed guest).
    pub fn call_tool(&mut self, name: &str, params_json: &str) -> Result<Result<String, String>> {
        self.world
            .prompt_plugin_guest()
            .call_call_tool(&mut self.store, name, params_json)
    }
}

#[cfg(test)]
#[path = "../tests/pluginrt.rs"]
mod tests;
