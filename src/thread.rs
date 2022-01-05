use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use crate::{
    chess::{Move, Position},
    engine::EngineIO,
    eval::Eval,
    evaluate::evaluate,
    nnue::NNUEState,
    search::SearchInfo,
    tt::TranspositionTable,
};

pub trait Thread {
    fn pos(&self) -> &Position;
    fn pos_mut(&mut self) -> &mut Position;

    fn nodes(&self) -> &u64;
    fn nodes_mut(&mut self) -> &mut u64;

    fn tt_mut(&mut self) -> &mut TranspositionTable;
    fn tt(&self) -> &TranspositionTable;

    fn is_helper(&self) -> bool;

    fn stop_flag(&self) -> &Arc<RwLock<bool>>;

    //These are optional and not used for helper threads
    fn threads(&self) -> usize {
        1
    }
    fn set_bestmove(&mut self, _m: Option<Move>) {}

    fn do_move(&mut self, m: Move);
    fn undo_move(&mut self);

    fn do_null_move(&mut self);
    fn undo_null_move(&mut self);

    fn evaluate(&mut self) -> Eval;

    //save and retrieve killer moves
    fn register_killer(&mut self, ply: u8, m: Move);
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2];
    fn invalidate_killers(&mut self, ply: u8);
}

pub struct MainThread {
    pos: Position,
    nodes: u64,
    threads: usize,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
    sender: Sender<EngineIO>,
    bestmove: Option<Move>,
    killers: Vec<([Option<Move>; 2], [u8; 2])>,
    use_nnue: bool,
    nnue: NNUEState,
}

impl Thread for MainThread {
    #[inline]
    fn pos_mut(&mut self) -> &mut Position {
        &mut self.pos
    }
    #[inline]
    fn pos(&self) -> &Position {
        &self.pos
    }
    #[inline]
    fn nodes(&self) -> &u64 {
        &self.nodes
    }
    #[inline]
    fn nodes_mut(&mut self) -> &mut u64 {
        &mut self.nodes
    }
    #[inline]
    fn tt(&self) -> &TranspositionTable {
        &self.tt
    }
    #[inline]
    fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }
    #[inline]
    fn is_helper(&self) -> bool {
        false
    }
    #[inline]
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {
        &self.stop_flag
    }
    #[inline]
    fn threads(&self) -> usize {
        self.threads
    }
    #[inline]
    fn set_bestmove(&mut self, m: Option<Move>) {
        self.bestmove = m;
    }
    #[inline]
    fn do_move(&mut self, m: Move) {
        self.pos.do_move(m);
        if self.use_nnue {
            self.nnue.do_move(m, &self.pos);
        }
        self.nodes += 1;
    }
    #[inline]
    fn undo_move(&mut self) {
        self.pos.undo_move();
        if self.use_nnue {
            self.nnue.undo_move();
        }
    }
    #[inline]
    fn do_null_move(&mut self) {
        self.pos.do_null_move();
        self.nodes += 1;
    }
    #[inline]
    fn undo_null_move(&mut self) {
        self.pos.undo_null_move();
    }
    #[inline]
    fn evaluate(&mut self) -> Eval {
        if self.use_nnue {
            Eval::exact_from_cents(self.nnue.evaluate_position(self.pos(), self.pos.color()))
        } else {
            evaluate(self.pos_mut())
        }
    }

    #[inline]
    fn register_killer(&mut self, ply: u8, m: Move) {
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
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2] {
        if self.killers.len() <= ply as usize {
            &[None, None]
        } else {
            &self.killers[ply as usize].0
        }
    }
    #[inline]
    fn invalidate_killers(&mut self, ply: u8) {
        if self.killers.len() > ply as usize + 1 {
            self.killers[ply as usize + 1].1 = [0, 0];
        }
    }
}

impl MainThread {
    pub fn new(
        pos: Position,
        threads: usize,
        tt: TranspositionTable,
        stop_flag: Arc<RwLock<bool>>,
        search_info: SearchInfo,
        sender: Sender<EngineIO>,
        use_nnue: bool,
    ) -> MainThread {
        let nnue = NNUEState::new(&pos);
        MainThread {
            pos,
            nodes: 0,
            threads,
            tt,
            stop_flag,
            search_info,
            sender,
            bestmove: None,
            killers: Vec::new(),
            nnue,
            use_nnue,
        }
    }
    #[inline]
    pub fn bestmove(&self) -> Option<Move> {
        self.bestmove
    }
    pub fn print_pv(&self, depth: u8) {
        if self.bestmove().is_none() {
            return;
        }
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        drop(write!(
            handle,
            "info depth {} pv {}",
            depth,
            self.bestmove().unwrap()
        ));
        let mut pos = self.pos().from_move(self.bestmove().unwrap());
        let mut index = 1;
        loop {
            let hashentry = self.tt.get(pos.zobrist_hash());
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
    pub fn send_info(&self, io: EngineIO) {
        drop(self.sender.send(io));
    }
    pub fn search_info(&self) -> &SearchInfo {
        &self.search_info
    }
    pub fn search_info_mut(&mut self) -> &mut SearchInfo {
        &mut self.search_info
    }
    pub fn uses_nnue(&self) -> bool {
        self.use_nnue
    }
}

pub struct HelperThread {
    pos: Position,
    nodes: u64,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    killers: Vec<([Option<Move>; 2], [u8; 2])>,
    nnue: NNUEState,
    use_nnue: bool,
}

impl HelperThread {
    pub fn new(
        pos: Position,
        tt: TranspositionTable,
        stop_flag: Arc<RwLock<bool>>,
        use_nnue: bool,
    ) -> HelperThread {
        let nnue = NNUEState::new(&pos);
        HelperThread {
            pos,
            nodes: 0,
            tt,
            stop_flag,
            killers: Vec::new(),
            nnue,
            use_nnue,
        }
    }
}

impl Thread for HelperThread {
    #[inline]
    fn pos_mut(&mut self) -> &mut Position {
        &mut self.pos
    }
    #[inline]
    fn pos(&self) -> &Position {
        &self.pos
    }
    #[inline]
    fn nodes(&self) -> &u64 {
        &self.nodes
    }
    #[inline]
    fn nodes_mut(&mut self) -> &mut u64 {
        &mut self.nodes
    }
    #[inline]
    fn tt(&self) -> &TranspositionTable {
        &self.tt
    }
    #[inline]
    fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }
    #[inline]
    fn is_helper(&self) -> bool {
        true
    }
    #[inline]
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {
        &self.stop_flag
    }
    #[inline]
    fn do_move(&mut self, m: Move) {
        if self.use_nnue {
            self.nnue.do_move(m, &self.pos);
        }
        self.pos.do_move(m);
        self.nodes += 1;
    }
    #[inline]
    fn undo_move(&mut self) {
        self.pos.undo_move();
        if self.use_nnue {
            self.nnue.undo_move();
        }
    }
    #[inline]
    fn do_null_move(&mut self) {
        self.pos.do_null_move();
        self.nodes += 1;
    }
    #[inline]
    fn undo_null_move(&mut self) {
        self.pos.undo_null_move();
    }
    #[inline]
    fn evaluate(&mut self) -> Eval {
        if self.use_nnue {
            Eval::exact_from_cents(self.nnue.evaluate_position(self.pos(), self.pos.color()))
        } else {
            evaluate(self.pos_mut())
        }
    }
    #[inline]
    fn register_killer(&mut self, ply: u8, m: Move) {
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
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2] {
        if self.killers.len() <= ply as usize {
            &[None, None]
        } else {
            &self.killers[ply as usize].0
        }
    }
    #[inline]
    fn invalidate_killers(&mut self, ply: u8) {
        if self.killers.len() > ply as usize + 1 {
            self.killers[ply as usize + 1].1 = [0, 0];
        }
    }
}
