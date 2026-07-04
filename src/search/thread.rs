use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::evaluate::Bound;
use crate::evaluate::eval::print_eval;
use crate::{
    chess::{Move, Piece, Position},
    evaluate::evaluate,
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
    pub results: [AtomicU64; 8],
}

unsafe impl Sync for SharedData {}

impl SharedData {
    pub fn new() -> Self {
        SharedData {
            nodes: AtomicU64::new(0),
            tt: TranspositionTable::new(16),
            stop_flag: AtomicBool::new(true),
            results: [const { AtomicU64::new(0) }; 8],
        }
    }
}

pub struct SearchHead {
    pub pos: Position,
    pub history: History,
    pub pv: [Move; 256],
    pub time_manager: TimeManager,
    pub shared: Arc<SharedData>,
    pub next_null: i32,
}

impl SearchHead {
    pub fn new(pos: Position, shared: Arc<SharedData>, time_manager: TimeManager) -> SearchHead {
        SearchHead {
            pos,
            history: History::new(),
            pv: [Move::ZERO; 256],
            time_manager,
            shared,
            next_null: 0,
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
    pub fn evaluate(&mut self) -> i32 {
        evaluate(self.pos_mut())
    }
    #[inline]
    pub fn update_quiet_history(&mut self, m: Move, bonus: i32) {
        self.history.quiet.register(self.pos().color(), m, bonus);
    }
    #[inline]
    pub fn update_continuation_history(&mut self, m: Move, bonus: i32) {
        let last_move = self.pos().last_move();
        if last_move == Move::ZERO {
            return;
        }
        let last_piece = if m.typ().is_promotion() {
            Piece::Pawn
        } else {
            self.pos().get_board().piece_at(last_move.to()).unwrap()
        };
        let p = self.pos().get_board().piece_at(m.from()).unwrap();
        self.history
            .continuation
            .register(self.pos().color(), last_piece, last_move, p, m, bonus);
    }
    #[inline]
    pub fn update_capture_history(&mut self, m: Move, bonus: i32) {
        let cap = self
            .pos()
            .get_board()
            .piece_at(m.to())
            .unwrap_or(Piece::Pawn);
        let p = self.pos().get_board().piece_at(m.from()).unwrap();
        self.history.capture.register(p, m, cap, bonus);
    }

    pub fn write_uci_info(&self, eval: i32, bound: Bound, depth: u8) {
        let nodes = self.shared.nodes.load(Ordering::Relaxed);
        print!(
            "info nodes {} nps {} ",
            nodes,
            1000 * u128::from(nodes)
                / self
                    .time_manager
                    .start_time
                    .elapsed()
                    .as_millis()
                    .clamp(1, u128::MAX)
        );
        print_eval(eval, bound);
        print!(
            "depth {} time {} hashfull {}",
            depth,
            self.time_manager.start_time.elapsed().as_millis(),
            self.shared.tt.hash_full()
        );
        if self.pv[0].compress() != 0 {
            print!(" pv");
            for m in &self.pv {
                if m.compress() == 0 {
                    break;
                }
                print!(" {m}");
            }
        }
        println!();
    }

    pub fn write_best_move(&self) {
        let mut vote_map = HashMap::new();
        for vote in self.shared.results.iter().filter_map(|x| {
            let res = x.load(Ordering::Acquire);
            if res == 0 { None } else { Some(res) }
        }) {
            let mv = ((vote >> 8) & 0xffff) as u16;
            let depth = vote & 0xff;
            let eval = ((vote >> 24) as i16) as i32;
            *vote_map.entry(mv).or_insert(0) += depth as i32 * eval;
        }

        if let Some((m, _)) = vote_map.iter().max_by(|(_, v), (_, v2)| v.cmp(v2))
            && *m != 0
        {
            println!("bestmove {}", Move::decompress(*m));
        } else {
            println!("bestmove 0000");
        }
        // Clear out results before next search
        for res in &self.shared.results {
            res.store(0, Ordering::Release);
        }
    }
}
