mod history;
mod movepick;
mod thread;
mod tt;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
    chess::{Color, Move, MoveType, Piece, Position},
    constants::piece_value,
    evaluate::{Bound, Eval, Value, has_major_pieces, has_minor_pieces, is_material_draw},
};
use thread::{SearchHead, SearchResult, SharedData};
use tt::{TTEntry, TranspositionTable};

use thread::TimeManager;

pub struct SearchManager {
    pos: Position,
    threads: usize,
    shared: Arc<SharedData>,
}

impl SearchManager {
    pub fn new() -> SearchManager {
        SearchManager {
            pos: Position::new(),
            threads: 1,
            shared: Arc::new(SharedData::new()),
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        if self.shared.stop_flag.load(Ordering::Acquire) {
            self.shared = Arc::new(SharedData {
                nodes: AtomicU64::new(0),
                tt: TranspositionTable::new(
                    size * 1_000_000 / std::mem::size_of::<(TTEntry, TTEntry)>(),
                ),
                stop_flag: AtomicBool::new(true),
            });
        }
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threads = threads;
    }
    pub fn set_position(&mut self, pos: Position) {
        self.pos = pos;
    }
    pub fn do_move(&mut self, m: Move) {
        self.pos.do_move(m);
    }
    pub fn stop(&mut self) {
        self.shared.stop_flag.store(true, Ordering::Release);
    }
    pub fn color(&self) -> Color {
        self.pos.color()
    }
    pub fn reset_hash(&mut self) {
        self.shared.tt.clear();
    }
    pub fn root_position(&self) -> Position {
        self.pos.clone()
    }

    pub fn search(&mut self, target_depth: Option<u8>, time_limit: Option<std::time::Duration>) {
        let pos = self.pos.clone();
        let threads = self.threads;
        let shared = self.shared.clone();
        std::thread::spawn(move || start_searching(target_depth, time_limit, shared, pos, threads));
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

fn start_searching(
    target_depth: Option<u8>,
    time_limit: Option<std::time::Duration>,
    shared: Arc<SharedData>,
    pos: Position,
    threads: usize,
) {
    let depth = target_depth.unwrap_or(u8::MAX);
    let time_manager = TimeManager::new(std::time::Instant::now(), time_limit);

    shared.stop_flag.store(false, Ordering::Release);
    shared.nodes.store(0, Ordering::Release);

    let mut search_head = SearchHead::new(pos.clone(), shared.clone(), time_manager);

    let main_handle =
        std::thread::spawn(move || iterative_deepening::<true>(&mut search_head, depth));

    let mut helper_handles = Vec::new();

    for _ in 1..threads {
        let mut search_head = SearchHead::new(pos.clone(), shared.clone(), time_manager);

        helper_handles.push(std::thread::spawn(move || {
            iterative_deepening::<false>(&mut search_head, depth)
        }));
    }

    let mut move_votes = Vec::new();

    move_votes.push(main_handle.join().unwrap());

    for handle in helper_handles {
        move_votes.push(handle.join().unwrap());
    }

    let mut vote_map = HashMap::new();
    for vote in move_votes.iter().filter_map(|&x| x) {
        let weight = match vote.eval.value() {
            Value::Centis(n) => n,
            Value::Mate(n) => 10_000 - n,
            _ => 0,
        };
        *vote_map.entry(vote.mv.compress()).or_insert(0) += vote.depth as i32 * weight;
    }

    if let Some((m, _)) = vote_map.iter().max_by(|(_, v), (_, v2)| v.cmp(v2))
        && *m != 0
    {
        println!("bestmove {}", Move::decompress(*m).unwrap());
    } else {
        println!("bestmove (null)");
    }
}

fn iterative_deepening<const IS_MAIN: bool>(
    search_head: &mut SearchHead,
    depth: u8,
) -> Option<SearchResult> {
    let depth = if IS_MAIN { depth } else { u8::MAX };
    let mut alpha = Eval::MIN;
    let mut beta = Eval::MAX;
    for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        let mut eval;
        loop {
            eval = search_step::<true>(search_head, Depth::new(d as i16), 0, alpha, beta);

            // Make sure we only ever update with values that are from a complete search
            if search_head.shared_data().stop_flag.load(Ordering::Acquire) {
                return search_head.result;
            }

            if IS_MAIN {
                search_head.write_uci_info(eval, d);
            }

            match eval.bound() {
                Bound::Exact => {
                    alpha = eval.aspiration_lower(0);
                    beta = eval.aspiration_higher(0);
                    break;
                }
                Bound::Lower => {
                    fail_highs += 1;
                    beta = eval.aspiration_higher(fail_highs);
                }
                Bound::Upper => {
                    fail_lows += 1;
                    alpha = eval.aspiration_lower(fail_lows);
                }
            }
        }

        // register our newest vote for the best move
        search_head.result = Some(SearchResult {
            eval,
            mv: search_head.pv[0],
            depth: d,
        });
    }
    if IS_MAIN {
        search_head.shared.stop_flag.store(true, Ordering::Release);
    }
    search_head.result
}

//Parameters:
// sh: The search thread head.
// depth: the depth information consisting of target depth and reductions/extension
// null_moves: how many null moves have been performed in the current search
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
fn search_step<const IS_PV_NODE: bool>(
    sh: &mut SearchHead,
    mut depth: Depth,
    null_moves: u8,
    mut alpha: Eval,
    beta: Eval,
) -> Eval {
    if sh.shared.nodes.load(Ordering::Relaxed) & 0xff == 0
        && let Some(limit) = sh.time_manager.limit
        && sh.time_manager.start_time.elapsed() > limit
    {
        sh.shared.stop_flag.store(true, Ordering::Release);
        return Eval::DRAW;
    }

    //check for obviously drawn positions
    if is_material_draw(sh.pos()) {
        return Eval::DRAW;
    }

    //Repeated positions are draws
    if sh.pos.is_threefold() {
        return Eval::DRAW;
    }

    //If we cannot beat the score, just return immediately
    if alpha.value() == Value::Mate(1) {
        return Eval::mate_in(1).to_upperbound();
    }

    if IS_PV_NODE && depth.current <= 255 {
        sh.pv[depth.current as usize] = Move::ZERO;
    }

    //Check if the move is already hashed
    let hash_entry = sh.shared_data().tt.get(sh.pos().zobrist_hash());

    let mut ttmove = None;
    let mut tteval = None;

    if let Some(entry) = hash_entry {
        ttmove = entry.mov();
        tteval = Some(entry.eval());

        //make sure we do not return a repetition from tt, allowing a threefold
        let repetition = sh.pos.will_repeat(ttmove.unwrap());

        //see if we have a TT-hit
        if entry.depth() as i16 >= depth.remaining() && !repetition && !IS_PV_NODE {
            match entry.eval().bound() {
                Bound::Exact => {
                    if depth.current > 0 {
                        return entry.eval();
                    }
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
    let mut moves = sh.pos_mut().get_moves::<true>();

    if moves.is_empty() && sh.pos_mut().in_check() {
        return Eval::MATE_NOW;
    } else if moves.is_empty() {
        return Eval::STALEMATE;
    }

    //Calculate the depth we are still to search.
    let depth_left: u8 = depth.remaining().clamp(0, 255) as u8;
    let eval = tteval.unwrap_or(sh.evaluate());

    //We extend the normal search if we are  in check, else go into quiescence
    if depth_left == 0 && !sh.pos_mut().in_check() {
        return quiesce(sh, alpha, beta, 200 - 20 * depth.reduction as i32);
    }

    //Futility pruning
    if !IS_PV_NODE
        && depth.target > 3
        && ((depth_left == 1 && eval + 300 < alpha) || (depth_left == 2 && eval + 500 < alpha))
    {
        return quiesce(sh, alpha, beta, 100);
    }

    //Try a null move to find a beta cutoff; search the first three plys fully.
    //Should maybe avoid in late game?
    if !IS_PV_NODE
        && null_moves < std::cmp::max(depth.target as u8 / 6 + 1, 2)
        && (has_minor_pieces(sh.pos()) || has_major_pieces(sh.pos()))
        && !sh.pos_mut().in_check()
        && moves.len() > 2
        && depth.current > 2
        && !matches!(alpha.value(), Value::Mate(_))
        && !matches!(beta.value(), Value::Mate(_))
    {
        sh.do_null_move();
        let null_score = -search_step::<false>(
            sh,
            depth.reduce(3).next(),
            null_moves + 1,
            beta.neg_down(),
            alpha.neg_down(),
        );
        sh.undo_null_move();
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

    let move_picker = movepick::MovePicker::from_move_list(
        &mut moves,
        sh.pos(),
        depth.current,
        &sh.history,
        ttmove,
    );

    for (i, m) in move_picker.enumerate() {
        //lmr reduction depth
        let lmr = ((depth_left as f32).sqrt() * (i as f32).sqrt() / 5.) as i16;

        sh.do_move(m);

        let mut movescore = if i == 0 && IS_PV_NODE {
            -search_step::<true>(
                sh,
                depth.next(),
                null_moves,
                beta.neg_down(),
                alpha.neg_down(),
            )
        //Apply lmr at sufficiently high depths on non-PV nodes
        } else if !IS_PV_NODE && depth.current > 2 && !sh.pos_mut().in_check() && !m.is_tactical() {
            -search_step::<false>(
                sh,
                depth.reduce(lmr).next(),
                null_moves,
                beta.neg_down(),
                alpha.neg_down(),
            )
        //search late nodes in PV-nodes as zero windows
        } else {
            -search_step::<false>(
                sh,
                depth.reduce(lmr / 3).next(),
                null_moves,
                alpha.zero_window().neg_down(),
                alpha.neg_down(),
            )
        };

        //Research if we failed high in a PV node
        if IS_PV_NODE && i != 0 && movescore > alpha && movescore < beta {
            movescore = -search_step::<true>(
                sh,
                depth.next(),
                null_moves,
                beta.neg_down(),
                movescore.neg_down(),
            );
        }

        sh.undo_move();

        //Abort search if the helper gets a stop signal
        if sh.shared_data().stop_flag.load(Ordering::Relaxed).eq(&true) {
            return Eval::MIN;
        };

        //Adjust results
        if movescore >= beta {
            let zh = sh.pos().zobrist_hash();

            sh.shared.tt.set(
                zh,
                TTEntry::new(movescore.to_lowerbound(), depth_left, zh, m),
            );
            if !matches!(m.typ(), MoveType::Capture) && ttmove != Some(m) {
                sh.history.beta_cutoff(sh.pos.color(), m, depth.remaining());
            }
            sh.history.killer.invalidate(depth.current as usize);
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

    if IS_PV_NODE && depth.current <= 255 {
        sh.pv[depth.current as usize] = bestmove.unwrap();
    }

    //reset the killer move counts for ply+1
    sh.history.killer.invalidate(depth.current as usize);

    let zh = sh.pos().zobrist_hash();

    if fail_low {
        sh.shared.tt.set(
            zh,
            TTEntry::new(score.to_upperbound(), depth_left, zh, bestmove.unwrap()),
        );
        for m in moves.iter().filter(|&m| !m.is_capture()) {
            sh.history
                .alpha_cutoff(sh.pos.color(), *m, depth.remaining());
        }
        score.to_upperbound()
    } else {
        sh.shared.tt.set(
            zh,
            TTEntry::new(score.to_exact(), depth_left, zh, bestmove.unwrap()),
        );
        score.to_exact()
    }
}

fn quiesce(sh: &mut SearchHead, mut alpha: Eval, beta: Eval, delta: i32) -> Eval {
    if sh.shared.nodes.load(Ordering::Relaxed) & 0xff == 0
        && let Some(limit) = sh.time_manager.limit
        && sh.time_manager.start_time.elapsed() > limit
    {
        sh.shared.stop_flag.store(true, Ordering::Release);
        return Eval::DRAW;
    }

    //check for obviously drawn positions
    if is_material_draw(sh.pos()) {
        return Eval::DRAW;
    }

    //Search all evasions, else only captures.
    let mut cand_moves = if sh.pos_mut().in_check() {
        sh.pos_mut().get_moves::<true>()
    } else {
        sh.pos_mut().get_moves::<false>()
    };

    //check for terminal position
    if cand_moves.is_empty() && sh.pos_mut().in_check() {
        return Eval::MATE_NOW;
    }

    let static_eval = sh.evaluate();

    //If we are not in check we filter for tactical moves.
    if !sh.pos_mut().in_check() {
        //Adjust based on null-move hypothesis
        if static_eval >= beta {
            return static_eval.to_lowerbound();
        } else if alpha < static_eval {
            alpha = static_eval;
        }

        cand_moves = cand_moves
            .iter()
            .copied()
            .filter(|m| match m.typ() {
                MoveType::Capture => {
                    static_eval + sh.pos().see(*m) + delta > alpha && sh.pos().see(*m) > 0
                }
                MoveType::PromotionCaptureN
                | MoveType::PromotionCaptureB
                | MoveType::PromotionCaptureR
                | MoveType::PromotionCaptureQ => true,
                MoveType::Enpassant => static_eval + delta + 100 > alpha,
                _ => false,
            })
            .collect();
        if cand_moves.is_empty() {
            return static_eval;
        }
        cand_moves.sort_by_key(|m| -match m.typ() {
            MoveType::Capture => sh.pos().see(*m),
            MoveType::PromotionN => piece_value(Piece::Knight) - piece_value(Piece::Pawn),
            MoveType::PromotionB => piece_value(Piece::Bishop) - piece_value(Piece::Pawn),
            MoveType::PromotionR => piece_value(Piece::Rook) - piece_value(Piece::Pawn),
            MoveType::PromotionQ => piece_value(Piece::Queen) - piece_value(Piece::Pawn),
            MoveType::PromotionCaptureN => {
                let q = sh.pos().get_board().piece_at(m.to()).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureB => {
                let q = sh.pos().get_board().piece_at(m.to()).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureR => {
                let q = sh.pos().get_board().piece_at(m.to()).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            MoveType::PromotionCaptureQ => {
                let q = sh.pos().get_board().piece_at(m.to()).unwrap();
                piece_value(Piece::Knight) + piece_value(q) - piece_value(Piece::Pawn)
            }
            _ => 0,
        });
    }

    for m in cand_moves {
        //stop if we receive the flag is set;
        if sh
            .shared_data()
            .stop_flag
            .load(std::sync::atomic::Ordering::Relaxed)
            .eq(&true)
        {
            return alpha;
        }

        sh.do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(sh, -beta, -alpha, delta - 20);
        sh.undo_move();

        if score >= beta {
            return score.to_lowerbound();
        } else if alpha < score {
            alpha = score;
        }
    }
    alpha
}
