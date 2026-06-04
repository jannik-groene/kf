use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use crate::{
    chess::{Move, Position},
    engine::EngineIO,
    evaluate::{evaluate, nnue::NNUEState, Eval},
    search::SearchInfo,
};

use super::tt::TranspositionTable;

pub struct MainThread {
    threads: usize,
    sender: Sender<EngineIO>,
    search_info: SearchInfo,
    search_head: SearchHead,
}

impl MainThread {
    pub fn new(
        pos: Position,
        tt: TranspositionTable,
        history: Vec<Position>,
        stop_flag: Arc<RwLock<bool>>,
        threads: usize,
        use_nnue: bool,
        sender: Sender<EngineIO>,
        search_info: SearchInfo,
    ) -> MainThread {
        MainThread {
            threads,
            sender,
            search_info,
            search_head: SearchHead {
                pos,
                history,
                nodes: 0,
                tt,
                stop_flag,
                killers: Vec::new(),
                nnue: None,
                use_nnue,
                bestmove: None,
            },
        }
    }
    pub fn print_pv(&self, depth: u8, handle: &mut std::io::StdoutLock) {
        if self.search_head.bestmove().is_none() {
            return;
        }
        drop(write!(handle, " pv {}", self.search_head.bestmove().unwrap()));
        let mut pos = self.search_head.pos().from_move(self.search_head.bestmove().unwrap());
        let mut index = 1;
        loop {
            let hashentry = self.search_head.tt.get(pos.zobrist_hash());
            if hashentry.is_none() {
                break;
            }
            let next_move = hashentry.unwrap().mov();
            if next_move.is_none() {
                break;
            }
            drop(write!(handle, " {}", next_move.unwrap()));
            index += 1;
            if index >= depth {
                break;
            }
            pos.do_move(next_move.unwrap());
        }
        drop(writeln!(handle));
    }
    pub fn threads(&self) -> usize {
        self.threads
    }
    pub fn search_head(&self) -> &SearchHead {
        &self.search_head
    }
    pub fn search_head_mut(&mut self) -> &mut SearchHead {
        &mut self.search_head
    }
    #[inline]
    pub fn send_info(&self, io: EngineIO) {
        drop(self.sender.send(io));
    }
    #[inline]
    pub fn search_info(&self) -> &SearchInfo {
        &self.search_info
    }
    #[inline]
    pub fn search_info_mut(&mut self) -> &mut SearchInfo {
        &mut self.search_info
    }
    #[inline]
    pub fn uses_nnue(&self) -> bool {
        self.search_head.use_nnue
    }
}

#[derive(Clone)]
pub struct SearchHead {
    pos: Position,
    history: Vec<Position>,
    nodes: u64,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    killers: Vec<([Option<Move>; 2], [u8; 2])>,
    nnue: Option<NNUEState>,
    use_nnue: bool,
    bestmove: Option<Move>
}

impl SearchHead {
    pub fn new(
        pos: Position,
        tt: TranspositionTable,
        history: Vec<Position>,
        stop_flag: Arc<RwLock<bool>>,
        use_nnue: bool,
    ) -> SearchHead {
        SearchHead {
            pos,
            history,
            nodes: 0,
            tt,
            stop_flag,
            killers: Vec::new(),
            nnue: None,
            use_nnue,
            bestmove: None,
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
    #[inline]
    pub fn nodes(&self) -> &u64 {
        &self.nodes
    }
    #[inline]
    pub fn nodes_mut(&mut self) -> &mut u64 {
        &mut self.nodes
    }
    #[inline]
    pub fn tt(&self) -> &TranspositionTable {
        &self.tt
    }
    #[inline]
    pub fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }
    #[inline]
    pub fn history(&self) -> Vec<Position> {
        self.history.clone()
    }
    #[inline]
    pub fn stop_flag(&self) -> &Arc<RwLock<bool>> {
        &self.stop_flag
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        if self.use_nnue {
            self.nnue.as_mut().unwrap().do_move(m, &self.pos);
        }
        self.history.push(self.pos.clone());
        self.pos.do_move(m);
        self.nodes += 1;
    }
    #[inline]
    pub fn undo_move(&mut self) {
        self.pos = self.history.pop().unwrap();
        if self.use_nnue {
            self.nnue.as_mut().unwrap().undo_move();
        }
    }
    #[inline]
    pub fn do_null_move(&mut self) {
        self.history.push(self.pos.clone());
        self.pos.do_null_move();
        self.nodes += 1;
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
        if self.use_nnue {
            Eval::exact_from_cents(self.nnue.as_ref().unwrap().evaluate_position(self.pos(), self.pos.color()))
        } else {
            evaluate(self.pos_mut())
        }
    }
    #[inline]
    pub fn set_bestmove(&mut self, m: Option<Move>) {
        self.bestmove = m;
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
    pub fn clear_killers(&mut self) {
        self.killers.clear();
    }
    #[inline]
    pub fn bestmove(&self) -> Option<Move> {
        self.bestmove
    }
}
