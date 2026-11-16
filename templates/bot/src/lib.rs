//! A starting point: attack when it can, else capture, else build, else libaw's baseline pick.
use arena_bot::{ATTACK, CAPTURE, KIND_BUILD, KIND_MOVE};

fn pick(_seat: u32) -> i32 {
    let legal = arena_bot::legal();
    let first = |f: &dyn Fn(&arena_bot::Legal) -> bool| legal.iter().position(|a| f(a)).map(|i| i as i32);
    first(&|a| a.kind == KIND_MOVE && a.then == ATTACK)
        .or_else(|| first(&|a| a.kind == KIND_MOVE && a.then == CAPTURE))
        .or_else(|| first(&|a| a.kind == KIND_BUILD))
        .unwrap_or_else(arena_bot::baseline)
}
arena_bot::bot!(pick);
