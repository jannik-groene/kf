use super::chess;
use super::evaluate;
use std::time::{Instant, Duration};
use std::sync::{Arc, RwLock};
use std::cmp::{PartialOrd, Ord};
use std::ops::Neg;
use std::fmt::Display;

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
    hash: Arc<RwLock<Vec<ABResultHashEntry>>>,
    size: usize,
}

impl ABResultHash {
    fn new(size: usize) -> Self {
        let mut hash_vec = Vec::with_capacity(size);
        for _ in 0..size {
            hash_vec.push(ABResultHashEntry::UNCHECKED);
        }
        hash_vec.shrink_to_fit();
        ABResultHash {
            size,
            hash: Arc::new(RwLock::new(hash_vec)),
        }
    }
    #[inline(always)]
    fn get(&self, zobrist_key: u64) -> ABResultHashEntry {
        self.hash.read().unwrap()[zobrist_key as usize % self.size]
    }
    #[inline(always)]
    fn set(&mut self, zobrist_key: u64, entry: ABResultHashEntry) {
        self.hash.write().unwrap()[zobrist_key as usize % self.size] = entry;
    }
    #[inline(always)]
    fn set_if_deeper(&mut self, zobrist_key: u64, entry: ABResultHashEntry) {
        let mut hash_entry = self.hash.write().unwrap();
        if hash_entry[zobrist_key as usize % self.size].depth < entry.depth {
            *hash_entry.get_mut(zobrist_key as usize % self.size).unwrap() = entry;
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
            _ => ABResult{typ: ABResultType::EXACT, value: ABResultValueType::NEGINFTY}
        }
    }
    fn aspiration_higher(&self, count: usize) -> ABResult {
        if count >= 5 {
            return ABResult{typ: ABResultType::EXACT, value: ABResultValueType::INFTY};
        }
        match self.value {
            ABResultValueType::CENTIS(c) => ABResult { typ: self.typ, value: ABResultValueType::CENTIS(c+ABResult::ASPIRATION_ADJUSTMENTS[count]) },
            _ => ABResult{typ: ABResultType::EXACT, value: ABResultValueType::INFTY}
        }

    }
    fn zero_window(&self) -> ABResult {
        match self.value {
            ABResultValueType::CENTIS(c) => ABResult { value: ABResultValueType::CENTIS(c+1), typ: self.typ },
            ABResultValueType::MATE(n) => ABResult { value: ABResultValueType::MATE(n-2), typ: self.typ },
            ABResultValueType::INFTY => *self,
            ABResultValueType::NEGINFTY => *self,
        }
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
            typ: self.typ,
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
}

impl SearchInfo {
    fn new() -> SearchInfo {
        SearchInfo {
            bestmove: None,
            eval: ABResult::MIN,
            depth: 0,
            pv: Vec::new(),
            nodes: 0,
        }
    }
}

pub struct ABSearchManager {
    pos: chess::Position,
    threads: usize,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
}

impl ABSearchManager {
    pub fn new() -> ABSearchManager {
        ABSearchManager {
            pos: chess::Position::new(),
            threads: 1,
            move_hash: ABResultHash::new(2),
            stop_flag: Arc::new(RwLock::new(false)),
            search_info: SearchInfo::new(),
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        self.move_hash = ABResultHash::new(size*1_000_000/std::mem::size_of::<ABResultHashEntry>());
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }
    pub fn set_position(&mut self, pos: chess::Position) {
        self.pos = pos;
    }
    pub fn search(&mut self, out_channel: Sender<EngineIO>, target_depth: Option<u8>) -> std::thread::JoinHandle<()> {
        let depth = target_depth.unwrap_or(u8::MAX);
        self.reset_search_info();
        *self.stop_flag.write().unwrap() = false;
        let mut root_search_info = ABSearchMainThread {
            pos: self.pos.clone(),
            nodes: 0,
            move_hash: self.move_hash.clone(),
            threads: self.threads,
            stop_flag: self.stop_flag.clone(),
            pv: Vec::with_capacity(depth as usize * (depth as usize + 1) / 2),
            pv_depth: 0,
            prev_pv: None,
            search_info: self.search_info.clone(),
            sender: out_channel,
        };
        std::thread::spawn(move || search(&mut root_search_info, depth, ABResult::MIN, ABResult::MAX))
    }
    pub fn reset_search_info(&mut self) {
        self.search_info = SearchInfo::new();
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
    fn write_pv(&mut self, _: u8, _: Option<chess::Move>, _: bool) {}
    fn save_pv(&mut self) {}
    fn previous_pv_move(&self, _: u8) -> Option<chess::Move> {None}
}

struct ABSearchMainThread {
    pos: chess::Position,
    nodes: u64,
    threads: usize,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    pv: Vec<Option<chess::Move>>,
    prev_pv: Option<Vec<chess::Move>>,
    pv_depth: usize,
    search_info: SearchInfo,
    sender: Sender<EngineIO>,
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
    // We index by how many moves are remaining
    fn write_pv(&mut self, rem_depth: u8, m: Option<chess::Move>, from_hash_hit: bool) {
        if rem_depth == 0 {return;}
        let index = (rem_depth as usize)*(rem_depth as usize - 1) / 2;
        //Extend the pv. Should be cheap since we already have the capacity
        if self.pv.len() < index + rem_depth as usize {
            self.pv.extend(std::iter::repeat(None).take(index + rem_depth as usize - self.pv.len()));
            self.pv_depth = rem_depth as usize;
        }
        self.pv[index] = m;
        for i in index+1-rem_depth as usize..index {
            if !from_hash_hit {
                self.pv[i+rem_depth as usize] = self.pv[i];
            } else {
                self.pv[i+rem_depth as usize] = None;
            }
        }
    }
    fn save_pv(&mut self) {
        let mut ppv = Vec::new();
        let mut index = self.pv_depth * (self.pv_depth - 1) / 2;
        while self.pv.len() > index && self.pv[index].is_some() {
            ppv.push(self.pv[index].unwrap());
            index += 1;
        }
        if ppv.len() > 0 {
            self.prev_pv = Some(ppv);
        }
        self.pv = Vec::new()
    }
    fn previous_pv_move(&self, depth: u8) -> Option<chess::Move> {
        if self.prev_pv.is_none() {
            return None;
        }
        let m = self.prev_pv.as_ref().unwrap().get(depth as usize);
        match m {
            Some(mm) => Some(*mm),
            None => None,
        }
    }
}

impl ABSearchMainThread {
    fn bestmove(&self) -> Option<chess::Move> {
        if self.pv_depth == 0 {None}
        else if self.prev_pv.is_some() {
            let m = self.prev_pv.as_ref().unwrap().get(0);
            match m {
                Some(mm) => Some(*mm),
                None => None,
            }
        } else {
            None
        }
    }
    fn print_pv(&self) {
        print!("info depth {} pv", self.pv_depth);
        let mut index = self.pv_depth*(self.pv_depth-1)/2;
        while index < self.pv.len() && self.pv[index].is_some() {
            print!(" {}", self.pv[index].unwrap());
            index += 1;
        }
        print!("\n");
    }
}

struct ABSearchHelperThread {
    pos: chess::Position,
    nodes: u64,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
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
}

impl ABSearchHelperThread {
    fn search(&mut self, depth: u8, alpha: ABResult, beta: ABResult, lm: Option<chess::Move>) -> u64 {
        search_step(self, depth, 0, 0, alpha, beta, lm);
        self.nodes
    }
}

fn search(thread: &mut ABSearchMainThread, depth: u8, mut alpha: ABResult, mut beta: ABResult) {
    let now = Instant::now();
    let mut helper_handles = Vec::new();
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
                        stop_flag: thread.stop_flag().clone(),
                    };
                    helper_handles.push(std::thread::spawn(move || helper_thread.search(d + i as u8 / 2, alpha, beta, None)));
                }
            }
            let eval = search_step(thread, d, 0, 0, alpha, beta, thread.pos().get_last_move());
            //We reached the target depth and stopped, so we update the external values
            if thread.stop_flag().read().unwrap().eq(&true) {
                drop(thread.sender.send(EngineIO::SEARCHENDED));
                return;
            }
            if d > 2 {
                //Set stop flag and join all helpers
                *thread.stop_flag().write().unwrap() = true;
                for helper in helper_handles {
                    match helper.join() {
                        //Concerning but not fatal?
                        Err(_) => {},
                        Ok(n) => *thread.nodes_mut() += n,
                    }
                }
                helper_handles = Vec::new();
                //Reset stop flag
                *thread.stop_flag().write().unwrap() = false;
            }
            println!("info nodes {} nps {}", thread.nodes(), 1000* *thread.nodes() as u128 /now.elapsed().as_millis().clamp(1,u128::MAX));
            match eval.typ {
                ABResultType::EXACT => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    thread.print_pv();
                    thread.save_pv();
                    thread.search_info.eval = eval;
                    thread.search_info.bestmove = thread.bestmove();
                    match thread.sender.send(EngineIO::SEARCHUPDATE(thread.search_info.clone())) {
                        Ok(_) => {},
                        //We end the search if we cannot communicate the results anymore
                        Err(_) => break,
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
    drop(thread.sender.send(EngineIO::SEARCHENDED));
}

fn search_step(thread: &mut impl ABSearchThread, depth: u8, ply: u8, depth_reduction: u8,
               mut alpha: ABResult, beta: ABResult, lm: Option<chess::Move>) -> ABResult {

    *thread.nodes_mut() += 1;

    //Check if the move is already hashed
    let hash_entry = thread.move_hash().get(thread.pos().zobrist_hash());

    if hash_entry.zobrist_hash == thread.pos().zobrist_hash() && hash_entry.mov().is_some() {
        if hash_entry.depth >= depth-ply {
            match hash_entry.res.typ {
                ABResultType::EXACT => {
                    if !thread.is_helper() {
                        thread.write_pv(depth-ply,hash_entry.mov(),true);
                    }
                    return hash_entry.res;
                },
                ABResultType::LOWERBOUND => {
                    if hash_entry.res >= beta {
                        return hash_entry.res;
                    }
                },
                ABResultType::UPPERBOUND => {
                    if hash_entry.res < alpha {
                        return hash_entry.res;
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

    //Try a null move to find a beta cutoff; search the first three plys fully.
    if !thread.pos_mut().in_check() && moves.len() > 0 && ply > 3 && ply < depth {
        thread.pos_mut().do_null_move();
        if -search_step(thread, depth, ply+1, depth_reduction, -beta, -alpha, None) >= beta {
            thread.pos_mut().undo_null_move(lm);
            return beta;
        }
        thread.pos_mut().undo_null_move(lm);
    }

    if ply + depth_reduction >= depth {
        return quiesce(thread, alpha, beta, 200, 0, lm);
    }
    //Futility pruning
    else if ply + depth_reduction == depth - 1 && depth > 3 {
        let eval = evaluate::evaluate(thread.pos_mut());
        if ABResult::exact_from_cents(eval + 300) < alpha {
            return quiesce(thread, alpha, beta, 100, 0, lm);
        }
    }
    //Extended futility pruning
    else if depth > 3 && ply + depth_reduction == depth - 2 {
        let eval = evaluate::evaluate(thread.pos_mut());
        if ABResult::exact_from_cents(eval + 500) < alpha {
            return quiesce(thread, alpha, beta, 100, 0, lm);
        }
    }


    //Store castling rights for move undoing
    let castling = thread.pos().get_castling_rights();

    //if !thread.is_helper() {
        evaluate::order_moves(&mut moves, thread.pos(), thread.previous_pv_move(ply), thread.move_hash().get(thread.pos().zobrist_hash()).mov());
    //} else {
    //    evaluate::order_moves_with_random_bias(&mut moves, thread.pos(), thread.move_hash().get(thread.pos().zobrist_hash()).mov());
    //}

    //Set up paramaters
    let mut score = ABResult::MIN;
    let mut fail_low = true;
    let mut bestmove = None;
    let mut zws = false;

    for i in 0..moves.len() {
        thread.pos_mut().do_move(moves[i]);
        let mut movescore = if thread.pos().pos_in_history() {
                            //If we repeat twice, it's gonna happen thrice
                                ABResult::DRAW
                            //Apply LMR
                            } else if ply > 3 && !thread.pos_mut().in_check() {
                                -search_step(thread, depth, ply+1, (i as u8)/6, -beta, -alpha, Some(moves[i]))
                            } else if ply > 3 && !thread.pos_mut().in_check() && zws {
                                -search_step(thread, depth, ply+1, (i as u8)/6, -alpha.zero_window(), -alpha, Some(moves[i]))
                            } else if zws {
                                -search_step(thread, depth, ply+1, 0, -alpha.zero_window(), -alpha, Some(moves[i]))
                            } else {
                                -search_step(thread, depth, ply+1, 0, -beta, -alpha, Some(moves[i]))
                            };
        if zws && movescore > alpha && movescore < beta {
            movescore = if ply > 3 && !thread.pos_mut().in_check() {
                                -search_step(thread, depth, ply+1, (i as u8)/6, -beta, -alpha, Some(moves[i]))
                            } else {
                                -search_step(thread, depth, ply+1, 0, -beta, -alpha, Some(moves[i]))
                            };
        }
        thread.pos_mut().undo_move(moves[i], castling, lm);
        //Abort search if the helper gets a stop signal
        if thread.stop_flag().read().unwrap().eq(&true) {return ABResult::MIN;};
        if movescore >= beta {
            let zh = thread.pos().zobrist_hash();
            thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry::new(movescore.to_lowerbound(), depth-ply, zh, moves[i]));
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

    let zh = thread.pos().zobrist_hash();
    if fail_low {
        thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry::new(score.to_upperbound(), depth-ply, zh, bestmove.unwrap()));
        score.to_upperbound()
    } else {
        if !thread.is_helper() {
            thread.write_pv(depth-ply,bestmove,false);
        }
        thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry::new(score.to_exact(), depth-ply, zh, bestmove.unwrap()));
        score.to_exact()
    }
}

fn quiesce(thread: &mut impl ABSearchThread, mut alpha: ABResult, beta: ABResult, delta: i32, qply: u8, lm: Option<chess::Move>) -> ABResult {
    *thread.nodes_mut() += 1;

    let mut cand_moves = thread.pos_mut().get_moves();
    if cand_moves.len() == 0 && thread.pos_mut().in_check() {
        return ABResult::MATE_NOW;
    } else if cand_moves.len() == 0 {
        return ABResult::STALEMATE;
    }
    //We want to explore checks in the near future fully
    if !thread.pos_mut().in_check() || qply > 5 {
        let static_eval_centis = evaluate::evaluate(thread.pos_mut());
        let static_eval = ABResult::exact_from_cents(static_eval_centis);
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

        thread.pos_mut().do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(thread, -beta, -alpha, delta-20, qply+1, Some(m));
        thread.pos_mut().undo_move(m, castling, lm);

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
    assert!(hash.get(1234628935786765) == entry);
    let entry2 = ABResultHashEntry::new(-ABResult::MATE_NOW, 3, 1234628935786798, chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE});
    let entry4 = ABResultHashEntry::new(ABResult::DRAW, 2, 1234628935786798, chess::Move{from: 4, to: 8, piece: chess::Piece::QUEEN, typ: chess::MoveType::MOVE});
    hash.set_if_deeper(1234628935786798, entry2.clone());
    hash.set_if_deeper(1234628935786798, entry4.clone());
    assert!(hash.get(1234628935786798) == entry2);
    let entry3 = ABResultHashEntry::new(-ABResult::MATE_NOW, 3, 1234628935786700, chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE});
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set_if_deeper(1234628935786700, entry3.clone()));
    std::thread::sleep(Duration::from_millis(100));
    assert!(hash.get(1234628935786700) == entry3);
}
