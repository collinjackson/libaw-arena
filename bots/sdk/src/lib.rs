//! Write an arena bot in Rust: a function from the referee's input to an action, plus `ask` for
//! the engine's questions. Build for wasm32-unknown-unknown (see docs/bot-abi.md).
//!
//! ```ignore
//! fn act(input: &serde_json::Value) -> serde_json::Value {
//!     let legal = arena_bot::ask(&serde_json::json!({"q": "legal_actions"}));
//!     legal[0].clone()
//! }
//! arena_bot::bot!(act);
//! ```
pub use serde_json;
use serde_json::Value;

#[link(wasm_import_module = "libaw")]
extern "C" {
    fn query(ptr: *const u8, len: usize) -> i64;
}

/// The host writes its replies (and the input) into memory it asks for here.
#[no_mangle]
pub extern "C" fn arena_alloc(len: i32) -> i32 {
    let mut v = Vec::<u8>::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as i32
}

fn take(packed: i64) -> Vec<u8> {
    let (p, n) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
    // Safety: the host wrote n bytes at p, into a buffer of capacity n from arena_alloc.
    unsafe { Vec::from_raw_parts(p as *mut u8, n, n) }
}

/// Ask the engine about your own view of the game: {"q": "legal_actions"},
/// {"q": "unit_actions", "unit": ID} or {"q": "is_legal", "action": ACTION}.
pub fn ask(q: &Value) -> Value {
    let b = serde_json::to_vec(q).unwrap_or_default();
    let r = unsafe { query(b.as_ptr(), b.len()) };
    serde_json::from_slice(&take(r)).unwrap_or(Value::Null)
}

#[doc(hidden)]
pub fn __input(ptr: i32, len: i32) -> Value {
    serde_json::from_slice(&take(((ptr as u32 as u64) << 32 | len as u32 as u64) as i64)).unwrap_or(Value::Null)
}

#[doc(hidden)]
pub fn __reply(v: &Value) -> i64 {
    let mut b = serde_json::to_vec(v).unwrap_or_default();
    b.shrink_to_fit();
    let (p, n) = (b.as_ptr() as u64, b.len() as u64);
    std::mem::forget(b);
    (p << 32 | n) as i64
}

/// Exports `arena_act` calling `f(&input) -> action`.
#[macro_export]
macro_rules! bot {
    ($f:path) => {
        #[no_mangle]
        pub extern "C" fn arena_act(ptr: i32, len: i32) -> i64 {
            let input = $crate::__input(ptr, len);
            $crate::__reply(&$f(&input))
        }
    };
}
