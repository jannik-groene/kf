mod history;
mod movepick;
mod thread;
mod threadpool;
mod tt;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::search::movepick::MovePicker;
pub use crate::search::thread::SearchLimit;
use crate::{
    chess::{Color, Move, Piece, Position},
    evaluate::{Bound, eval, has_major_pieces, has_minor_pieces, is_material_draw},
    report::Reporter,
};
use thread::{SearchHead, SharedData};
use threadpool::ThreadPool;

pub struct SearchManager<T: Reporter> {
    pos: Position,
    threadpool: ThreadPool<T>,
    shared: Arc<SharedData>,
}

impl<T: Reporter> SearchManager<T> {
    pub fn new(reporter: T) -> SearchManager<T> {
        let shared = Arc::new(SharedData::new());
        SearchManager::<T> {
            pos: Position::new(),
            threadpool: ThreadPool::new(shared.clone(), 1, reporter),
            shared,
        }
    }
    pub fn set_hash_size(&mut self, size: usize) {
        if self.shared.stop_flag.load(Ordering::Acquire) {
            unsafe {
                self.shared.tt.resize(size);
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

    pub fn search(&mut self, limit: SearchLimit) {
        self.threadpool.start_searching(&self.pos, limit);
    }
}

fn iterative_deepening<T: Reporter>(id: usize, search_head: &mut SearchHead, reporter: &T) {
    let depth = if let SearchLimit::Depth(d) = search_head.limit
        && id == 0
    {
        d
    } else {
        u8::MAX
    };
    let mut alpha = -eval::INFTY;
    let mut beta = eval::INFTY;
    'depth: for d in 1..=depth {
        let mut fail_highs = 0;
        let mut fail_lows = 0;
        let mut eval;
        loop {
            search_head.sel_depth = 0;
            eval = search_step::<true>(search_head, i32::from(d), 0, alpha, beta, false);

            // Make sure we only ever update with values that are from a complete search
            if search_head.shared_data().stop_flag.load(Ordering::Acquire) {
                break 'depth;
            }

            if eval <= alpha {
                fail_lows += 1;
                alpha = eval::aspiration_lower(eval, fail_lows);
                if id == 0 {
                    search_head.report_update(eval, Bound::Upper, d, reporter);
                }
            } else if eval >= beta {
                fail_highs += 1;
                beta = eval::aspiration_higher(eval, fail_highs);
                if id == 0 {
                    search_head.report_update(eval, Bound::Lower, d, reporter);
                }
            } else {
                alpha = eval::aspiration_lower(eval, 0);
                beta = eval::aspiration_higher(eval, 0);
                if id == 0 {
                    search_head.report_update(eval, Bound::Exact, d, reporter);
                }
                break;
            }
        }

        // register our newest vote for the best move
        let res = (eval::pack_for_tt(eval, 0) << 24)
            ^ (u64::from(search_head.pv[0].compress()) << 8)
            ^ u64::from(d);

        search_head.shared.results[id].store(res, Ordering::Release);

        // soft bound check
        if let SearchLimit::Time { soft, .. } = search_head.limit
            && id == 0
            && search_head.start_time.elapsed() > soft
        {
            search_head.shared.stop_flag.store(true, Ordering::Release);
            break;
        }
    }
    if T::REPORT {
        search_head.shared.stop_flag.store(true, Ordering::Release);
        search_head.report_best_move(reporter);
    }
    search_head.pv.fill(Move::ZERO);
}

//Parameters:
// sh: The search thread head.
// depth: the remaining depth
// ply: the current search ply
// alpha: the alpha value of the current ab search
// beta: the beta of the current ab search
// cut_node: whether we expect to see a beta cutoff
fn search_step<const IS_PV_NODE: bool>(
    sh: &mut SearchHead,
    depth: i32,
    ply: usize,
    mut alpha: i32,
    beta: i32,
    cut_node: bool,
) -> i32 {
    if IS_PV_NODE && ply <= 254 {
        sh.pv[ply] = Move::ZERO;
        sh.pv[ply + 1] = Move::ZERO;
    }

    if sh.shared.nodes.load(Ordering::Relaxed) & 0xff == 0 && sh.limit.should_stop(sh.start_time) {
        sh.shared.stop_flag.store(true, Ordering::Release);
        return eval::DRAW;
    }

    //check for obviously drawn positions
    if is_material_draw(sh.pos()) {
        return eval::DRAW;
    }

    //Repeated positions are draws; Stop searching if we see a position again within our search or a
    //third time over the whole game
    if ply > 0 && (sh.pos.is_repetition_in_plys(ply) || sh.pos.is_threefold()) {
        return eval::DRAW;
    }

    //50 move rule
    if sh.pos.rule_50_count() >= 100 {
        return eval::DRAW;
    }

    //If we cannot beat the score, just return immediately
    if alpha == eval::mate_in(ply + 1) {
        return alpha;
    }

    //Check if the move is already hashed
    let hash_entry = sh.shared_data().tt.get(sh.pos().zobrist_hash());

    let mut ttmove = None;
    let mut tteval = None;
    let mut ttbound = None;

    if let Some(entry) = hash_entry
        && sh.pos.is_legal(entry.mov())
    {
        let eval = entry.eval(ply);
        let bound = entry.bound();
        ttmove = Some(entry.mov());
        tteval = Some(eval);
        ttbound = Some(bound);
        assert!(entry.mov() != Move::ZERO);

        //make sure we do not return a repetition from tt, allowing a threefold
        let repetition = sh.pos.will_repeat(ttmove.unwrap());

        //see if we have a TT-hit
        if i32::from(entry.depth()) >= depth && !repetition && !IS_PV_NODE {
            match bound {
                Bound::Exact => {
                    if ply > 0 {
                        return eval;
                    }
                }
                Bound::Lower => {
                    if let Some(m) = ttmove
                        && eval >= beta
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
                        return eval;
                    }
                }
                Bound::Upper => {
                    if eval < alpha {
                        return eval;
                    }
                }
            }
        }
    }

    //Go into quiescence search when target depth has been reached
    if depth <= 0 {
        return quiesce(sh, alpha, beta, 0, ply);
    }

    let static_eval = sh.evaluate();
    // If we have a tt entry use its eval, else do static eval
    let eval = if let Some(v) = tteval
        && (ttbound == Some(Bound::Exact)
            || ttbound == Some(Bound::Upper) && v < static_eval
            || ttbound == Some(Bound::Lower) && v > static_eval)
    {
        v
    } else {
        let pawn_hash = sh.pos.pawn_hash();
        static_eval
            + sh.history
                .correction
                .get(sh.pos.color(), pawn_hash)
                / 64
    };

    //Razoring
    if !IS_PV_NODE && !eval::is_win(alpha) && eval + 300 + 200 * (depth * depth) < alpha {
        return quiesce(sh, alpha, beta, 100, ply);
    }

    let in_check = sh.pos.in_check();

    //Reverse futility pruning
    if !IS_PV_NODE
        && !in_check
        && eval >= beta + 150 * depth - 50 * i32::from(cut_node)
        && !eval::is_decisive(beta)
    {
        return eval;
    }

    //Try a null move to find a beta cutoff; verify by re-searching
    if !IS_PV_NODE // This should check for cut node, but that somehow loses ELO. Added bonus in
                   // cutoff estimate instead for now.
        && sh.next_null <= ply as i32
        && (has_minor_pieces(sh.pos()) || has_major_pieces(sh.pos()))
        && !in_check
        && !eval::is_decisive(alpha)
        && !eval::is_decisive(beta) // This check seems redundant, since we must be in zw-search here
                                    // anyway?
        && (!ttmove.is_some_and(|m| m.is_capture())
            || ttbound != Some(Bound::Lower)
            || sh
                .pos
                .board
                .piece_at(ttmove.unwrap().to())
                .is_some_and(|p| p == Piece::Pawn))
        && eval >= beta + 50 - 20 * depth - 50 * i32::from(cut_node)
    {
        let reduction = 5 + (depth + i32::from(cut_node)) / 4;

        sh.do_null_move();
        let null_score =
            -search_step::<false>(sh, depth - reduction, ply + 1, -beta, -alpha, false);
        sh.undo_null_move();

        if null_score >= beta && !eval::is_decisive(null_score) {
            if sh.next_null != 0 || depth < 8 {
                return null_score;
            }

            sh.next_null = ply as i32 + ((depth - reduction) * 3 / 4);
            let veri_score = search_step::<false>(sh, depth - reduction, ply, alpha, beta, false);
            sh.next_null = 0;

            if veri_score >= beta {
                return null_score;
            }
        }
    }

    //Set up paramaters
    let mut score = -eval::INFTY;
    let mut bound = Bound::Upper;
    let mut bestmove = None;

    let mut move_picker = movepick::MovePicker::new(sh, ply as i32, ttmove, None);
    let mut move_idx = 0;

    while let Some(m) = move_picker.next(sh) {
        move_idx += 1;
        let i = move_idx - 1;

        let is_capture = m.is_capture();

        let history = if m.is_capture() {
            let p = sh.pos.board.piece_at(m.from()).unwrap();
            let c = sh.pos.board.piece_at(m.to()).unwrap_or(Piece::Pawn);
            sh.history.capture.get(p, m, c)
        } else {
            sh.history.quiet.get_score(sh.pos.color(), m)
        };

        //Quiet Pruning
        if ply > 0
            && !is_capture
            && !in_check
            && !eval::is_decisive(score)
            && !sh.pos.gives_check(m)
        {
            //LMP
            if i > 3 + (depth * depth) as usize {
                continue;
            }

            //Futility Pruning
            if eval + 60 * depth <= alpha && depth < 7 && !m.typ().is_promotion() {
                if score < eval + 60 * depth {
                    score = eval + 60 * depth
                }
                continue;
            }
        }

        let mut reduction = 200 * (depth.ilog2() * move_idx.ilog2()) as i32;
        reduction += 900 * i32::from(cut_node);
        reduction += 1500 * i32::from(!is_capture);
        reduction -= 1500 * i32::from(IS_PV_NODE);
        reduction += 1000 * i32::from(ttmove.is_none());
        reduction -= history / 8;

        sh.do_move(m);

        let mut movescore = if i == 0 && IS_PV_NODE {
            -search_step::<true>(sh, depth - 1, ply + 1, -beta, -alpha, false)
        //Apply lmr at sufficiently high depths
        } else if depth >= 2 && i > 0 {
            //lmr reduction depth

            let mut val = -search_step::<false>(
                sh,
                (depth - reduction / 1024 - 1).max(1),
                ply + 1,
                -alpha - 1,
                -alpha,
                !cut_node,
            );
            // Verify on fail high
            if val > alpha {
                val = -search_step::<false>(sh, depth - 1, ply + 1, -alpha - 1, -alpha, !cut_node);
            }
            val
        // Reduce less otherwise
        } else {
            reduction -= 5000;
            if Some(m) == ttmove {
                reduction -= 1000;
            }

            -search_step::<false>(
                sh,
                depth - (reduction / 1024).clamp(0, 2) - 1,
                ply + 1,
                -alpha - 1,
                -alpha,
                !cut_node,
            )
        };

        //Research if we failed high in a PV node
        if IS_PV_NODE && i != 0 && movescore > alpha {
            movescore = -search_step::<true>(sh, depth - 1, ply + 1, -beta, -alpha, false);
        }

        sh.undo_move();

        //Abort search if the helper gets a stop signal
        if sh.shared_data().stop_flag.load(Ordering::Relaxed) {
            return eval::DRAW;
        }

        //Adjust results
        if movescore >= beta {
            bestmove = Some(m);
            score = movescore;
            bound = Bound::Lower;
            if !is_capture {
                sh.history.killer.register(m, ply);
            }
            break;
        }

        if movescore > score {
            bestmove = Some(m);
            score = movescore;
            if score > alpha {
                bound = Bound::Exact;
                alpha = score;
                //break search if result is already optimal
                if alpha == eval::mate_in(ply + 1) {
                    break;
                }
            }
        }
    }

    if move_idx == 0 {
        if in_check {
            return -eval::mate_in(ply);
        } else {
            return eval::DRAW;
        }
    }

    assert!(bestmove.is_some());
    assert!(-eval::INFTY < score && score < eval::INFTY);

    if IS_PV_NODE && ply <= 255 && bound == Bound::Exact {
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

    if let Some(m) = bestmove
        && !m.is_capture()
    {
        //Update Quiet Histories
        sh.update_continuation_history(m, quiet_bonus);
        sh.update_quiet_history(m, cont_bonus);
    } else if let Some(m) = bestmove {
        sh.update_capture_history(m, cap_bonus);
    }

    for m in move_picker.searched_moves() {
        if Some(*m) == bestmove {
            continue;
        }
        if !m.is_capture() && bestmove.is_some_and(|bm| !bm.is_capture()) {
            // Only update quiet history if the best move is quiet
            sh.update_quiet_history(*m, -quiet_malus);
            sh.update_continuation_history(*m, -cont_malus);
        } else if m.is_capture() {
            sh.update_capture_history(*m, -cap_malus);
        }
    }

    let zh = sh.pos().zobrist_hash();

    if !(in_check
        || bestmove.is_some_and(|m| m.is_capture())
        || (bound == Bound::Upper && score > static_eval)
        || (bound == Bound::Lower && score < static_eval))
    {
        let pawn_hash = sh.pos.pawn_hash();
        let diff = score - static_eval;
        let bonus = (diff * depth).clamp(-2047, 2047);
        sh.history.correction.register(sh.pos.color(), pawn_hash, bonus);
    }

    //Write to TT
    sh.shared.tt.set(
        zh,
        score,
        bound,
        bestmove.unwrap(),
        depth.clamp(0, 255) as u8,
        ply,
    );

    score
}

fn quiesce(sh: &mut SearchHead, mut alpha: i32, beta: i32, delta: i32, ply: usize) -> i32 {
    if sh.shared.nodes.load(Ordering::Relaxed) & 0xff == 0 && sh.limit.should_stop(sh.start_time) {
        sh.shared.stop_flag.store(true, Ordering::Release);
        return eval::DRAW;
    }

    sh.sel_depth = sh.sel_depth.max(ply);

    //check for obviously drawn positions
    if is_material_draw(sh.pos()) {
        return eval::DRAW;
    }

    //50 move rule
    if sh.pos.rule_50_count() >= 100 {
        return eval::DRAW;
    }

    //Check if the move is already hashed
    let hash_entry = sh.shared_data().tt.get(sh.pos().zobrist_hash());

    let ttmove = hash_entry.map(|e| e.mov()).take_if(|m| m.is_capture());
    let tteval = hash_entry.map(|e| e.eval(ply));
    let ttbound = hash_entry.map(|e| e.bound());

    if let Some(eval) = tteval
        && !eval::is_decisive(eval)
        && ((ttbound == Some(Bound::Upper) && eval < alpha)
            || (ttbound == Some(Bound::Lower) && eval >= beta)
            || ttbound == Some(Bound::Exact))
    {
        return eval;
    }

    let static_eval = sh.evaluate();
    let in_check = sh.pos_mut().in_check();

    //If we are not in check we filter for tactical moves.
    if !in_check {
        //Adjust based on null-move hypothesis
        if static_eval >= beta {
            return static_eval;
        } else if alpha < static_eval {
            alpha = static_eval;
        }
    }

    let val = alpha - static_eval - delta;

    let cutoff = if in_check { None } else { Some(val) };

    let mut move_picker = MovePicker::new(sh, 255, ttmove, cutoff);

    let mut best_score = if in_check { -eval::INFTY } else { static_eval };

    alpha = alpha.max(best_score);

    let mut move_idx = 0;

    while let Some(m) = move_picker.next(sh) {
        move_idx += 1;
        let i = move_idx - 1;
        //stop if we receive the flag is set;
        if sh
            .shared_data()
            .stop_flag
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return eval::DRAW;
        }

        if i >= 3 && !in_check && !eval::is_loss(alpha) && !sh.pos.gives_check(m) {
            continue;
        }

        sh.do_move(m);
        //The deeper we are the more valuable captures need to be
        let score = -quiesce(sh, -beta, -alpha, delta - 50, ply + 1);
        sh.undo_move();

        if score >= beta {
            return score;
        } else if best_score < score {
            best_score = score;
            if best_score > alpha {
                alpha = best_score;
            }
        }
    }

    if in_check && move_idx == 0 {
        return -eval::mate_in(ply);
    }

    best_score
}
