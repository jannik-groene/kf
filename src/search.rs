mod history;
mod movepick;
mod thread;
mod threadpool;
mod tt;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::search::movepick::MovePicker;
use crate::{
    chess::{Color, Move, Piece, Position},
    evaluate::{Bound, Eval, Value, has_major_pieces, has_minor_pieces, is_material_draw},
};
use thread::{SearchHead, SharedData};
use threadpool::ThreadPool;
use tt::TTEntry;

use thread::TimeManager;

pub struct SearchManager {
    pos: Position,
    threadpool: ThreadPool,
    shared: Arc<SharedData>,
}

impl SearchManager {
    pub fn new() -> SearchManager {
        let shared = Arc::new(SharedData::new());
        SearchManager {
            pos: Position::new(),
            threadpool: ThreadPool::new(shared.clone(), 1),
            shared,
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        if self.shared.stop_flag.load(Ordering::Acquire) {
            unsafe {
                self.shared
                    .tt
                    .resize(size * 1_000_000 / std::mem::size_of::<(TTEntry, TTEntry)>());
            }
        }
    }
    pub fn set_threads(&mut self, threads: usize) {
        self.threadpool.set_threads(threads);
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
    pub fn reset_thread_data(&mut self) {
        self.threadpool.reset_threads();
    }
    pub fn root_position(&self) -> Position {
        self.pos.clone()
    }

    pub fn search(&mut self, target_depth: Option<u8>, time_limit: Option<std::time::Duration>) {
        self.threadpool
            .start_searching(&self.pos, target_depth, time_limit);
    }
}

fn iterative_deepening(id: usize, search_head: &mut SearchHead, depth: u8) {
    let depth = if id == 0 { depth } else { u8::MAX };
    let mut alpha = Eval::MIN;
    let mut beta = Eval::MAX;
    'depth: for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        let mut eval;
        loop {
            eval = search_step::<true>(search_head, i32::from(d), 0, alpha, beta, false);

            // Make sure we only ever update with values that are from a complete search
            if search_head.shared_data().stop_flag.load(Ordering::Acquire) {
                break 'depth;
            }

            if id == 0 {
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
        let res =
            (eval.pack_for_tt() << 24) ^ (u64::from(search_head.pv[0].compress()) << 8) ^ u64::from(d);

        search_head.shared.results[id].store(res, Ordering::Release);
    }
    if id == 0 {
        search_head.shared.stop_flag.store(true, Ordering::Release);
        search_head.write_best_move();
    }
    search_head.pv.fill(Move::ZERO);
}

//Parameters:
// sh: The search thread head.
// depth: the depth information consisting of target depth and reductions/extension
// null_moves: how many null moves have been performed in the current search
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
fn search_step<const IS_PV_NODE: bool>(
    sh: &mut SearchHead,
    depth: i32,
    ply: usize,
    mut alpha: Eval,
    beta: Eval,
    cut_node: bool,
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

    if IS_PV_NODE && ply <= 255 {
        sh.pv[ply] = Move::ZERO;
    }

    //Check if the move is already hashed
    let hash_entry = sh.shared_data().tt.get(sh.pos().zobrist_hash());

    let mut ttmove = None;
    let mut tteval = None;

    if let Some(entry) = hash_entry {
        ttmove = Some(entry.mov());
        tteval = Some(entry.eval());

        //make sure we do not return a repetition from tt, allowing a threefold
        let repetition = sh.pos.will_repeat(ttmove.unwrap());

        //see if we have a TT-hit
        if i32::from(entry.depth()) >= depth && !repetition && !IS_PV_NODE {
            match entry.eval().bound() {
                Bound::Exact => {
                    if ply > 0 {
                        return entry.eval();
                    }
                }
                Bound::Lower => {
                    if let Some(m) = ttmove
                        && entry.eval() >= beta
                    {
                        if m.is_capture() {
                            let cap_bonus = (140 * depth - 105).min(1120);
                            sh.update_capture_history(m, cap_bonus);
                        } else {
                            let quiet_bonus = (100 * depth - 75).min(800);
                            let cont_bonus = (60 * depth - 45).min(480);
                            sh.update_quiet_history(m, quiet_bonus);
                            sh.update_continuation_history(m, cont_bonus);
                        }
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

    //We extend the normal search if we are  in check, else go into quiescence
    if depth <= 0 {
        return quiesce(sh, alpha, beta, 0);
    }

    let static_eval = sh.evaluate();
    // If we have a tt entry use its eval, else do static eval
    let eval = if let Some(v) = tteval
        && (v.bound() == Bound::Exact
            || v.bound() == Bound::Upper && v < static_eval
            || v.bound() == Bound::Lower && v > static_eval)
    {
        v
    } else {
        static_eval
    }
    .to_exact();

    //Razoring
    if !IS_PV_NODE && eval + 300 + 200 * (depth * depth) < alpha {
        return quiesce(sh, alpha, beta, 100);
    }

    //Reverse futility pruning
    if !IS_PV_NODE
        && !sh.pos_mut().in_check()
        && eval >= beta + 200 * depth
        && !matches!(eval.value(), Value::Mate(_))
        && !matches!(beta.value(), Value::Mate(_))
    {
        return eval.to_lowerbound();
    }

    //Try a null move to find a beta cutoff; verify by re-searching
    if !IS_PV_NODE // This should check for cut node, but that somehow loses ELO. Added bonus in
                   // cutoff estimate instead for now.
        && sh.next_null <= ply as i32
        && (has_minor_pieces(sh.pos()) || has_major_pieces(sh.pos()))
        && !sh.pos_mut().in_check()
        && moves.len() > 2
        && !matches!(alpha.value(), Value::Mate(_))
        && !matches!(beta.value(), Value::Mate(_))
        && (!ttmove.is_some_and(|m| m.is_capture())
            || !tteval.is_some_and(|e| e.bound() == Bound::Lower)
            || sh
                .pos
                .board
                .piece_at(ttmove.unwrap().to())
                .is_some_and(|p| p != Piece::Pawn))
        && eval >= beta + 50 - 20 * depth - 50 * i32::from(cut_node)
    {
        let reduction = 5 + (depth + i32::from(cut_node)) / 4;
        sh.do_null_move();
        let null_score = -search_step::<false>(
            sh,
            depth - reduction,
            ply + 1,
            beta.neg_down(),
            alpha.neg_down(),
            false,
        );
        sh.undo_null_move();
        if null_score >= beta && !matches!(null_score.value(), Value::Mate(_)) {
            if sh.next_null != 0 || depth < 8 {
                return null_score.to_lowerbound();
            }

            sh.next_null = ply as i32 + ((depth - reduction) * 3 / 4);
            let veri_score = search_step::<false>(sh, depth - reduction, ply, alpha, beta, false);
            sh.next_null = 0;

            if veri_score >= beta {
                return null_score.to_lowerbound();
            }
        }
    }

    //Set up paramaters
    let mut score = Eval::MIN;
    let mut fail_low = true;
    let mut bestmove = None;

    let mut extension = 0;

    if moves.len() == 1 {
        extension += 1;
    }

    let move_picker = movepick::MovePicker::from_move_list(
        &mut moves,
        sh.pos(),
        depth,
        &sh.history,
        ttmove,
        None,
    );

    for (i, m) in move_picker.enumerate() {
        //LMP
        if i > 3 + (depth * depth) as usize
            && ply > 0
            && !m.is_capture()
            && !sh.pos.in_check()
            && !sh.pos.gives_check(m)
            && !matches!(beta.value(), Value::Mate(_))
            && !matches!(score.value(), Value::Mate(_))
        {
            continue;
        }

        sh.do_move(m);

        let mut movescore = if i == 0 && IS_PV_NODE {
            -search_step::<true>(
                sh,
                depth + extension - 1,
                ply + 1,
                beta.neg_down(),
                alpha.neg_down(),
                false,
            )
        //Apply lmr at sufficiently high depths
        } else if depth >= 2 && i > 0 {
            //lmr reduction depth
            let mut reduction = 200 * (depth.ilog2() * (i + 1).ilog2()) as i32;
            reduction += 900 * i32::from(cut_node);
            reduction += 1500 * i32::from(!m.is_capture());
            reduction -= 1500 * i32::from(IS_PV_NODE);
            if cut_node && ttmove.is_none() {
                reduction += 500;
            }

            let mut val = -search_step::<false>(
                sh,
                (depth - reduction / 1024 - 1).max(1),
                ply + 1,
                alpha.zero_window().neg_down(),
                alpha.neg_down(),
                !cut_node,
            );
            // Verify on fail high
            if val > alpha {
                val = -search_step::<false>(
                    sh,
                    depth - 1,
                    ply + 1,
                    alpha.zero_window().neg_down(),
                    alpha.neg_down(),
                    !cut_node,
                );
            }
            val
        // Reduce less otherwise
        } else {
            let mut reduction = 100 * (depth.ilog2() * (i + 1).ilog2()) as i32;
            reduction += 200 * i32::from(cut_node);
            reduction += 1000 * i32::from(!m.is_capture());
            reduction -= 1000 * i32::from(IS_PV_NODE);
            if Some(m) == ttmove {
                reduction -= 1000;
            } else if cut_node && ttmove.is_none() {
                reduction += 300;
            }

            -search_step::<false>(
                sh,
                depth - (reduction / 2048).clamp(0, 2) - 1,
                ply + 1,
                alpha.zero_window().neg_down(),
                alpha.neg_down(),
                !cut_node,
            )
        };

        //Research if we failed high in a PV node
        if IS_PV_NODE && i != 0 && movescore > alpha {
            movescore = -search_step::<true>(
                sh,
                depth + extension - 1,
                ply + 1,
                beta.neg_down(),
                alpha.neg_down(),
                false,
            );
        }

        sh.undo_move();

        //Abort search if the helper gets a stop signal
        if sh.shared_data().stop_flag.load(Ordering::Relaxed).eq(&true) {
            return Eval::MIN;
        }

        //Adjust results
        if movescore >= beta {
            let zh = sh.pos().zobrist_hash();

            sh.shared.tt.set(
                zh,
                TTEntry::new(movescore.to_lowerbound(), depth.clamp(0, 255) as u8, zh, m),
            );

            if m.is_capture() {
                let cap_bonus = (140 * depth - 105).min(1120);
                sh.update_capture_history(m, cap_bonus);
            } else {
                let quiet_bonus = (100 * depth - 75).min(800);
                let cont_bonus = (60 * depth - 45).min(480);
                sh.update_quiet_history(m, quiet_bonus);
                sh.update_continuation_history(m, cont_bonus);
                sh.history.killer.register(m, ply);
            }
            sh.history.killer.invalidate(ply);
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

    if IS_PV_NODE && ply <= 255 {
        sh.pv[ply] = bestmove.unwrap();
    }

    //reset the killer move counts for ply+1
    sh.history.killer.invalidate(ply);

    let quiet_bonus = (100 * depth - 75).min(800);
    let quiet_malus = quiet_bonus / 4;

    let cont_bonus = (60 * depth - 45).min(480);
    let cont_malus = cont_bonus * 3 / 2;

    let cap_bonus = (140 * depth - 105).min(1120);
    let cap_malus = cap_bonus / 4;

    //Quiet Histories
    if let Some(m) = bestmove
        && !m.is_capture()
    {
        //Update Quiet Histories
        sh.update_continuation_history(m, quiet_bonus);
        sh.update_quiet_history(m, cont_bonus);
    } else if let Some(m) = bestmove {
        sh.update_capture_history(m, cap_bonus);
    }

    let zh = sh.pos().zobrist_hash();

    for m in moves {
        if Some(m) == bestmove {
            continue;
        }
        if !m.is_capture() && bestmove.is_some_and(|bm| !bm.is_capture()) {
            // Only update quiet history if the best move is quiet
            sh.update_quiet_history(m, -quiet_malus);
            sh.update_continuation_history(m, -cont_malus);
        } else if m.is_capture() {
            sh.update_capture_history(m, -cap_malus);
        }
    }

    if fail_low {
        sh.shared.tt.set(
            zh,
            TTEntry::new(
                score.to_upperbound(),
                depth.clamp(0, 255) as u8,
                zh,
                bestmove.unwrap(),
            ),
        );
        score.to_upperbound()
    } else {
        //Write to TT
        sh.shared.tt.set(
            zh,
            TTEntry::new(
                score.to_exact(),
                depth.clamp(0, 255) as u8,
                zh,
                bestmove.unwrap(),
            ),
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

    //Check if the move is already hashed
    let hash_entry = sh.shared_data().tt.get(sh.pos().zobrist_hash());

    let ttmove = hash_entry.map(|e| e.mov());
    let tteval = hash_entry.map(|e| e.eval());

    if let Some(eval) = tteval
        && !matches!(eval.value(), Value::Mate(_))
        && ((eval.bound() == Bound::Upper && eval < alpha)
            || (eval.bound() == Bound::Lower && eval >= beta)
            || eval.bound() == Bound::Exact)
    {
        return eval;
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

        if cand_moves.is_empty() {
            return static_eval;
        }
    }

    let val = match alpha.value() {
        Value::Centis(n) => n,
        Value::Mate(n) => {
            if n % 2 == 0 {
                i32::MIN
            } else {
                i32::MAX
            }
        }
        Value::Infty => i32::MAX,
        Value::NegInfty => i32::MIN,
    } - if let Value::Centis(c) = static_eval.value() {
        c
    } else {
        0
    } - delta;

    let cutoff = if sh.pos.in_check() { None } else {  Some(val) };

    let move_picker =
        MovePicker::from_move_list(&mut cand_moves, &sh.pos, 255, &sh.history, ttmove, cutoff);

    let mut best_score = if sh.pos.in_check() {
        Eval::MIN
    } else {
        static_eval
    };

    alpha = alpha.max(best_score);

    for (i, m) in move_picker.enumerate() {
        //stop if we receive the flag is set;
        if sh
            .shared_data()
            .stop_flag
            .load(std::sync::atomic::Ordering::Relaxed)
            .eq(&true)
        {
            return Eval::DRAW;
        }

        if i >= 3 && !matches!(beta.value(), Value::Mate(_)) && !sh.pos.gives_check(m) {
            continue;
        }

        sh.do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(sh, beta.neg_down(), alpha.neg_down(), delta - 50);
        sh.undo_move();

        if score >= beta {
            return score.to_lowerbound();
        } else if best_score < score {
            best_score = score;
            if best_score > alpha {
                alpha = best_score;
            }
        }
    }
    best_score
}
