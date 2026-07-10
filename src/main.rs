#![feature(portable_simd)]
mod chess;
mod constants;
mod engine;
mod evaluate;
mod search;
mod uci;
mod report;

use crate::{engine::Engine, uci::UCIHandler};

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = Engine::default();
    engine.uci_loop();
}
