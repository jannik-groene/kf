use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::{
    chess::{Move, Position},
    evaluate::{Eval, evaluate},
};

use super::history::History;
use super::tt::TranspositionTable;

#[derive(Clone, Copy)]
pub struct TimeManager {
    pub start_time: Instant,
    pub limit: Option<Duration>,
}

impl TimeManager {
    pub fn new(start_time: Instant, limit: Option<Duration>) -> Self {
        TimeManager { start_time, limit }
    }
}

pub struct SharedData {
    pub nodes: AtomicU64,
    pub tt: TranspositionTable,
    pub stop_flag: AtomicBool,
}

unsafe impl Sync for SharedData {}

impl SharedData {
    pub fn new() -> Self {
        SharedData {
            nodes: AtomicU64::new(0),
            tt: TranspositionTable::new(2),
            stop_flag: AtomicBool::new(true),
        }
    }
}

#[derive(Copy, Clone)]
pub struct SearchResult {
    pub eval: Eval,
    pub mv: Move,
    pub depth: u8,
}

pub struct SearchHead {
    pub pos: Position,
    pub history: History,
    pub pv: [Move; 256],
    pub time_manager: TimeManager,
    pub result: Option<SearchResult>,
    pub shared: Arc<SharedData>,
}

impl SearchHead {
    pub fn new(pos: Position, shared: Arc<SharedData>, time_manager: TimeManager) -> SearchHead {
        SearchHead {
            pos,
            history: History::new(),
            pv: [Move::ZERO; 256],
            time_manager,
            result: None,
            shared,
        }
    }
    #[inline]
    pub fn pos_mut(&mut self) -> &mut Position {
        &mut self.pos
    }
    #[inline]
    pub fn pos(&self) -> &Position {
        &self.pos
    }
    pub fn shared_data(&self) -> &SharedData {
        &self.shared
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        self.pos.do_move(m);
        self.shared.nodes.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn undo_move(&mut self) {
        self.pos.undo_move();
    }
    #[inline]
    pub fn do_null_move(&mut self) {
        self.pos.do_null_move();
        self.shared.nodes.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn undo_null_move(&mut self) {
        self.pos.undo_null_move();
    }
    #[inline]
    pub fn evaluate(&mut self) -> Eval {
        evaluate(self.pos_mut())
    }
    //    #[inline]
    //    pub fn register_killer(&mut self, ply: u8, m: Move) {
    //        self.killers.register(m, ply as usize);
    //    }
    //    #[inline]
    //    pub fn get_killers(&self, ply: u8) -> &[Move; 2] {
    //        self.killers.get(ply as usize)
    //    }
    //    #[inline]
    //    pub fn invalidate_killers(&mut self, ply: u8) {
    //        self.killers.invalidate(ply as usize);
    //    }

    pub fn write_uci_info(&self, eval: Eval, depth: u8) {
        let nodes = self.shared.nodes.load(Ordering::Relaxed);
        print!(
            "info nodes {} nps {}",
            nodes,
            1000 * nodes as u128
                / self
                    .time_manager
                    .start_time
                    .elapsed()
                    .as_millis()
                    .clamp(1, u128::MAX)
        );
        print!(
            " {} depth {} time {}",
            eval,
            depth,
            self.time_manager.start_time.elapsed().as_millis()
        );
        if self.pv[0].compress() != 0 {
            print!(" pv");
            for m in self.pv.iter() {
                if m.compress() == 0 {
                    break;
                }
                print!(" {}", m);
            }
        }
        println!();
    }
}
