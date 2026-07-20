use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::chess::Color;
use crate::chess::{Move, Piece, Position};
use crate::evaluate::nnue::{Accumulator, NNUE};
use crate::evaluate::{Bound, eval};
use crate::report::Reporter;

use super::history::History;
use super::tt::TranspositionTable;

#[derive(Copy, Clone)]
pub enum SearchLimit {
    Infinite,
    Depth(u8),
    MoveTime { movetime: Duration },
    Time { soft: Duration, hard: Duration },
}

impl SearchLimit {
    pub fn from_soft_hard_bound(soft: Duration, hard: Duration, safety: Duration) -> Self {
        let hard = hard.saturating_sub(safety).max(Duration::from_millis(1));
        let soft = soft.min(hard);
        Self::Time { soft, hard }
    }
    pub fn from_movetime(movetime: Duration, safety: Duration) -> Self {
        let movetime = movetime
            .saturating_sub(safety)
            .max(Duration::from_millis(1));
        Self::MoveTime { movetime }
    }
    pub fn should_stop(self, start_time: Instant) -> bool {
        if let SearchLimit::MoveTime { movetime } = self
            && start_time.elapsed() >= movetime
        {
            true
        } else if let SearchLimit::Time { hard, .. } = self
            && start_time.elapsed() >= hard
        {
            true
        } else {
            false
        }
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
    pub limit: SearchLimit,
    pub start_time: Instant,
    pub shared: Arc<SharedData>,
    pub next_null: i32,
    pub sel_depth: usize,
    accumulators: [Accumulator; 2],
}

impl SearchHead {
    pub fn new(
        pos: Position,
        shared: Arc<SharedData>,
        start_time: Instant,
        limit: SearchLimit,
    ) -> Self {
        let accumulators = [
            Accumulator::from_pos(&pos, &NNUE, Color::White),
            Accumulator::from_pos(&pos, &NNUE, Color::Black),
        ];
        Self {
            pos,
            history: History::new(),
            pv: [Move::ZERO; 256],
            limit,
            start_time,
            shared,
            next_null: 0,
            sel_depth: 0,
            accumulators,
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
    pub fn set_pos(&mut self, pos: Position) {
        self.pos = pos;
        self.accumulators = [
            Accumulator::from_pos(&self.pos, &NNUE, Color::White),
            Accumulator::from_pos(&self.pos, &NNUE, Color::Black),
        ];
    }
    pub fn shared_data(&self) -> &SharedData {
        &self.shared
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        self.accumulators[0].do_move(m, &self.pos, Color::White, &NNUE);
        self.accumulators[1].do_move(m, &self.pos, Color::Black, &NNUE);
        self.pos.do_move(m);
        self.shared.nodes.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn undo_move(&mut self) {
        let m = self.pos.last_move();
        self.pos.undo_move();
        self.accumulators[0].undo_move(m, &self.pos, Color::White, &NNUE);
        self.accumulators[1].undo_move(m, &self.pos, Color::Black, &NNUE);
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
        NNUE.evaluate(
            &self.accumulators[self.pos.color() as usize],
            &self.accumulators[self.pos.color().other() as usize],
        )
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
}

impl SearchHead {
    pub fn report_update<T: Reporter>(&self, eval: i32, bound: Bound, depth: u8, reporter: &T) {
        let nodes = self.shared.nodes.load(Ordering::Relaxed);
        let time = self.start_time.elapsed().as_millis() as u64;
        let hashfull = self.shared.tt.hash_full();
        reporter.report_update(
            eval,
            bound,
            depth as usize,
            self.sel_depth,
            nodes,
            time,
            hashfull,
            self.pv,
        );
    }

    pub fn report_best_move<T: Reporter>(&self, reporter: &T) {
        let mut vote_map = HashMap::new();
        let mut best_score = -eval::INFTY;
        for vote in self.shared.results.iter().filter_map(|x| {
            let res = x.load(Ordering::Acquire);
            if res == 0 { None } else { Some(res) }
        }) {
            let mv = ((vote >> 8) & 0xffff) as u16;
            let depth = vote & 0xff;
            let eval = ((vote >> 24) as i16) as i32;
            if eval > best_score {
                best_score = eval;
            }
            *vote_map.entry(mv).or_insert(0) += depth as i32 * eval;
        }

        let mv = vote_map
            .iter()
            .max_by(|(_, v), (_, v2)| v.cmp(v2))
            .map_or(Move::ZERO, |(m, _)| Move::decompress(*m));

        reporter.report_result(best_score, mv);

        // Clear out results before next search
        for res in &self.shared.results {
            res.store(0, Ordering::Release);
        }
    }
}
