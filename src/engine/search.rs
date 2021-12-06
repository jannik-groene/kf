use std::time::Instant;
use std::sync::{Arc, RwLock};
use std::io::Write;

use std::sync::mpsc::Sender;

mod tt;

use tt::{TranspositionTable, TTEntry};
use super::EngineIO;
use super::chess::{Position, Piece, Move, Color, MoveType};
use super::evaluate::{evaluate, order_moves, has_major_pieces, has_minor_pieces, is_material_draw};
use super::evaluate::eval::{Eval, Value, Bound};

#[derive(Clone)]
pub struct SearchInfo {
    pub bestmove: Option<Move>,
    pub eval: Eval,
    pub depth: u8,
    pub pv: Vec<Move>,
    pub nodes: u64,
    pub id: u64,
}

impl SearchInfo {
    fn new(id: u64) -> SearchInfo {
        SearchInfo {
            bestmove: None,
            eval: Eval::MIN,
            depth: 0,
            pv: Vec::new(),
            nodes: 0,
            id,
        }
    }
}

pub struct ABSearchManager {
    pos: Position,
    threads: usize,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
}

impl ABSearchManager {
    pub fn new() -> ABSearchManager {
        ABSearchManager {
            pos: Position::new(),
            threads: 1,
            tt: TranspositionTable::new(2),
            stop_flag: Arc::new(RwLock::new(false)),
            search_info: SearchInfo::new(0),
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        self.tt = TranspositionTable::new(size*1_000_000/std::mem::size_of::<(TTEntry, TTEntry)>());
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }
    pub fn set_position(&mut self, pos: Position) {
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
            tt: self.tt.clone(),
            threads: self.threads,
            stop_flag: self.stop_flag.clone(),
            search_info: self.search_info.clone(),
            sender: out_channel,
            bestmove: None,
            killers: Vec::new(),
            //nnue: self.nnue.clone(),
        };
        std::thread::spawn(move || search(&mut root_search_info, depth, Eval::MIN, Eval::MAX))
    }
    pub fn reset_search_info(&mut self, id: u64) {
        self.search_info = SearchInfo::new(id);
    }
    pub fn stop(&mut self) {
        *self.stop_flag.write().unwrap() = true;
    }
    pub fn color(&self) -> Color {
        self.pos.color()
    }
    pub fn reset_hash(&mut self) {
        self.tt.reset();
    }
}

trait ABSearchThread {
    fn pos(&self) -> &Position;
    fn pos_mut(&mut self) -> &mut Position;

    fn nodes(&self) -> &u64;
    fn nodes_mut(&mut self) -> &mut u64;

    fn tt_mut(&mut self) -> &mut TranspositionTable;
    fn tt(&self) -> &TranspositionTable;

    fn is_helper(&self) -> bool;

    fn stop_flag(&self) -> &Arc<RwLock<bool>>;

    //These are optional and not used for helper threads
    fn threads(&self) -> usize {1}
    fn set_bestmove(&mut self, _m: Option<Move>) {}

    fn do_move(&mut self, m: Move);
    fn undo_move(&mut self);

    fn evaluate(&mut self) -> Eval;

    //save and retrieve killer moves
    fn register_killer(&mut self, ply: u8, m: Move);
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2];
    fn invalidate_killers(&mut self, ply: u8);
}

struct ABSearchMainThread {
    pos: Position,
    nodes: u64,
    threads: usize,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
    sender: Sender<EngineIO>,
    bestmove: Option<Move>,
    killers: Vec<([Option<Move>; 2], [u8; 2])>,
}

impl ABSearchThread for ABSearchMainThread {
    fn pos_mut(&mut self) -> &mut Position {&mut self.pos}
    fn pos(&self) -> &Position {&self.pos}
    fn nodes(&self) -> &u64 {&self.nodes}
    fn nodes_mut(&mut self) -> &mut u64 {&mut self.nodes}
    fn tt(&self) -> &TranspositionTable {&self.tt}
    fn tt_mut(&mut self) -> &mut TranspositionTable {&mut self.tt}
    fn is_helper(&self) -> bool {false}
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {&self.stop_flag}
    fn threads(&self) -> usize {self.threads}
    fn set_bestmove(&mut self, m: Option<Move>) {
        self.bestmove = m;
    }
    fn do_move(&mut self, m: Move) {
        self.pos.do_move(m);
    }
    fn undo_move(&mut self) {
        self.pos.undo_move();
    }
    fn evaluate(&mut self) -> Eval {
        evaluate(self.pos_mut())
    }

    fn register_killer(&mut self, ply: u8, m: Move) {
        if self.killers.len() <= ply as usize {
            self.killers.resize(ply as usize+1, ([None,None],[0,0]));
        }
        if self.killers[ply as usize].0[0] == Some(m) {
            self.killers[ply as usize].1[0] += 1;
        } else if self.killers[ply as usize].0[1] == Some(m) {
            self.killers[ply as usize].1[1] += 1;
        } else if self.killers[ply as usize].1[0] > self.killers[ply as usize].1[1] {
            self.killers[ply as usize].0[1] = Some(m);
            self.killers[ply as usize].1[1] = 1;
        } else {
            self.killers[ply as usize].0[1] = Some(m);
            self.killers[ply as usize].1[1] = 1;
        }
    }
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2] {
        if self.killers.len() <= ply as usize {
            &[None, None]
        } else {
            &self.killers[ply as usize].0
        }
    }
    fn invalidate_killers(&mut self, ply: u8) {
        if self.killers.len() > ply as usize + 1 {
            self.killers[ply as usize + 1].1 = [0,0];
        }
    }
}

impl ABSearchMainThread {
    fn bestmove(&self) -> Option<Move> {
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
            let hashentry = self.tt.get(pos.zobrist_hash());
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
    pos: Position,
    nodes: u64,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    killers: Vec<([Option<Move>; 2], [u8; 2])>,
}

impl ABSearchThread for ABSearchHelperThread {
    fn pos_mut(&mut self) -> &mut Position {&mut self.pos}
    fn pos(&self) -> &Position {&self.pos}
    fn nodes(&self) -> &u64 {&self.nodes}
    fn nodes_mut(&mut self) -> &mut u64 {&mut self.nodes}
    fn tt(&self) -> &TranspositionTable {&self.tt}
    fn tt_mut(&mut self) -> &mut TranspositionTable {&mut self.tt}
    fn is_helper(&self) -> bool {true}
    fn stop_flag(&self) -> &Arc<RwLock<bool>> {&self.stop_flag}
    fn do_move(&mut self, m: Move) {
        self.pos.do_move(m);
    }
    fn undo_move(&mut self) {
        self.pos.undo_move();
    }
    fn evaluate(&mut self) -> Eval {
        evaluate(self.pos_mut())
    }
    fn register_killer(&mut self, ply: u8, m: Move) {
        if self.killers.len() <= ply as usize {
            self.killers.resize(ply as usize+1, ([None,None],[0,0]));
        }
        if self.killers[ply as usize].0[0] == Some(m) {
            self.killers[ply as usize].1[0] += 1;
        } else if self.killers[ply as usize].0[1] == Some(m) {
            self.killers[ply as usize].1[1] += 1;
        } else if self.killers[ply as usize].1[0] > self.killers[ply as usize].1[1] {
            self.killers[ply as usize].0[1] = Some(m);
            self.killers[ply as usize].1[1] = 1;
        } else {
            self.killers[ply as usize].0[1] = Some(m);
            self.killers[ply as usize].1[1] = 1;
        }
    }
    fn get_killers(&self, ply: u8) -> &[Option<Move>; 2] {
        if self.killers.len() <= ply as usize {
            &[None, None]
        } else {
            &self.killers[ply as usize].0
        }
    }
    fn invalidate_killers(&mut self, ply: u8) {
        if self.killers.len() > ply as usize + 1 {
            self.killers[ply as usize + 1].1 = [0,0];
        }
    }
}

impl ABSearchHelperThread {
    fn search(&mut self, depth: u8, alpha: Eval, beta: Eval) -> u64 {
        search_step(self, depth, 0, 0, 0, 0, false, alpha, beta);
        self.nodes
    }
}

fn search(thread: &mut ABSearchMainThread, depth: u8, mut alpha: Eval, mut beta: Eval) {
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
                        tt: thread.tt().clone(),
                        stop_flag: helper_stop_flag.clone(),
                        killers: Vec::new(),
                        //nnue: thread.nnue().clone(),
                    };
                    helper_handles.push(std::thread::spawn(move || helper_thread.search(d.saturating_add(i as u8 / 2), alpha, beta)));
                }
            }
            let eval = search_step(thread, d, 0, 0, 0, 0, false, alpha, beta);
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
            match eval.bound() {
                Bound::EXACT => {
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
                Bound::LOWERBOUND => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    fail_highs += 1;
                    beta = eval.aspiration_higher(fail_highs);
                }
                Bound::UPPERBOUND => {
                    println!("info {} depth {} time {}", eval, d, now.elapsed().as_millis());
                    fail_lows += 1;
                    alpha = eval.aspiration_lower(fail_lows);
                }
            }
        }
    }
    drop(thread.sender.send(EngineIO::SEARCHENDED(thread.search_info.id)));
}

fn is_tactical(pos: &Position, m: Move) -> bool {
    if pos.gives_check(&m) {
        return true;
    }
    match m.typ {
        MoveType::CAPTURE(_)
            | MoveType::PROMOTION(_)
            | MoveType::PROMOTIONCAPTURE(_) => true,
        _ => false,
    }
}

//Parameters:
// thread: The search thread head.
// depth: the depth to search to
// ply; the current depth
// depth_reduction: how far to reduce the search depth
// extension: how many plys to extend the search due to only moves in the tree
// null_moves: how many null moves have been performed in the current search
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
fn search_step(thread: &mut impl ABSearchThread,
               depth: u8,
               ply: u8,
               depth_reduction: u8,
               mut extension: u8,
               null_moves: u8,
               zw: bool,
               mut alpha: Eval,
               beta: Eval) -> Eval {

    *thread.nodes_mut() += 1;

    //Check if the move is already hashed
    let hash_entry = thread.tt().get(thread.pos().zobrist_hash());

    let mut ttmove = None;

    if hash_entry.is_some() && hash_entry.unwrap().mov().is_some() {
        //see if we have a TT-hit
        if hash_entry.unwrap().depth() >= (depth+extension).saturating_sub(ply) {
            match hash_entry.unwrap().eval().bound() {
                Bound::EXACT => {
                    if ply == 0 {
                        thread.set_bestmove(hash_entry.unwrap().mov());
                    }
                    return hash_entry.unwrap().eval();
                },
                Bound::LOWERBOUND => {
                    if hash_entry.unwrap().eval() >= beta {
                        return hash_entry.unwrap().eval();
                    }
                },
                Bound::UPPERBOUND => {
                    if hash_entry.unwrap().eval() < alpha {
                        return hash_entry.unwrap().eval();
                    }
                }
            }
        }

        ttmove = hash_entry.unwrap().mov();

    }

    //check for obviously drawn positions
    if is_material_draw(thread.pos()) {
        return Eval::DRAW;
    }

    //Check if this is a terminal position
    let mut moves = thread.pos_mut().get_moves();

    if moves.len() == 0 && thread.pos_mut().in_check() {
        return Eval::MATE_NOW;
    } else if moves.len() == 0 {
        return Eval::STALEMATE;
    }

    //If we cannot beat the score, just return immediately
    if alpha.value() == Value::MATE(1) {
        return Eval::mate_in(1).to_upperbound();
    }

    //Repeated positions are probably draws
    if thread.pos().pos_in_history() && ply > 0 {
        return Eval::DRAW;
    }

    //Calculate the depth we are still to search.
    let depth_left: u8 = depth.saturating_add(extension).saturating_sub(depth_reduction).saturating_sub(ply);

    //We extend the normal search if we are  in check, else go into quiescence
    if depth_left == 0 && !thread.pos_mut().in_check() {
        return quiesce(thread, alpha, beta, 200 - 20 * depth_reduction as i32, 0);
    }
    //Futility pruning
    else if depth > 3 && depth_left == 1 {
        let eval = thread.evaluate();
        if eval + 300 < alpha {
            return quiesce(thread, alpha, beta, 100, 0);
        }
    }
    //Extended futility pruning
    else if depth > 3 && depth_left == 2 {
        let eval = thread.evaluate();
        if eval + 500 < alpha {
            return quiesce(thread, alpha, beta, 100, 0);
        }
    }

    //Try a null move to find a beta cutoff; search the first three plys fully.
    //Should maybe avoid in late game?
    if null_moves < std::cmp::max(depth / 6 + 1, 2)
            && (has_minor_pieces(thread.pos()) || has_major_pieces(thread.pos()))
            && !thread.pos_mut().in_check() && moves.len() > 2 && ply > 2
            && !matches!(alpha.value(), Value::MATE(_))
            && !matches!(beta.value(), Value::MATE(_)) {
        thread.pos_mut().do_null_move();
        let null_score = -search_step(thread,
                                      depth,
                                      ply+1,
                                      depth_reduction + 3,
                                      0,
                                      null_moves + 1,
                                      zw,
                                      beta.neg_down(),
                                      alpha.neg_down());
        thread.pos_mut().undo_null_move();
        if null_score >= beta {
            return null_score.to_lowerbound();
        }
    }

    //Move ordering
    order_moves(&mut moves, thread.pos(), ttmove, thread.get_killers(ply));

    //Set up paramaters
    let mut score = Eval::MIN;
    let mut fail_low = true;
    let mut bestmove = None;

    if moves.len() == 1 {
        extension += 1;
    }

    for i in 0..moves.len() {

        //lmr reduction depth
        let mut lmr = ((depth_left as f64).sqrt() * (i as f64).sqrt() / 7.) as u8;

        //We do not reduce to zero moves left
        lmr = lmr.clamp(0, depth_left - 1);

        thread.do_move(moves[i]);

        let mut movescore = if i == 0 && !zw {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             0, //depth reduction is always zero PV nodes
                                             extension,
                                             null_moves,
                                             false,
                                             beta.neg_down(),
                                             alpha.neg_down())
                            //Apply lmr at sufficiently high depths on non-PV nodes
                            } else if zw && depth_left > 2
                                         && !thread.pos_mut().in_check()
                                         && !is_tactical(thread.pos(), moves[i]) {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             depth_reduction+lmr,
                                             extension,
                                             null_moves,
                                             true,
                                             beta.neg_down(),
                                             alpha.neg_down())
                            //search late nodes in PV-nodes as zero windows
                            } else {
                                -search_step(thread,
                                             depth,
                                             ply+1,
                                             depth_reduction+lmr/3,
                                             extension,
                                             null_moves,
                                             true,
                                             alpha.zero_window().neg_down(),
                                             alpha.neg_down())
                            };

        //Research if we failed high in a PV node
        if i != 0 && movescore > alpha && movescore < beta {
            movescore = -search_step(thread,
                                     depth,
                                     ply+1,
                                     0,
                                     extension,
                                     null_moves,
                                     false,
                                     beta.neg_down(),
                                     movescore.neg_down());
        }

        thread.undo_move();

        //Abort search if the helper gets a stop signal
        if thread.stop_flag().read().unwrap().eq(&true) {return Eval::MIN;};

        //Adjust results
        if movescore >= beta {
            let zh = thread.pos().zobrist_hash();
            thread.tt_mut().set(zh, TTEntry::new(movescore.to_lowerbound(), depth_left, zh, moves[i]));
            if !matches!(moves[i].typ, MoveType::CAPTURE(_)) && ttmove != Some(moves[i]) {
                thread.register_killer(ply, moves[i]);
            }
            thread.invalidate_killers(ply);
            return movescore.to_lowerbound();
        }

        if movescore > score {
            bestmove = Some(moves[i]);
            score = movescore;
            if score > alpha {
                fail_low = false;
                alpha = score;
                //break search if result is already optimal
                if alpha == Eval::mate_in(1) {
                    break;
                }
            }
        }
    }

    assert!(bestmove.is_some());
    assert!(Eval::MIN < score && score < Eval::MAX);

    if ply == 0 {
        thread.set_bestmove(bestmove);
    }

    //reset the killer move counts for ply+1
    thread.invalidate_killers(ply);

    let zh = thread.pos().zobrist_hash();
    if fail_low {
        thread.tt_mut().set(zh, TTEntry::new(score.to_upperbound(), depth_left, zh, bestmove.unwrap()));
        score.to_upperbound()
    } else {
        thread.tt_mut().set(zh, TTEntry::new(score.to_exact(), depth_left, zh, bestmove.unwrap()));
        score.to_exact()
    }
}

fn quiesce(thread: &mut impl ABSearchThread, mut alpha: Eval, beta: Eval, delta: i32, qply: u8) -> Eval {

    if qply > 0 {
        *thread.nodes_mut() += 1;
    }

    //check for obviously drawn positions
    if is_material_draw(thread.pos()) {
        return Eval::DRAW;
    }

    let mut cand_moves = thread.pos_mut().get_moves();

    //check for terminal position
    if cand_moves.len() == 0 && thread.pos_mut().in_check() {
        return Eval::MATE_NOW;
    }

    let static_eval = thread.evaluate();

    //If we are not in check we filter for tactical moves.
    if !thread.pos_mut().in_check() {

        //Adjust based on null-move hypothesis
        if static_eval >= beta {
            return static_eval.to_lowerbound();
        } else if alpha < static_eval {
            alpha = static_eval;
        }

        cand_moves = cand_moves.iter().copied().filter(|m| match m.typ {
                                                        MoveType::CAPTURE(_) => static_eval + thread.pos().see(*m)+delta > alpha && thread.pos().see(*m) > 0,
                                                        MoveType::PROMOTION(_) |
                                                        MoveType::PROMOTIONCAPTURE((_,_)) => true,
                                                        MoveType::ENPASSANT => static_eval+delta+100 > alpha,
                                                        _ => false}).collect();
        if cand_moves.len() == 0 {
            return static_eval;
        }
        cand_moves.sort_by_key(|m| match m.typ {
                                    MoveType::CAPTURE(_) => thread.pos().see(*m),
                                    MoveType::PROMOTION(p) => p.value()-Piece::PAWN.value(),
                                    MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => p_prom.value()+p_cap.value()-Piece::PAWN.value(),
                                    _ => 0,
                                });
    }

    for m in cand_moves {
        //stop if we receive the flag is set;
        if thread.stop_flag().read().unwrap().eq(&true) {
            return alpha;
        }

        thread.do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(thread, -beta, -alpha, delta-20, qply+1);
        thread.undo_move();

        if score >= beta {
            return score.to_lowerbound();
        } else if alpha < score {
            alpha = score;
        }
    }
    alpha
}

