use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use crate::{
    chess::{Move, Position},
    evaluate::{evaluate, nnue::NNUEState, Eval},
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
    pub history: Vec<Position>,
    pub killers: Vec<([Option<Move>; 2], [u8; 2])>,
    pub nnue: Option<NNUEState>,
    pub pv: [Move; 256],
    pub start_time: Instant,
    pub result: Option<SearchResult>,
    pub shared: Arc<SharedData>,
}

impl SearchHead {
    pub fn new(
        pos: Position,
        history: Vec<Position>,
        shared: Arc<SharedData>,
        _use_nnue: bool,
    ) -> SearchHead {
        SearchHead {
            pos,
            history,
            killers: Vec::new(),
            nnue: None,
            pv: [Move::decompress(0).unwrap(); 256],
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
    pub fn history(&self) -> Vec<Position> {
        self.history.clone()
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        if let Some(nnue) = &mut self.nnue {
            nnue.do_move(m, &self.pos);
        }
        self.history.push(self.pos.clone());
        self.pos.do_move(m);
        self.shared.nodes.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn undo_move(&mut self) {
        self.pos = self.history.pop().unwrap();
        if let Some(nnue) = &mut self.nnue {
            nnue.undo_move();
        }
    }
    #[inline]
    pub fn do_null_move(&mut self) {
        self.history.push(self.pos.clone());
        self.pos.do_null_move();
        self.shared.nodes.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn undo_null_move(&mut self) {
        self.pos = self.history.pop().unwrap();
    }
    #[inline]
    pub fn is_threefold(&self) -> bool {
        self.history.iter().filter(|x| x.zobrist_hash() == self.pos.zobrist_hash()).count() > 1
    }
    #[inline]
    pub fn is_repetition(&self) -> bool {
        self.history.iter().filter(|x| x.zobrist_hash() == self.pos.zobrist_hash()).count() > 0
    }
    #[inline]
    pub fn evaluate(&mut self) -> Eval {
        if let Some(nnue) = &self.nnue {
            Eval::exact_from_cents(nnue.evaluate_position(self.pos(), self.pos.color()))
        } else {
            evaluate(self.pos_mut())
        }
    }
//    #[inline]
//    pub fn set_bestmove(&mut self, m: Option<Move>) {
//        self.bestmove = m;
//    }
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
    pub fn clear_killers(&mut self) {
        self.killers.clear();
    }

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
//    #[inline]
//    pub fn bestmove(&self) -> Option<Move> {
//        self.bestmove
//    }
}
