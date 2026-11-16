//! A bot as a sandboxed WebAssembly module (docs/bot-abi.md).
//!
//! The module may import nothing but `libaw.ask`: no WASI, so no clock, randomness, files or
//! network. Each call runs on a fuel budget (wasmtime's count of executed instructions), so a
//! bot gets the same budget on any machine and a replay runs out at the same point. Memory is
//! capped. A bot that traps (out of fuel, a panic, a bad pointer) is re-instantiated before its
//! next call: its memory between moves is lost, its game goes on.

use libaw::actions::Action;
use libaw::State;
use wasmtime::{Caller, Config, Engine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

/// Limits for every bot alike.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Fuel (roughly instructions) for one `arena_pick` call; the host's answers cost none.
    pub fuel_per_action: u64,
    /// Linear memory, bytes.
    pub memory: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { fuel_per_action: 2_000_000_000, memory: 64 << 20 }
    }
}

/// What the host functions see during a call: the seat's own view of the game.
struct Ctx {
    view: State,
    /// This decision's legal list, which `arena_pick` indexes.
    legal: Vec<Action>,
    limits: StoreLimits,
    queries: u64,
}

/// Why a call gave no usable action.
#[derive(Debug, Clone, PartialEq)]
pub enum Fault {
    /// Ran out of fuel.
    OutOfFuel,
    /// Trapped otherwise (panic, bad pointer, memory limit), or returned a bad reply.
    Trap(String),
}

pub struct Reply {
    /// The legal action the bot picked; None if its pick wasn't -1 or an index into the list.
    pub action: Result<Option<Action>, Fault>,
    pub fuel_used: u64,
    pub queries: u64,
}

pub struct WasmBot {
    pub name: String,
    /// sha256 of the module bytes, as recorded in the game.
    pub hash: String,
    engine: Engine,
    module: Module,
    linker: Linker<Ctx>,
    limits: Limits,
    live: Option<(Store<Ctx>, Instance)>,
}

fn pack(ptr: u32, len: u32) -> i64 {
    ((ptr as u64) << 32 | len as u64) as i64
}
impl WasmBot {
    pub fn new(name: &str, wasm: &[u8], limits: Limits) -> Result<WasmBot, String> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true);
        let engine = Engine::new(&cfg).map_err(|e| e.to_string())?;
        let module = Module::new(&engine, wasm).map_err(|e| format!("not a wasm module: {e}"))?;
        for imp in module.imports() {
            if (imp.module(), imp.name()) != ("libaw", "ask") {
                return Err(format!("imports {}.{}: bots may import only libaw.ask", imp.module(), imp.name()));
            }
        }
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap("libaw", "ask", |mut caller: Caller<'_, Ctx>, kind: i32, arg: i32| -> wasmtime::Result<i64> {
                caller.data_mut().queries += 1;
                let out = arena_abi::answer(&caller.data().view, &caller.data().legal, kind, arg);
                let Some(Extern::Memory(mem)) = caller.get_export("memory") else { wasmtime::bail!("no memory export") };
                let Some(Extern::Func(alloc)) = caller.get_export("arena_alloc") else { wasmtime::bail!("no arena_alloc export") };
                let at = alloc.typed::<i32, i32>(&caller)?.call(&mut caller, out.len() as i32)?;
                mem.write(&mut caller, at as u32 as usize, &out)?;
                Ok(pack(at as u32, out.len() as u32))
            })
            .map_err(|e| e.to_string())?;
        use sha2::Digest;
        let hash = format!("{:x}", sha2::Sha256::digest(wasm));
        Ok(WasmBot { name: name.to_string(), hash, engine, module, linker, limits, live: None })
    }

    fn instance(&mut self, view: State) -> Result<(), String> {
        let legal = arena_abi::legal(&view);
        if let Some((store, _)) = &mut self.live {
            store.data_mut().view = view;
            store.data_mut().legal = legal;
            store.data_mut().queries = 0;
            return Ok(());
        }
        let limits = StoreLimitsBuilder::new().memory_size(self.limits.memory).instances(1).tables(4).memories(1).build();
        let mut store = Store::new(&self.engine, Ctx { view, legal, limits, queries: 0 });
        store.limiter(|c| &mut c.limits);
        store.set_fuel(self.limits.fuel_per_action).map_err(|e| e.to_string())?;
        let inst = self.linker.instantiate(&mut store, &self.module).map_err(|e| format!("instantiate: {e}"))?;
        self.live = Some((store, inst));
        Ok(())
    }

    /// One decision for the seat to move in `view`: the bot picks from the legal list.
    pub fn act(&mut self, view: State) -> Reply {
        let seat = view.current;
        if let Err(e) = self.instance(view) {
            self.live = None;
            return Reply { action: Err(Fault::Trap(e)), fuel_used: 0, queries: 0 };
        }
        let budget = self.limits.fuel_per_action;
        let (store, inst) = self.live.as_mut().unwrap();
        let _ = store.set_fuel(budget);
        let res = (|| -> wasmtime::Result<Option<Action>> {
            let pick: TypedFunc<i32, i32> = inst.get_typed_func(&mut *store, "arena_pick")?;
            let i = pick.call(&mut *store, seat as i32)?;
            Ok(arena_abi::resolve(&store.data().legal, i))
        })();
        let left = store.get_fuel().unwrap_or(0);
        let queries = store.data().queries;
        let fuel_used = budget - left;
        let action = match res {
            Ok(v) => Ok(v),
            Err(e) => {
                let fault = if left == 0 || matches!(e.downcast_ref::<wasmtime::Trap>(), Some(wasmtime::Trap::OutOfFuel)) {
                    Fault::OutOfFuel
                } else {
                    Fault::Trap(format!("{e:#}").chars().take(300).collect())
                };
                self.live = None; // a trapped instance starts fresh next time
                Err(fault)
            }
        };
        Reply { action, fuel_used, queries }
    }
}
