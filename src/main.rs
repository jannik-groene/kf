#![feature(portable_simd)]
mod bitboard;
mod board;
mod chess;
mod engine;
mod eval;
mod evaluate;
mod nnue;
mod piecetables;
mod search;
mod thread;
mod tt;
mod uci;
mod constants;

use crate::{engine::Engine, uci::UCIHandler};

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = Engine::new();
    engine.uci_loop();
}
