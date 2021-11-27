use super::chess;
use super::evaluate;
use std::time::{Instant, Duration};
use std::sync::{Arc, RwLock};
use std::cmp::{PartialOrd, Ord};
use std::ops::Neg;
use std::fmt::Display;
use std::io::{stdout, Write};

use std::sync::mpsc::Sender;
use super::EngineIO;

#[derive(Clone,PartialEq,Copy,Eq)]
pub enum ABResultValueType {
    MATE(i32),
    CENTIS(i32),
    NEGINFTY,
    INFTY,
}

#[derive(Clone,PartialEq,Copy,Eq)]
pub enum ABResultType {
    EXACT,
    UPPERBOUND,
    LOWERBOUND,
}

#[derive(Clone,PartialEq,Copy,Eq)]
pub struct ABResult {
    typ: ABResultType,
    value: ABResultValueType,
}

#[derive(Clone)]
struct ABResultHash {
    //We run with two buckets. One replace on depth, on always replace
    hash: Arc<RwLock<Vec<(ABResultHashEntry, ABResultHashEntry)>>>,
    size: usize,
}

impl ABResultHash {
    fn new(size: usize) -> Self {
        let mut hash_vec = Vec::with_capacity(size);
        for _ in 0..size {
            hash_vec.push((ABResultHashEntry::UNCHECKED, ABResultHashEntry::UNCHECKED));
        }
        hash_vec.shrink_to_fit();
        ABResultHash {
            size,
            hash: Arc::new(RwLock::new(hash_vec)),
        }
    }
    #[inline(always)]
    fn get(&self, zobrist_key: u64) -> Option<ABResultHashEntry> {
        let entry = self.hash.read().unwrap()[zobrist_key as usize % self.size];
        if entry.0.zobrist_hash == zobrist_key {
            Some(entry.0)
        } else if entry.1.zobrist_hash == zobrist_key {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline(always)]
    fn set(&mut self, zobrist_key: u64, entry: ABResultHashEntry) {
        let mut hash = self.hash.write().unwrap();
        if zobrist_key == 0 {
            println!("debug Zero Zobrist Key found!");
        }
        //Mate scores may be seen as having infinite depth
        if hash[zobrist_key as usize % self.size].0.depth < entry.depth ||
            (matches!(entry.res.value, ABResultValueType::MATE(_))
                && entry.res > hash[zobrist_key as usize % self.size].0.res) {
            hash.get_mut(zobrist_key as usize % self.size).unwrap().0 = entry;
        } else {
            hash.get_mut(zobrist_key as usize % self.size).unwrap().1 = entry;
        }
    }
}

impl ABResult {
    const MIN: ABResult = ABResult {typ: ABResultType::EXACT, value: ABResultValueType::NEGINFTY};
    const MAX: ABResult = ABResult {typ: ABResultType::EXACT, value: ABResultValueType::INFTY};
    const MATE_NOW: ABResult = ABResult {typ: ABResultType::EXACT, value: ABResultValueType::MATE(0)};
    const STALEMATE: ABResult = ABResult {typ: ABResultType::EXACT, value: ABResultValueType::CENTIS(0)};
    const DRAW: ABResult = ABResult {typ: ABResultType::EXACT, value: ABResultValueType::CENTIS(0)};
    fn exact_from_cents(centis: i32) -> ABResult {
        ABResult {typ: ABResultType::EXACT, value: ABResultValueType::CENTIS(centis)}
    }
    fn lowerbound_from_cents(centis: i32) -> ABResult {
        ABResult {typ: ABResultType::LOWERBOUND, value: ABResultValueType::CENTIS(centis)}
    }
    fn upperbound_from_cents(centis: i32) -> ABResult {
        ABResult {typ: ABResultType::UPPERBOUND, value: ABResultValueType::CENTIS(centis)}
    }
    fn to_exact(&self) -> ABResult {
        ABResult {typ: ABResultType::EXACT, value: self.value}
    }
    fn to_lowerbound(&self) -> ABResult {
        ABResult {typ: ABResultType::LOWERBOUND, value: self.value}
    }
    fn to_upperbound(&self) -> ABResult {
        ABResult {typ: ABResultType::UPPERBOUND, value: self.value}
    }
    const ASPIRATION_ADJUSTMENTS: [i32; 5] = [25, 50, 200, 400, 800];
    fn aspiration_lower(&self, count: usize) -> ABResult {
        if count >= 5 {
            return ABResult{typ: ABResultType::EXACT, value: ABResultValueType::NEGINFTY};
        }
        match self.value {
            ABResultValueType::CENTIS(c) => ABResult { typ: self.typ, value: ABResultValueType::CENTIS(c-ABResult::ASPIRATION_ADJUSTMENTS[count]) },
            //In case of mate take the next worse mate score, e.g. mate in 2 if we are being mated
            //in 3 or mate in 5 if we will mate in 3
            ABResultValueType::MATE(m) => ABResult{ typ: self.typ, value: ABResultValueType::MATE(m+2*(2*(m%2)-1)) },
            _ => ABResult{typ: ABResultType::EXACT, value: self.value}
        }
    }
    fn aspiration_higher(&self, count: usize) -> ABResult {
        if count >= 5 {
            return ABResult{typ: ABResultType::EXACT, value: ABResultValueType::INFTY};
        }
        match self.value {
            ABResultValueType::CENTIS(c) => ABResult { typ: self.typ, value: ABResultValueType::CENTIS(c+ABResult::ASPIRATION_ADJUSTMENTS[count]) },
            //In case of mate take the next best mate score, e.g. mate in 4 if we are being mated
            //in 2 or mate in 3 if we will mate in 5
            ABResultValueType::MATE(m) => ABResult{ typ: self.typ, value: ABResultValueType::MATE(m-2*(2*(m%2)-1)) },
            _ => ABResult{typ: ABResultType::EXACT, value: ABResultValueType::INFTY}
        }

    }
    fn zero_window(&self) -> ABResult {
        match self.value {
            ABResultValueType::CENTIS(c) => ABResult { value: ABResultValueType::CENTIS(c+1), typ: self.typ },
            ABResultValueType::MATE(n) => ABResult { value: ABResultValueType::MATE(n-2*(2*(n%2)-1)), typ: self.typ },
            ABResultValueType::INFTY => *self,
            ABResultValueType::NEGINFTY => *self,
        }
    }
    //Use to pass ab-bounds down the tree
    fn neg_down(&self) -> ABResult {
        let val = match self.value {
            ABResultValueType::MATE(m) => ABResultValueType::MATE(m-1),
            ABResultValueType::CENTIS(c) => ABResultValueType::CENTIS(-c),
            ABResultValueType::NEGINFTY => ABResultValueType::INFTY,
            ABResultValueType::INFTY => ABResultValueType::NEGINFTY,
        };
        ABResult {value: val, typ: self.typ}
    }
}

impl Display for ABResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match write!(f,"score ") {
            Err(e) => {return std::fmt::Result::Err(e);},
            _ => {},
        }
        match {match self.value {
            ABResultValueType::CENTIS(c) => write!(f, "cp {}", c),
            ABResultValueType::INFTY => write!(f, "cp 100000000"),
            ABResultValueType::NEGINFTY => write!(f, "cp -100000000"),
            ABResultValueType::MATE(m) =>write!(f, "mate {}", (2*(m%2)-1)*(m+1)/2),
            }} {
            Err(e) => {return std::fmt::Result::Err(e);},
            _ => {},
        }
        match self.typ {
            ABResultType::LOWERBOUND => write!(f," lowerbound"),
            ABResultType::UPPERBOUND => write!(f," upperbound"),
            ABResultType::EXACT => write!(f,""),
        }
    }
}

//Neg moves a result UP the searchtree, i.e. mates become father away. Use ABResult::neg_down
impl Neg for ABResultValueType {
    type Output = Self;
    fn neg(self) -> Self {
        match self {
            Self::MATE(m) => Self::MATE(m+1),
            Self::CENTIS(c) => Self::CENTIS(-c),
            Self::NEGINFTY => Self::INFTY,
            Self::INFTY => Self::NEGINFTY,
        }
    }
}

impl Neg for ABResult {
    type Output = ABResult;
    fn neg(self) -> Self {
        ABResult {
            typ: match self.typ {
                ABResultType::EXACT => ABResultType::EXACT,
                ABResultType::UPPERBOUND => ABResultType::LOWERBOUND,
                ABResultType::LOWERBOUND => ABResultType::UPPERBOUND,
            },
            value: -self.value,
        }
    }
}

impl Ord for ABResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialOrd for ABResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>{
        match self.value {
            ABResultValueType::MATE(m) => match other.value {
                ABResultValueType::MATE(m2) => {
                    //Note that we have m2.cmp(m), since mate in 3 plys is better than mate in
                    //5 plys
                    if m % 2 == 1 && m2 % 2 == 1 { Some(m2.cmp(&m)) }
                    else if m % 2 == 1 && m2 % 2 == 0 { Some(std::cmp::Ordering::Greater) }
                    else if m % 2 == 1 && m2 % 2 == 1 { Some(std::cmp::Ordering::Less) }
                    //If we get mated, a long time off is best!
                    else { Some(m.cmp(&m2)) }
                }
                ABResultValueType::INFTY => Some(std::cmp::Ordering::Less),
                ABResultValueType::NEGINFTY => Some(std::cmp::Ordering::Greater),
                ABResultValueType::CENTIS(_) => if m % 2 == 1 { Some(std::cmp::Ordering::Greater) } else { Some(std::cmp::Ordering::Less) },
            },
            ABResultValueType::INFTY => match other.value {
                ABResultValueType::INFTY => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Greater),
            },
            ABResultValueType::NEGINFTY => match other.value {
                ABResultValueType::NEGINFTY => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Less),
            },
            ABResultValueType::CENTIS(c) => match other.value {
                ABResultValueType::MATE(m) => if m % 2 == 1 { Some(std::cmp::Ordering::Less) } else { Some(std::cmp::Ordering::Greater) },
                ABResultValueType::INFTY => Some(std::cmp::Ordering::Less),
                ABResultValueType::NEGINFTY => Some(std::cmp::Ordering::Greater),
                ABResultValueType::CENTIS(c2) => Some(c.cmp(&c2)),
            }
        }
    }
}

#[derive(Clone,PartialEq,Copy)]
pub struct ABResultHashEntry {
    res: ABResult,
    depth: u8,
    zobrist_hash: u64,
    mov: chess::CompressedMove,
}

impl ABResultHashEntry {
    const UNCHECKED: ABResultHashEntry = ABResultHashEntry {res: ABResult::MIN, depth: 0, zobrist_hash: 0, mov: chess::CompressedMove{to:0, from:0, piece_and_type:0}};
    fn mov(&self) -> Option<chess::Move> {
        self.mov.decompress()
    }
    fn new(res: ABResult, depth: u8, zobrist_hash: u64, mov: chess::Move) -> ABResultHashEntry {
        ABResultHashEntry {
            res,
            depth,
            zobrist_hash,
            mov: mov.compress(),
        }
    }
}

#[derive(Clone)]
pub struct SearchInfo {
    pub bestmove: Option<chess::Move>,
    pub eval: ABResult,
    pub depth: u8,
    pub pv: Vec<chess::Move>,
    pub nodes: u64,
    pub id: u64,
}

impl SearchInfo {
    fn new(id: u64) -> SearchInfo {
        SearchInfo {
            bestmove: None,
            eval: ABResult::MIN,
            depth: 0,
            pv: Vec::new(),
            nodes: 0,
            id,
        }
    }
}

pub struct ABSearchManager {
    pos: chess::Position,
    threads: usize,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
    //nnue: evaluate::nnue::NNUEState,
}

impl ABSearchManager {
    pub fn new() -> ABSearchManager {
        ABSearchManager {
            pos: chess::Position::new(),
            threads: 1,
            move_hash: ABResultHash::new(2),
            stop_flag: Arc::new(RwLock::new(false)),
            search_info: SearchInfo::new(0),
            //nnue: evaluate::nnue::NNUEState::from_weights(std::path::Path::new("/home/jannik/Code/kf/model.nnue")),
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        self.move_hash = ABResultHash::new(size*1_000_000/std::mem::size_of::<(ABResultHashEntry, ABResultHashEntry)>());
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }
    pub fn set_position(&mut self, pos: chess::Position) {
        //self.nnue.initialize_state(&pos);
        self.pos = pos;
    }
    pub fn search(&mut self, out_channel: Sender<EngineIO>, target_depth: Option<u8>, search_id: u64) -> std::thread::JoinHandle<()> {
        let depth = target_depth.unwrap_or(u8::MAX);
        self.reset_search_info(search_id);
        self.stop_flag = Arc::new(RwLock::new(false));
        let mut root_search_info = ABSearchMainThread {
            pos: self.pos.clone(),
            nodes: 0,
            move_hash: self.move_hash.clone(),
            threads: self.threads,
            stop_flag: self.stop_flag.clone(),
            search_info: self.search_info.clone(),
            sender: out_channel,
            bestmove: None,
            //nnue: self.nnue.clone(),
        };
        std::thread::spawn(move || search(&mut root_search_info, depth, ABResult::MIN, ABResult::MAX))
    }
    pub fn reset_search_info(&mut self, id: u64) {
        self.search_info = SearchInfo::new(id);
    }
    pub fn stop(&mut self) {
        *self.stop_flag.write().unwrap() = true;
    }
    pub fn color(&self) -> chess::Color {
        self.pos.color()
    }
    pub fn reset_hash(&mut self) {
        let s = self.move_hash.size;
        self.move_hash = ABResultHash::new(0);
        self.move_hash = ABResultHash::new(s);
    }
}

trait ABSearchThread {
    fn pos(&self) -> &chess::Position;
    fn pos_mut(&mut self) -> &mut chess::Position;
    fn nodes(&self) -> &u64;
    fn nodes_mut(&mut self) -> &mut u64;
    fn move_hash_mut(&mut self) -> &mut ABResultHash;
    fn move_hash(&self) -> &ABResultHash;
    fn is_helper(&self) -> bool;
    fn stop_flag(&self) -> &Arc<RwLock<bool>>;
    //These are optional and not used for helper threads
    fn threads(&self) -> usize {1}
    fn set_bestmove(&mut self, _m: Option<chess::Move>) {}
    //fn nnue(&mut self) -> &mut evaluate::nnue::NNUEState;
    fn do_move(&mut self, m: chess::Move);
    fn undo_move(&mut self, m: chess::Move, castling: [[bool; 2]; 2], lm: Option<chess::Move>);
    fn evaluate(&mut self) -> i32;
}

struct ABSearchMainThread {
    pos: chess::Position,
    nodes: u64,
    threads: usize,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
    sender: Sender<EngineIO>,
    bestmove: Option<chess::Move>,
    //nnue: evaluate::nnue::NNUEState,
}

impl ABSearchThread for ABSearchMainThread {
    fn pos_mut(&mut self) -> &mut chess::Position {&mut self.pos}
    fn pos(&self) -> &chess::Position {&self.pos}
    fn nodes(&self) -> &u64 {&self.nodes}
    fn nodes_mut(&mut self) -> &mut u64 {&mut self.nodes}
    fn move_hash(&self) -> &ABResultHash {&self.move_hash}
    fn move_hash_mut(&mut self) -> &mut ABResultHash {&mut self.move_hash}
    fn is_helper(&self) -> bool {false}
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {&self.stop_flag}
    fn threads(&self) -> usize {self.threads}
    fn set_bestmove(&mut self, m: Option<chess::Move>) {
        self.bestmove = m;
    }
    //fn nnue(&mut self) -> &mut evaluate::nnue::NNUEState {&mut self.nnue}
    fn do_move(&mut self, m: chess::Move) {
        //self.nnue.do_move(m, &self.pos);
        self.pos.do_move(m);
    }
    fn undo_move(&mut self, m: chess::Move, castling: [[bool; 2]; 2], lm: Option<chess::Move>) {
        self.pos.undo_move(m, castling, lm);
        //self.nnue.undo_move(m, &self.pos);
    }
    fn evaluate(&mut self) -> i32 {
        //self.nnue.evaluate_position(&self.pos) / 10
        evaluate::evaluate(self.pos_mut())
    }
}

impl ABSearchMainThread {
    fn bestmove(&self) -> Option<chess::Move> {
        self.bestmove
    }
    fn print_pv(&self, depth: u8) {
        if self.bestmove().is_none() {return;}
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        drop(write!(handle, "info depth {} pv {}", depth, self.bestmove().unwrap()));
        let mut pos = self.pos().from_move(self.bestmove().unwrap());
        let mut index = 1;
        loop {
            let hashentry = self.move_hash.get(pos.zobrist_hash());
            if hashentry.is_none() {break;}
            let next_move = hashentry.unwrap().mov();
            if next_move.is_none() {break;}
            drop(write!(handle, " {}", next_move.unwrap()));
            index += 1;
            if index >= depth {break;}
            pos.do_move(next_move.unwrap());
        }
        drop(write!(handle, "\n"));
    }
}

struct ABSearchHelperThread {
    pos: chess::Position,
    nodes: u64,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    //nnue: evaluate::nnue::NNUEState,
}

impl ABSearchThread for ABSearchHelperThread {
    fn pos_mut(&mut self) -> &mut chess::Position {&mut self.pos}
    fn pos(&self) -> &chess::Position {&self.pos}
    fn nodes(&self) -> &u64 {&self.nodes}
    fn nodes_mut(&mut self) -> &mut u64 {&mut self.nodes}
    fn move_hash(&self) -> &ABResultHash {&self.move_hash}
    fn move_hash_mut(&mut self) -> &mut ABResultHash {&mut self.move_hash}
    fn is_helper(&self) -> bool {true}
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {&self.stop_flag}
    //fn nnue(&mut self) -> &mut evaluate::nnue::NNUEState {&mut self.nnue}
    fn do_move(&mut self, m: chess::Move) {
        //self.nnue.do_move(m, &self.pos);
        self.pos.do_move(m);
    }
    fn undo_move(&mut self, m: chess::Move, castling: [[bool; 2]; 2], lm: Option<chess::Move>) {
        self.pos.undo_move(m, castling, lm);
        //self.nnue.undo_move(m, &self.pos);
    }
    fn evaluate(&mut self) -> i32 {
        //self.nnue.evaluate_position(&self.pos) / 10
        evaluate::evaluate(self.pos_mut())
    }
}

impl ABSearchHelperThread {
    fn search(&mut self, depth: u8, alpha: ABResult, beta: ABResult, lm: Option<chess::Move>) -> u64 {
        search_step(self, depth, 0, 0, 0, 0, alpha, beta, lm);
        self.nodes
    }
}

fn search(thread: &mut ABSearchMainThread, depth: u8, mut alpha: ABResult, mut beta: ABResult) {
    let now = Instant::now();
    let mut helper_handles = Vec::new();
    let helper_stop_flag = Arc::new(RwLock::new(false));
    for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        loop {
            //Spawn helper threads for search
            if thread.threads() > 1 && d > 2 {
                for i in 1..thread.threads() {
                    let mut helper_thread = ABSearchHelperThread {
                        pos: thread.pos().clone(),
                        nodes: 0,
                        move_hash: thread.move_hash().clone(),
                        stop_flag: helper_stop_flag.clone(),
                        //nnue: thread.nnue().clone(),
                    };
                    helper_handles.push(std::thread::spawn(move || helper_thread.search(d + i as u8 / 2, alpha, beta, None)));
                }
            }
            let eval = search_step(thread, d, 0, 0, 0, 0, alpha, beta, thread.pos().get_last_move());
            if thread.stop_flag().read().unwrap().eq(&true) {
                *helper_stop_flag.write().unwrap() = true;
                drop(thread.sender.send(EngineIO::SEARCHENDED(thread.search_info.id)));
                return;
            }
            if d > 2 {
                //Set stop flag and join all helpers
                *helper_stop_flag.write().unwrap() = true;
                for helper in helper_handles {
                    match helper.join() {
                        //Concerning but not fatal?
                        Err(_) => {},
                        Ok(n) => *thread.nodes_mut() += n,
                    }
                }
                helper_handles = Vec::new();
                //Reset stop flag
                *helper_stop_flag.write().unwrap() = false | *thread.stop_flag.read().unwrap();
            }
            println!("info nodes {} nps {}", thread.nodes(), 1000* *thread.nodes() as u128 /now.elapsed().as_millis().clamp(1,u128::MAX));
            //We reached the target depth and stopped, so we update the external values
            match eval.typ {
                ABResultType::EXACT => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    thread.print_pv(d);
                    thread.search_info.eval = eval;
                    thread.search_info.bestmove = thread.bestmove();
                    match thread.sender.send(EngineIO::SEARCHUPDATE(thread.search_info.clone())) {
                        Ok(_) => {},
                        //We end the search if we cannot communicate the results anymore
                        Err(_) => return,
                    };
                    alpha = eval.aspiration_lower(0);
                    beta = eval.aspiration_higher(0);
                    break;
                },
                ABResultType::LOWERBOUND => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    fail_highs += 1;
                    beta = eval.aspiration_higher(fail_highs);
                }
                ABResultType::UPPERBOUND => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    fail_lows += 1;
                    alpha = eval.aspiration_lower(fail_lows);
                }
            }
        }
    }
    drop(thread.sender.send(EngineIO::SEARCHENDED(thread.search_info.id)));
}

//Parameters:
// thread: The search thread head.
// depth: the depth to search to
// ply; the current depth
// depth_reduction: how far to reduce the search depth
// extension: how long to extend the search !! PROVIDED IN QUARTER PLYS !!
//            a value of of 4,5,6,7 all mean search is extended by one ply
// null_moves: how many null moves have been performed in the current search
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
// lm: the previous move. Needed since the last move needs to be restored
fn search_step(thread: &mut impl ABSearchThread, depth: u8, ply: u8, depth_reduction: u8, mut extension: u8,
               null_moves: u8, mut alpha: ABResult, beta: ABResult, lm: Option<chess::Move>) -> ABResult {

    *thread.nodes_mut() += 1;

    //Check if the move is already hashed
    let hash_entry = thread.move_hash().get(thread.pos().zobrist_hash());

    if hash_entry.is_some() && hash_entry.unwrap().mov().is_some() {
        if hash_entry.unwrap().depth >= depth-ply {
            match hash_entry.unwrap().res.typ {
                ABResultType::EXACT => {
                    thread.set_bestmove(hash_entry.unwrap().mov());
                    return hash_entry.unwrap().res;
                },
                ABResultType::LOWERBOUND => {
                    if hash_entry.unwrap().res >= beta {
                        return hash_entry.unwrap().res;
                    }
                },
                ABResultType::UPPERBOUND => {
                    if hash_entry.unwrap().res < alpha {
                        return hash_entry.unwrap().res;
                    }
                }
            }
        }
    }

    //Check if this is a terminal position
    let mut moves = thread.pos_mut().get_moves();

    if moves.len() == 0 && thread.pos_mut().in_check() {
        return ABResult::MATE_NOW;
    } else if moves.len() == 0 {
        return ABResult::STALEMATE;
    }

    //If we cannot beat the score, just return immediately
    if beta.value == ABResultValueType::MATE(0) {
        return ABResult::MATE_NOW.to_lowerbound()
    }

    //Try a null move to find a beta cutoff; search the first three plys fully.
    if null_moves < 2 && !thread.pos_mut().in_check() && moves.len() > 0 && ply > 3 && ply < depth
                      && !matches!(alpha.value, ABResultValueType::MATE(_))
                      && !matches!(beta.value, ABResultValueType::MATE(_)) {
        thread.pos_mut().do_null_move();
        let null_score = -search_step(thread,
                                      depth,
                                      ply+1,
                                      depth_reduction,
                                      0,
                                      null_moves + 1,
                                      beta.neg_down(),
                                      alpha.neg_down(),
                                      None);
        thread.pos_mut().undo_null_move(lm);
        if null_score >= beta {
            return null_score.to_lowerbound();
        }
    }

    //We extend the normal search if we are  in check, else go into quiescence
    if ply + depth_reduction >= depth + extension/4 && !thread.pos_mut().in_check() {
        return quiesce(thread, alpha, beta, 200 - 20 * depth_reduction as i32, 0, lm);
    }
    //Futility pruning
    else if ply + depth_reduction == depth + extension/4 - 1 && depth > 3 {
        let eval = thread.evaluate();//evaluate::evaluate(thread.pos_mut());
        if ABResult::exact_from_cents(eval + 300) < alpha {
            return quiesce(thread, alpha, beta, 100, 0, lm);
        }
    }
    //Extended futility pruning
    else if depth > 3 && ply + depth_reduction == depth + extension/4 - 2 {
        let eval = thread.evaluate();//evaluate::evaluate(thread.pos_mut());
        if ABResult::exact_from_cents(eval + 500) < alpha {
            return quiesce(thread, alpha, beta, 100, 0, lm);
        }
    }

    //Store castling rights for move undoing
    let castling = thread.pos().get_castling_rights();

    //if !thread.is_helper() {
    evaluate::order_moves(&mut moves, thread.pos(), thread.move_hash().get(thread.pos().zobrist_hash()).map(|h| h.mov()).flatten());
    //} else {
    //    evaluate::order_moves_with_random_bias(&mut moves, thread.pos(), thread.move_hash().get(thread.pos().zobrist_hash()).mov());
    //}

    //Set up paramaters
    let mut score = ABResult::MIN;
    let mut fail_low = true;
    let mut bestmove = None;
    let mut zws = false;

    if moves.len() == 1 {
        extension += 4;
    }

    for i in 0..moves.len() {
        //search deeper along the PV
        let pv_extension = if hash_entry.is_some()
                                && hash_entry.unwrap().mov.decompress().unwrap() == moves[i] {1} else {0};

        thread.do_move(moves[i]);

        let mut movescore = if thread.pos().pos_in_history() {
                            //If we repeat twice, it's gonna happen thrice
                                ABResult::DRAW
                            //Apply LMR to zws searches of late moves
                            } else if ply > 3 && !thread.pos_mut().in_check() && zws {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             std::cmp::min(((i as u8)/4)*2 + depth_reduction,
                                                            depth / 4),
                                             null_moves,
                                             0,
                                             alpha.zero_window().neg_down(),
                                             alpha.neg_down(),
                                             Some(moves[i]))
                            } else if zws {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             0,
                                             0,
                                             null_moves,
                                             alpha.zero_window().neg_down(),
                                             alpha.neg_down(),
                                             Some(moves[i]))
                            } else {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             0,
                                             extension + pv_extension,
                                             null_moves,
                                             beta.neg_down(),
                                             alpha.neg_down(),
                                             Some(moves[i]))
                            };
        if zws && movescore > alpha && movescore < beta {
            //We apply no LMR if we search for a PV
            movescore = -search_step(thread,
                                     depth,
                                     ply+1,
                                     0,
                                     extension + pv_extension,
                                     null_moves,
                                     beta.neg_down(),
                                     alpha.neg_down(),
                                     Some(moves[i]));
        }
        thread.undo_move(moves[i], castling, lm);
        //Abort search if the helper gets a stop signal
        if thread.stop_flag().read().unwrap().eq(&true) {return ABResult::MIN;};
        if movescore >= beta {
            let zh = thread.pos().zobrist_hash();
            thread.move_hash_mut().set(zh, ABResultHashEntry::new(movescore.to_lowerbound(), depth-ply, zh, moves[i]));
            return movescore.to_lowerbound();
        }
        if movescore > score {
            bestmove = Some(moves[i]);
            score = movescore;
            if score > alpha {
                zws = true;
                fail_low = false;
                alpha = score;
            }
        }
    }

    if ply == 0 {
        thread.set_bestmove(bestmove);
    }

    let zh = thread.pos().zobrist_hash();
    if fail_low {
        thread.move_hash_mut().set(zh, ABResultHashEntry::new(score.to_upperbound(), depth-ply, zh, bestmove.unwrap()));
        score.to_upperbound()
    } else {
        thread.move_hash_mut().set(zh, ABResultHashEntry::new(score.to_exact(), depth-ply, zh, bestmove.unwrap()));
        score.to_exact()
    }
}

fn quiesce(thread: &mut impl ABSearchThread, mut alpha: ABResult, beta: ABResult, delta: i32, qply: u8, lm: Option<chess::Move>) -> ABResult {

    *thread.nodes_mut() += 1;

    let mut cand_moves = thread.pos_mut().get_moves();

    //check for terminal position
    if cand_moves.len() == 0 && thread.pos_mut().in_check() {
        return ABResult::MATE_NOW;
    }

    let static_eval_centis = thread.evaluate();//evaluate::evaluate(thread.pos_mut());
    let static_eval = ABResult::exact_from_cents(static_eval_centis);

    //If we are not in check we filter for tactical moves.
    if !thread.pos_mut().in_check() {

        //Adjust based on null-move hypothesis
        if static_eval >= beta {
            return static_eval.to_lowerbound();
        } else if alpha < static_eval {
            alpha = static_eval;
        }

        cand_moves = cand_moves.iter().copied().filter(|m| match m.typ {
                                                        chess::MoveType::CAPTURE(_) => ABResult::exact_from_cents(static_eval_centis+thread.pos().see(*m)+delta) > alpha
                                                                                        && thread.pos().see(*m) > 0,
                                                        chess::MoveType::PROMOTION(_) |
                                                        chess::MoveType::PROMOTIONCAPTURE((_,_)) => true,
                                                        chess::MoveType::ENPASSANT => ABResult::exact_from_cents(static_eval_centis+delta+100) > alpha,
                                                        _ => false}).collect();
        if cand_moves.len() == 0 {
            return static_eval;
        }
        cand_moves.sort_by_key(|m| match m.typ {
                                    chess::MoveType::CAPTURE(_) => thread.pos().see(*m),
                                    chess::MoveType::PROMOTION(p) => p.value()-chess::Piece::PAWN.value(),
                                    chess::MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => p_prom.value()+p_cap.value()-chess::Piece::PAWN.value(),
                                    _ => 0,
                                });
    }
    let castling = thread.pos().get_castling_rights();
    for m in cand_moves {
        //stop if we receive the flag is set;
        if thread.stop_flag().read().unwrap().eq(&true) {
            return alpha;
        }

        thread.do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(thread, -beta, -alpha, delta-20, qply+1, Some(m));
        thread.undo_move(m, castling, lm);

        if score >= beta {
            return score.to_lowerbound();
        } else if alpha < score {
            alpha = score;
        }
    }
    alpha
}

//Tests
#[test]
fn ab_comp_test() {
    assert!(ABResult::MIN < ABResult::MAX);
    assert!(ABResult::MIN < ABResult::MATE_NOW);
    assert!(ABResult::MATE_NOW < ABResult::MAX);
    assert!(ABResult::MATE_NOW < ABResult::exact_from_cents(-100));
    assert!(ABResult::exact_from_cents(-100) < ABResult::exact_from_cents(100));
    assert!(ABResult::exact_from_cents(100) < -ABResult::MATE_NOW);
    assert!(ABResult::MATE_NOW < -ABResult::MATE_NOW);
    assert!(-ABResult::MATE_NOW > ABResult::MATE_NOW);
    assert!(-ABResult::MATE_NOW == ABResult{typ: ABResultType::EXACT, value: ABResultValueType::MATE(1)});
    assert!(-(-(-ABResult::MATE_NOW)) == ABResult{typ: ABResultType::EXACT, value: ABResultValueType::MATE(3)});
}

#[test]
fn write_and_read_tt() {
    let mut hash = ABResultHash::new(10000);
    let entry = ABResultHashEntry::new(-ABResult::MATE_NOW, 3, 1234628935786765, chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE});
    hash.set(1234628935786765, entry.clone());
    assert!(hash.get(1234628935786765).unwrap() == entry);
    let entry2 = ABResultHashEntry::new(-ABResult::MATE_NOW, 3, 1234628935786798, chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE});
    let entry4 = ABResultHashEntry::new(ABResult::DRAW, 2, 1234628935786798, chess::Move{from: 4, to: 8, piece: chess::Piece::QUEEN, typ: chess::MoveType::MOVE});
    hash.set(1234628935786798, entry2.clone());
    hash.set(1234628935786798, entry4.clone());
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = ABResultHashEntry::new(-ABResult::MATE_NOW, 3, 1234628935786700, chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE});
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set(1234628935786700, entry3.clone()));
    std::thread::sleep(Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
