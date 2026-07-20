#![feature(portable_simd)]
mod chess;
mod constants;
mod engine;
mod evaluate;
mod report;
mod search;
mod uci;

use crate::{engine::Engine, uci::UCIHandler};

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = Engine::default();
    engine.uci_loop();
}
