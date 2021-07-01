pub mod chess;
pub mod evaluate;
pub mod mcts;
pub mod alphabeta;

pub struct Engine {
    search_trees: Vec<mcts::MCSTree>, //Use root parallelization
//    config: Arc<RwLock<super::config::Config>>,
}

impl Engine {
    pub fn new() -> Engine { //config: Arc<RwLock<super::config::Config>>) -> Engine {
        Engine {
            search_trees: Vec::new(),
            //config,
        }
    }
    pub fn get_pv(&self) {}
    pub fn get_eval(&self) {}
    pub fn get_best_move(&self) {}
    pub fn set_position(&mut self, pos: chess::Position) {}
    pub fn start_search(&mut self) {}
}
