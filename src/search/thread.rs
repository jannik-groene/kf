use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use crate::{
    chess::{Move, Position},
    evaluate::{evaluate, Eval},
};

use super::tt::TranspositionTable;

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
            stop_flag: AtomicBool::new(true) 
        }
    }
}

#[derive(Copy,Clone)]
pub struct SearchResult {
    pub eval: Eval,
    pub mv: Move,
    pub depth: u8,
}

pub struct SearchHead {
    pub pos: Position,
    pub killers: Vec<([Option<Move>; 2], [u8; 2])>,
    pub pv: [Move; 256],
    pub start_time: Instant,
    pub result: Option<SearchResult>,
    pub shared: Arc<SharedData>,
}

impl SearchHead {
    pub fn new(
        pos: Position,
        shared: Arc<SharedData>,
        _use_nnue: bool,
    ) -> SearchHead {
        SearchHead {
            pos,
            killers: Vec::new(),
            pv: [Move::ZERO; 256],
            start_time: Instant::now(),
            result: None,
            shared
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
    #[inline]
    pub fn register_killer(&mut self, ply: u8, m: Move) {
        if self.killers.len() <= ply as usize {
            self.killers
                .resize(ply as usize + 1, ([None, None], [0, 0]));
        }
        if self.killers[ply as usize].0[0] == Some(m) {
            self.killers[ply as usize].1[0] += 1;
        } else if self.killers[ply as usize].0[1] == Some(m) {
            self.killers[ply as usize].1[1] += 1;
        } else if self.killers[ply as usize].1[0] > self.killers[ply as usize].1[1] {
            self.killers[ply as usize].0[1] = Some(m);
            self.killers[ply as usize].1[1] = 1;
        } else {
            self.killers[ply as usize].0[0] = Some(m);
            self.killers[ply as usize].1[0] = 1;
        }
    }
    #[inline]
    pub fn get_killers(&self, ply: u8) -> &[Option<Move>; 2] {
        if self.killers.len() <= ply as usize {
            &[None, None]
        } else {
            &self.killers[ply as usize].0
        }
    }
    #[inline]
    pub fn invalidate_killers(&mut self, ply: u8) {
        if self.killers.len() > ply as usize + 1 {
            self.killers[ply as usize + 1].1 = [0, 0];
        }
    }
//    pub fn clear_killers(&mut self) {
//        self.killers.clear();
//    }

    pub fn write_uci_info(&self, eval: Eval, depth: u8) {
        let nodes = self.shared.nodes.load(Ordering::Relaxed);
        print!(
            "info nodes {} nps {}",
            nodes,
            1000 * nodes as u128 / self.start_time.elapsed().as_millis().clamp(1, u128::MAX)
        );
        print!(" {} depth {} time {}", eval, depth, self.start_time.elapsed().as_millis());
        if self.pv[0].compress() != 0 {
            print!(" pv");
            for m in self.pv.iter() {
                if m.compress() == 0 { break; } 
                print!(" {}", m);
            }
        }
        println!();
    }
}
