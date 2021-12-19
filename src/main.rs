#![feature(portable_simd)]
mod engine;
mod uci;
mod chess;
mod search;
mod tt;
mod thread;
mod evaluate;
mod eval;
mod piecetables;
mod nnue;

use crate::{
    engine::Engine,
    uci::UCIHandler,
};

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = Engine::new();
    engine.uci_loop();
}

