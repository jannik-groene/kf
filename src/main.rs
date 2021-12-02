pub mod engine;
mod uci;

#[cfg(test)]
mod tests;

use uci::UCIHandler;

fn main() {
    let mut engine = engine::Engine::new();
    engine.uci_loop();
}

