use super::chess;
use super::evaluate;
use std::time::{Instant, Duration};
use std::sync::{Arc, RwLock};
use std::cmp::{PartialOrd, Ord};
use std::ops::Neg;
use std::fmt::Display;
use rand::{seq::SliceRandom, thread_rng};

fn print_pv(mut pos: chess::Position, hash: &ABResultHash, depth: usize) {
    let mut pv_entry = hash.get(pos.zobrist_hash());
    print!("info depth {} pv", depth);
    let mut move_count = 1;
    while pv_entry.mov.is_some() && move_count <= depth {
        print!(" {}", pv_entry.mov.unwrap());
        pos.do_move(pv_entry.mov.unwrap());
        pv_entry = hash.get(pos.zobrist_hash());
        move_count += 1;
    }
    print!("\n");
}

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
    hash: Arc<Vec<RwLock<ABResultHashEntry>>>,
    size: usize,
}

impl ABResultHash {
    fn new(size: usize) -> Self {
        ABResultHash {
            size,
            hash: Arc::new(std::iter::repeat_with(|| RwLock::new(ABResultHashEntry::UNCHECKED)).take(size).collect()),
        }
    }
    #[inline(always)]
    fn get(&self, zobrist_key: u64) -> ABResultHashEntry {
        *self.hash[zobrist_key as usize % self.size].read().unwrap()
    }
    #[inline(always)]
    fn set(&mut self, zobrist_key: u64, entry: ABResultHashEntry) {
        *self.hash[zobrist_key as usize % self.size].write().unwrap() = entry;
    }
    #[inline(always)]
    fn set_if_deeper(&mut self, zobrist_key: u64, entry: ABResultHashEntry) {
        let mut hash_entry = self.hash[zobrist_key as usize % self.size].write().unwrap();
        if hash_entry.depth < entry.depth {
            *hash_entry = entry;
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
    mov: Option<chess::Move>,
}

impl ABResultHashEntry {
    const UNCHECKED: ABResultHashEntry = ABResultHashEntry {res: ABResult::MIN, depth: 0, zobrist_hash: 0, mov: None};
}

pub struct ABSearch {
    pos: chess::Position,
    depth: u8,
    eval: ABResult,
    threads: usize,
    move_hash: ABResultHash,
}

impl ABSearch {
    pub fn new(pos: chess::Position, mut threads: usize) -> ABSearch {
        assert!(threads > 0);
        ABSearch {
            pos,
            depth: 1,
            eval: ABResult::MIN,
            threads,
            move_hash: ABResultHash::new(12800000),
        }
    }
    pub fn set_position(&mut self, pos: chess::Position) {
        self.pos = pos;
    }
    pub fn set_depth(&mut self, depth: u8) {
        self.depth = depth
    }
    pub fn search(&mut self, depth: u8) -> (ABResult,Option<chess::Move>) {
        self.depth = depth;
        let mut root_search_info = ABSearchMainThread {
            pos: self.pos.clone(),
            nodes: 0,
            move_hash: self.move_hash.clone(),
            threads: self.threads,
            stop_flag: Arc::new(RwLock::new(false)),
            pv: Vec::with_capacity(depth as usize * (depth as usize + 1) / 2),
            pv_depth: 0,
        };
        self.eval = search(&mut root_search_info, self.depth, ABResult::MIN, ABResult::MAX);
        (self.eval, root_search_info.bestmove())
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
}

struct ABSearchMainThread {
    pos: chess::Position,
    nodes: u64,
    threads: usize,
    move_hash: ABResultHash,
    stop_flag: Arc<RwLock<bool>>,
    pv: Vec<Option<chess::Move>>,
    pv_depth: usize,
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
}

impl ABSearchMainThread {
    fn bestmove(&self) -> Option<chess::Move> {
        if self.pv_depth == 0 {None}
        else {
            let m = self.pv.get(self.pv_depth * (self.pv_depth - 1) / 2);
            match m {
                Some(mm) => *mm,
                None => None,
            }
        }
    }
    fn print_pv(&self) {
        print!("info depth {} pv ", self.pv_depth);
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

fn search(thread: &mut ABSearchMainThread, depth: u8, mut alpha: ABResult, mut beta: ABResult) -> ABResult {
    let mut eval = ABResult::MIN;
    let now = Instant::now();
    let mut helper_handles = Vec::new();
    for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        loop {
            //Spawn helper threads for search
            if thread.threads() > 1 && d > 1 {
                for i in 1..thread.threads() {
                    let mut helper_thread = ABSearchHelperThread {
                        pos: thread.pos().clone(),
                        nodes: 0,
                        move_hash: thread.move_hash().clone(),
                        stop_flag: thread.stop_flag().clone(),
                    };
                    helper_handles.push(std::thread::spawn(move || search_step(&mut helper_thread, d + i as u8 / 2, 0, alpha, beta, None)));
                }
            }
            eval = search_step(thread, d, 0, alpha, beta, thread.pos().get_last_move());
            println!("info nodes {} nps {}", thread.nodes(), 1000* *thread.nodes() as u128 /now.elapsed().as_millis().clamp(1,u128::MAX));
            if d > 1 {
                //Set stop flag and join all helpers
                *thread.stop_flag().write().unwrap() = true;
                for helper in helper_handles {
                    helper.join();
                }
                helper_handles = Vec::new();
                //Reset stop flag
                *thread.stop_flag().write().unwrap() = false;
            }
            match eval.typ {
                ABResultType::EXACT => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    thread.print_pv();
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
        match eval.value {
            ABResultValueType::MATE(_) => break,
            _ => {}
        }
    }
    eval
}

fn search_step(thread: &mut impl ABSearchThread, depth: u8, ply: u8,
               mut alpha: ABResult, beta: ABResult, lm: Option<chess::Move>) -> ABResult {

    *thread.nodes_mut() += 1;

    //Check if this is a terminal position
    let mut moves = thread.pos_mut().get_moves();

    if moves.len() == 0 && thread.pos_mut().in_check() {
        return ABResult::MATE_NOW;
    } else if moves.len() == 0 {
        return ABResult::STALEMATE;
    }

    if ply == depth {
        return ABResult::exact_from_cents(evaluate::evaluate(thread.pos_mut()));
    }

    //Check if the move is already hashed
    let hash_entry = thread.move_hash().get(thread.pos().zobrist_hash());

    if hash_entry.zobrist_hash == thread.pos().zobrist_hash() {
        if hash_entry.depth >= depth-ply {
            match hash_entry.res.typ {
                ABResultType::EXACT => {
                    if !thread.is_helper() {
                        thread.write_pv(depth-ply,hash_entry.mov,true);
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

    //Store castling rights for move undoing
    let castling = thread.pos().get_castling_rights();

    //TODO: Proper move ordering!!
    if !thread.is_helper() {
        moves.sort_by_key(|m| {
            thread.pos_mut().do_move(*m);
            let res = thread.move_hash().get(thread.pos().zobrist_hash()).res;
            thread.pos_mut().undo_move(*m, castling, lm);
            if res.typ == ABResultType::EXACT {
                -res
            } else {
                ABResult::DRAW
            }
        });
    } else {
        moves.shuffle(&mut thread_rng());
    }

    //Set up paramaters
    let mut score = ABResult::MIN;
    let mut fail_low = true;
    let mut bestmove = None;

    for m in moves {
        thread.pos_mut().do_move(m);
        let movescore = -search_step(thread, depth, ply+1, -beta, -alpha, Some(m));
        thread.pos_mut().undo_move(m, castling, lm);
        //Abort search if the helper gets a stop signal
        if thread.is_helper() {
            if thread.stop_flag().read().unwrap().eq(&true) {return ABResult::MIN;};
        }
        if movescore >= beta {
            let zh = thread.pos().zobrist_hash();
            thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry {res: movescore.to_lowerbound(), depth: depth-ply, zobrist_hash: zh, mov: Some(m)});
            return movescore.to_lowerbound();
        }
        if movescore > score {
            bestmove = Some(m);
            score = movescore;
            if score > alpha {
                fail_low = false;
                alpha = score;
            }
        }
    }
    let zh = thread.pos().zobrist_hash();
    if fail_low {
        thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry {res: score.to_upperbound(), depth: depth-ply, zobrist_hash: zh, mov: bestmove});
        score.to_upperbound()
    } else {
        if !thread.is_helper() {
            thread.write_pv(depth-ply,bestmove,false);
        }
        thread.move_hash_mut().set_if_deeper(zh, ABResultHashEntry {res: score.to_exact(), depth: depth-ply, zobrist_hash: zh, mov: bestmove});
        score.to_exact()
    }
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
    let entry = ABResultHashEntry { res: -ABResult::MATE_NOW, zobrist_hash: 1234628935786765, depth: 3, mov: Some(chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE})};
    hash.set(1234628935786765, entry.clone());
    assert!(hash.get(1234628935786765) == entry);
    let entry2 = ABResultHashEntry { res: -ABResult::MATE_NOW, zobrist_hash: 1234628935786798, depth: 3, mov: Some(chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE})};
    let entry4 = ABResultHashEntry { res: -ABResult::MATE_NOW, zobrist_hash: 1234628935786798, depth: 2, mov: Some(chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE})};
    hash.set_if_deeper(1234628935786798, entry2.clone());
    assert!(hash.get(1234628935786798) == entry2);
    let entry3 = ABResultHashEntry { res: -ABResult::MATE_NOW, zobrist_hash: 1234628935786700, depth: 3, mov: Some(chess::Move{from: 1, to: 2, piece: chess::Piece::KING, typ: chess::MoveType::MOVE})};
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set_if_deeper(1234628935786700, entry3.clone()));
    std::thread::sleep(Duration::from_millis(100));
    assert!(hash.get(1234628935786700) == entry3);
}
