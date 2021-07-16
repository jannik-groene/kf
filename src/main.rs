pub mod engine;
mod uci;

use uci::UCIHandler;

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = engine::Engine::new();
    engine.uci_loop();
}

