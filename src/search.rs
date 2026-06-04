mod movepick;
mod thread;
mod tt;

use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::io::Write;

use crate::{
    chess::{Color, Move, MoveType, Piece, Position, Square},
    constants::piece_value,
    engine::EngineIO,
    evaluate::{has_major_pieces, has_minor_pieces, is_material_draw, Bound, Eval, Value},
};
use thread::{MainThread, SearchHead};
use tt::{TTEntry, TranspositionTable};

#[derive(Clone)]
pub struct SearchInfo {
    pub bestmove: Option<Move>,
    pub eval: Eval,
    pub depth: u8,
    pub nodes: u64,
    pub id: u64,
}

impl SearchInfo {
    fn new(id: u64) -> SearchInfo {
        SearchInfo {
            bestmove: None,
            eval: Eval::MIN,
            depth: 0,
            nodes: 0,
            id,
        }
    }
}

pub struct SearchManager {
    pos: Position,
    pos_history: Vec<Position>,
    threads: usize,
    tt: TranspositionTable,
    stop_flag: Arc<RwLock<bool>>,
    search_info: SearchInfo,
    use_nnue: bool,
}

impl SearchManager {
    pub fn new() -> SearchManager {
        SearchManager {
            pos: Position::new(),
            pos_history: Vec::new(),
            threads: 1,
            tt: TranspositionTable::new(2),
            stop_flag: Arc::new(RwLock::new(false)),
            search_info: SearchInfo::new(0),
            use_nnue: false,
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        self.tt =
            TranspositionTable::new(size * 1_000_000 / std::mem::size_of::<(TTEntry, TTEntry)>());
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }
    pub fn set_position(&mut self, pos: Position) {
        self.pos = pos;
        self.pos_history.clear();
    }
    pub fn do_move(&mut self, m: Move) {
        self.pos_history.push(self.pos.clone());
        self.pos.do_move(m);
    }
    pub fn set_use_nnue(&mut self, use_nnue: bool) {
        self.use_nnue = use_nnue;
    }
    pub fn search(
        &mut self,
        out_channel: Sender<EngineIO>,
        target_depth: Option<u8>,
        search_id: u64,
    ) -> std::thread::JoinHandle<()> {
        let depth = target_depth.unwrap_or(u8::MAX);
        self.reset_search_info(search_id);
        self.stop_flag = Arc::new(RwLock::new(false));
        let mut root_search_info = MainThread::new(
            self.pos.clone(),
            self.tt.clone(),
            self.pos_history.clone(),
            self.stop_flag.clone(),
            self.threads,
            self.use_nnue,
            out_channel,
            self.search_info.clone(),
        );
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
        let size = self.tt.size();
        self.tt = TranspositionTable::new(size);
    }
    pub fn root_position(&self) -> Position {
        self.pos.clone()
    }
}

#[derive(Copy, Clone)]
struct Depth {
    pub target: i16,
    pub current: i16,
    pub reduction: i16,
    pub extension: i16,
}

impl Depth {
    fn new(target: i16) -> Self {
        Self {
            target,
            current: 0,
            reduction: 0,
            extension: 0,
        }
    }

    fn remaining(&self) -> i16 {
        self.target + self.extension - self.current - self.reduction
    }

    fn reduce(mut self, reduction: i16) -> Self {
        self.reduction += reduction;
        self
    }

    fn extend(mut self, extension: i16) -> Self {
        self.extension += extension;
        self
    }

    fn next(mut self) -> Self {
        self.current += 1;
        self
    }
}

fn search(thread: &mut MainThread, depth: u8, mut alpha: Eval, mut beta: Eval) {
    let now = Instant::now();
    let mut helper_handles = Vec::new();
    let helper_stop_flag = Arc::new(RwLock::new(false));
    for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        loop {
            //Spawn helper threads for search
            if thread.threads() > 1 && d > 2 {
                for _ in 1..thread.threads() {
                    let mut search_head = SearchHead::new(thread.search_head().pos().clone(),
                                                          thread.search_head().tt().clone(),
                                                          thread.search_head().history(),
                                                          helper_stop_flag.clone(), 
                                                          thread.uses_nnue());
                    helper_handles.push(std::thread::spawn(move || {
                        search_helper(
                            &mut search_head,
                            d,
                            alpha,
                            beta,
                        )
                    }));
                }
            }
            let eval = search_step(thread.search_head_mut(), Depth::new(d as i16), 0, false, alpha, beta);
            if thread.search_head().stop_flag().read().unwrap().eq(&true) {
                *helper_stop_flag.write().unwrap() = true;
                thread.send_info(EngineIO::SearchEnded(thread.search_info().id));
                return;
            }
            if d > 2 {
                //Set stop flag and join all helpers
                //*helper_stop_flag.write().unwrap() = true;
                for helper in helper_handles {
                    match helper.join() {
                        //Concerning but not fatal?
                        Err(_) => {}
                        Ok(n) => *thread.search_head_mut().nodes_mut() += n,
                    }
                }
                helper_handles = Vec::new();
                //Reset stop flag
                *helper_stop_flag.write().unwrap() = false | *thread.search_head().stop_flag().read().unwrap();
            }
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            drop(write!(handle,
                "info nodes {} nps {}",
                thread.search_head().nodes(),
                1000 * *thread.search_head().nodes() as u128 / now.elapsed().as_millis().clamp(1, u128::MAX)
            ));
            //We reached the target depth and stopped, so we update the external values
            match eval.bound() {
                Bound::Exact => {
                    drop(write!(handle, " {} depth {} time {}", eval, d, now.elapsed().as_millis()));
                    thread.print_pv(d, &mut handle);
                    thread.search_info_mut().eval = eval;
                    thread.search_info_mut().bestmove = thread.search_head().bestmove();
                    thread.send_info(EngineIO::SearchUpdate(thread.search_info().clone()));
                    alpha = eval.aspiration_lower(0);
                    beta = eval.aspiration_higher(0);
                    break;
                }
                Bound::Lower => {
                    drop(writeln!(handle, " {} depth {} time {}", eval, d, now.elapsed().as_millis()));
                    fail_highs += 1;
                    beta = eval.aspiration_higher(fail_highs);
                }
                Bound::Upper => {
                    drop(writeln!(handle, " {} depth {} time {}", eval, d, now.elapsed().as_millis()));
                    fail_lows += 1;
                    alpha = eval.aspiration_lower(fail_lows);
                }
            }
        }
    }
    thread.send_info(EngineIO::SearchEnded(thread.search_info().id));
}

fn search_helper(helper: &mut SearchHead, depth: u8, alpha: Eval, beta: Eval) -> u64 {
    search_step(helper, Depth::new(depth as i16), 0, false, alpha, beta);
    *helper.nodes()
}

fn is_tactical(m: Move) -> bool {
    m.typ == MoveType::Capture || m.typ.is_promotion()
}
}

//Parameters:
// thread: The search thread head.
// depth: the depth information consisting of target depth and reductions/extension
// null_moves: how many null moves have been performed in the current search
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
fn search_step(
    thread: &mut SearchHead,
    mut depth: Depth,
    null_moves: u8,
    zw: bool,
    mut alpha: Eval,
    beta: Eval,
) -> Eval {
    //check for obviously drawn positions
    if is_material_draw(thread.pos()) {
        return Eval::DRAW;
    }

    //Repeated positions are draws
    if thread.is_threefold() {
        return Eval::DRAW;
    }

    //If we cannot beat the score, just return immediately
    if alpha.value() == Value::Mate(1) {
        return Eval::mate_in(1).to_upperbound();
    }

    //Check if the move is already hashed
    let hash_entry = thread.tt().get(thread.pos().zobrist_hash());

    let mut ttmove = None;

    if let Some(entry) = hash_entry {
        ttmove = entry.mov();

        //make sure we do not return a repetition from tt, allowing a threefold
        thread.do_move(ttmove.unwrap());
        let threefold = thread.is_repetition();
        thread.undo_move();

        //see if we have a TT-hit
        if entry.depth() as i16 >= depth.remaining() && !threefold && zw {
            match entry.eval().bound() {
                Bound::Exact => {
                    if depth.current == 0 {
                        thread.set_bestmove(ttmove);
                    }
                    return entry.eval();
                }
                Bound::Lower => {
                    if entry.eval() >= beta {
                        return entry.eval();
                    }
                }
                Bound::Upper => {
                    if entry.eval() < alpha {
                        return entry.eval();
                    }
                }
            }
        }
    }

    //Check if this is a terminal position
    let moves = thread.pos_mut().get_moves();

    if moves.is_empty() && thread.pos_mut().in_check() {
        return Eval::MATE_NOW;
    } else if moves.is_empty() {
        return Eval::STALEMATE;
    }

    //Calculate the depth we are still to search.
    let depth_left: u8 = depth.remaining().clamp(0, 255) as u8;

    //We extend the normal search if we are  in check, else go into quiescence
    if depth_left == 0 && !thread.pos_mut().in_check() {
        return quiesce(thread, alpha, beta, 200 - 20 * depth.reduction as i32, 0);
    }
    //Futility pruning
    else if depth.target > 3 && depth_left == 1 {
        let eval = thread.evaluate();
        if eval + 300 < alpha {
            return quiesce(thread, alpha, beta, 100, 0);
        }
    }
    //Extended futility pruning
    else if depth.target > 3 && depth_left == 2 {
        let eval = thread.evaluate();
        if eval + 500 < alpha {
            return quiesce(thread, alpha, beta, 100, 0);
        }
    }

    //Try a null move to find a beta cutoff; search the first three plys fully.
    //Should maybe avoid in late game?
    if zw
        && null_moves < std::cmp::max(depth.target as u8 / 6 + 1, 2)
        && (has_minor_pieces(thread.pos()) || has_major_pieces(thread.pos()))
        && !thread.pos_mut().in_check()
        && moves.len() > 2
        && depth.current > 2
        && !matches!(alpha.value(), Value::Mate(_))
        && !matches!(beta.value(), Value::Mate(_))
    {
        thread.do_null_move();
        let null_score = -search_step(
            thread,
            depth.reduce(3).next(),
            null_moves + 1,
            zw,
            beta.neg_down(),
            alpha.neg_down(),
        );
        thread.undo_null_move();
        if null_score >= beta {
            return null_score.to_lowerbound();
        }
    }

    //Set up paramaters
    let mut score = Eval::MIN;
    let mut fail_low = true;
    let mut bestmove = None;

    if moves.len() == 1 {
        depth = depth.extend(1);
    }

    let killers = *thread.get_killers(depth.current as u8);
    let move_picker = movepick::MovePicker::new(thread.pos_mut(), killers, ttmove);

    for (i, m) in move_picker.enumerate() {
        //lmr reduction depth
        let lmr = ((depth_left as f32).sqrt() * (i as f32).sqrt() / 5.) as i16;

        thread.do_move(m);

        let mut movescore = if i == 0 && !zw {
            -search_step(
                thread,
                depth.next(),
                null_moves,
                false,
                beta.neg_down(),
                alpha.neg_down(),
            )
        //Apply lmr at sufficiently high depths on non-PV nodes
        } else if zw
            && depth.current > 2
            && !thread.pos_mut().in_check()
            && !is_tactical(m)
        {
            -search_step(
                thread,
                depth.reduce(lmr).next(),
                null_moves,
                true,
                beta.neg_down(),
                alpha.neg_down(),
            )
        //search late nodes in PV-nodes as zero windows
        } else {
            -search_step(
                thread,
                depth.reduce(lmr/3).next(),
                null_moves,
                true,
                alpha.zero_window().neg_down(),
                alpha.neg_down(),
            )
        };

        //Research if we failed high in a PV node
        if !zw && i != 0 && movescore > alpha && movescore < beta {
            movescore = -search_step(
                thread,
                depth.next(),
                null_moves,
                false,
                beta.neg_down(),
                movescore.neg_down(),
            );
        }

        thread.undo_move();

        //Abort search if the helper gets a stop signal
        if thread.stop_flag().read().unwrap().eq(&true) {
            return Eval::MIN;
        };

        //Adjust results
        if movescore >= beta {
            let zh = thread.pos().zobrist_hash();

            thread.tt_mut().set(
                zh,
                TTEntry::new(movescore.to_lowerbound(), depth_left, zh, m),
            );
            if !matches!(m.typ, MoveType::Capture) && ttmove != Some(m) {
                thread.register_killer(depth.current as u8, m);
            }
            thread.invalidate_killers(depth.current as u8);
            return movescore.to_lowerbound();
        }

        if movescore > score {
            bestmove = Some(m);
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

    if depth.current == 0 {
        thread.set_bestmove(bestmove);
    }

    //reset the killer move counts for ply+1
    thread.invalidate_killers(depth.current as u8);

    let zh = thread.pos().zobrist_hash();

    if fail_low {
        thread.tt_mut().set(
            zh,
            TTEntry::new(score.to_upperbound(), depth_left, zh, bestmove.unwrap()),
        );
        score.to_upperbound()
    } else {
        thread.tt_mut().set(
            zh,
            TTEntry::new(score.to_exact(), depth_left, zh, bestmove.unwrap()),
        );
        score.to_exact()
    }
}

fn quiesce(thread: &mut SearchHead, mut alpha: Eval, beta: Eval, delta: i32, qply: u8) -> Eval {
    //check for obviously drawn positions
    if is_material_draw(thread.pos()) {
        return Eval::DRAW;
    }

    let mut cand_moves = thread.pos_mut().get_moves();

    //check for terminal position
    if cand_moves.is_empty() {
        if thread.pos_mut().in_check() {
            return Eval::MATE_NOW;
        } else {
            return Eval::STALEMATE;
        }
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

        cand_moves = cand_moves
            .iter()
            .copied()
            .filter(|m| match m.typ {
                MoveType::Capture => {
                    static_eval + thread.pos().see(*m) + delta > alpha && thread.pos().see(*m) > 0
                }
                MoveType::PromotionN | MoveType::PromotionB | MoveType::PromotionR 
                                     | MoveType::PromotionQ | MoveType::PromotionCaptureN
                                     | MoveType::PromotionCaptureB | MoveType::PromotionCaptureR
                                     | MoveType::PromotionCaptureQ                               => true,
                MoveType::Enpassant => static_eval + delta + 100 > alpha,
                _ => false,
            })
            .collect();
        if cand_moves.is_empty() {
            return static_eval;
        }
        cand_moves.sort_by_key(|m| -match m.typ {
            MoveType::Capture => thread.pos().see(*m),
            MoveType::PromotionN => piece_value(Piece::Knight) - piece_value(Piece::Pawn),
            MoveType::PromotionB => piece_value(Piece::Bishop) - piece_value(Piece::Pawn),
            MoveType::PromotionR => piece_value(Piece::Rook) - piece_value(Piece::Pawn),
            MoveType::PromotionQ => piece_value(Piece::Queen) - piece_value(Piece::Pawn),
            MoveType::PromotionCaptureN => {
                let q = thread.pos().get_board().piece_at(m.to).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureB => {
                let q = thread.pos().get_board().piece_at(m.to).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureR => {
                let q = thread.pos().get_board().piece_at(m.to).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureQ => {
                let q = thread.pos().get_board().piece_at(m.to).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
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
        let score = -quiesce(thread, -beta, -alpha, delta - 20, qply + 1);
        thread.undo_move();

        if score >= beta {
            return score.to_lowerbound();
        } else if alpha < score {
            alpha = score;
        }
    }
    alpha
}


#[test]
fn threefold_detection() {
    let pos = Position::from_fen(String::from("rnbqkbnr/ppp1pppp/3p4/8/8/3P4/PPP1PPPP/RNBQKBNR w KQkq - 0 2")).unwrap();
    let tt = TranspositionTable::new(2);
    let sf = Arc::new(RwLock::new(false));
    let mut head = SearchHead::new(pos, tt, Vec::new(), sf, false);
    let moves = [
        Move { from: Square::D1, to: Square::D2, typ: MoveType::Normal },
        Move { from: Square::D8, to: Square::D7, typ: MoveType::Normal },
        Move { from: Square::D2, to: Square::D1, typ: MoveType::Normal },
        Move { from: Square::D7, to: Square::D8, typ: MoveType::Normal },
    ];
    for m in moves {
        head.do_move(m);
    }
    assert!(head.is_repetition());
    assert!(!head.is_threefold());
    for m in moves {
        head.do_move(m);
    }
    assert!(head.is_threefold());

    let pos = Position::from_fen(String::from("8/8/pbpk1r2/p2pNn2/R2P1PKp/7P/1PP1NP2/8 w - - 2 41")).unwrap();
    let tt = TranspositionTable::new(2);
    let sf = Arc::new(RwLock::new(false));
    let mut head = SearchHead::new(pos, tt, Vec::new(), sf, false);
    let moves = [
        Move { from: Square::E5, to: Square::F3, typ: MoveType::Normal },
        Move { from: Square::B6, to: Square::D8, typ: MoveType::Normal },
        Move { from: Square::F3, to: Square::E5, typ: MoveType::Normal },
        Move { from: Square::D8, to: Square::B6, typ: MoveType::Normal },
    ];
    for m in moves {
        head.do_move(m);
    }
    assert!(head.is_repetition());
    assert!(!head.is_threefold());
    for m in moves {
        head.do_move(m);
    }
    assert!(head.is_threefold());
}
