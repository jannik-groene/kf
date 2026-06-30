use crate::chess::{Captures, Move, MoveList, Piece, Quiets};
use crate::constants::piece_value;

use std::iter::Iterator;

use super::thread::SearchHead;

enum MovePickingStage {
    TtMove,
    ScoreCaptures,
    WinningCaptures,
    Killers,
    ScoreQuiets,
    Quiets,
    LosingCaptures,
    Finished,
}

//Threshold after which a trade is considered winning in SEE
//const WINNING_THRESHOLD: i32 = piece_value(Piece::Pawn);
//Threshold after which a trade is considered even in SEE
const EVEN_THRESHOLD: i32 = piece_value(Piece::Knight) - piece_value(Piece::Bishop);

pub struct MovePicker {
    moves: MoveList,
    searched: MoveList,
    scores: Vec<i32>,
    ttmove: Option<Move>,
    killer: Move,
    stage: MovePickingStage,
    idx: usize,
    q_idx: usize,
    bc_idx: usize,
    cutoff: Option<i32>,
}

impl MovePicker {
    pub fn new(sh: &mut SearchHead, ply: i32, ttmove: Option<Move>, cutoff: Option<i32>) -> Self {
        let killer = if (0..255).contains(&ply) {
            sh.history.killer.get(ply as usize)
        } else {
            Move::ZERO
        };

        MovePicker {
            moves: MoveList::new(),
            searched: MoveList::new(),
            scores: Vec::new(),
            ttmove,
            killer,
            stage: MovePickingStage::TtMove,
            idx: 0,
            q_idx: 0,
            bc_idx: 0,
            cutoff,
        }
    }

    #[inline]
    fn score_captures(&mut self, sh: &mut SearchHead) {
        sh.pos.get_moves::<Captures>(&mut self.moves);

        // Set index at which quiets will be inserted
        self.q_idx = self.moves.len();

        if let Some(idx) = self.moves.iter().position(|m| Some(*m) == self.ttmove) {
            self.moves.swap(0, idx);
            self.idx += 1;
        }

        let mut scores = vec![0; self.moves.len()];

        for (i, m) in self.moves.iter().enumerate() {
            if m.is_capture() {
                let cap = sh.pos.get_board().piece_at(m.to()).unwrap_or(Piece::Pawn);
                let p = sh.pos.get_board().piece_at(m.from()).unwrap();
                scores[i] = sh.pos.see(*m) + sh.history.capture.get(p, *m, cap) / 8;
            }
        }

        self.scores = scores;
    }

    #[inline]
    fn score_quiets(&mut self, sh: &mut SearchHead) {
        sh.pos.get_moves::<Quiets>(&mut self.moves);

        // Swap ahead both TT and Killer
        if let Some(idx) = self
            .moves
            .iter()
            .skip(self.idx)
            .position(|m| Some(*m) == self.ttmove)
        {
            self.moves.swap(self.idx, idx);
            self.idx += 1;
        }
        if let Some(idx) = self
            .moves
            .iter()
            .skip(self.idx)
            .position(|m| *m == self.killer)
        {
            self.moves.swap(self.idx, idx);
            self.idx += 1;
        }

        self.scores.resize(self.moves.len(), 0);

        let last_move = sh.pos.last_move();

        let last_piece = if last_move.typ().is_promotion() {
            Some(Piece::Pawn)
        } else {
            sh.pos.get_board().piece_at(last_move.to())
        };
        for (m, s) in self.moves.iter().zip(self.scores.iter_mut()) {
            if !m.is_capture() {
                *s += sh.history.quiet.get_score(sh.pos.color(), *m);
                if let Some(p) = last_piece {
                    let piece = sh.pos.get_board().piece_at(m.from()).unwrap();
                    *s +=
                        sh.history
                            .continuation
                            .get_score(sh.pos.color(), p, last_move, piece, *m);
                }
                if sh.pos.gives_check(*m) {
                    *s += 20_000;
                }
            }
        }
    }

    pub fn next(&mut self, sh: &mut SearchHead) -> Option<Move> {
        match self.stage {
            MovePickingStage::TtMove => {
                self.stage = MovePickingStage::ScoreCaptures;
                if self.ttmove.is_some_and(|m| sh.pos.is_legal(m)) {
                    unsafe { self.searched.push_unchecked(self.ttmove.unwrap()); }
                    self.ttmove
                } else {
                    self.next(sh)
                }
            }
            MovePickingStage::ScoreCaptures => {
                self.score_captures(sh);
                self.stage = MovePickingStage::WinningCaptures;
                self.next(sh)
            }
            MovePickingStage::WinningCaptures => {
                if let Some((i, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .filter(|&(i, _)| {
                        //    m.is_capture()
                        self.scores[i] >= EVEN_THRESHOLD + self.cutoff.unwrap_or(0)
                    })
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(self.idx, i);
                    self.scores.swap(self.idx, i);
                    self.idx += 1;
                    unsafe { self.searched.push_unchecked(self.moves[self.idx-1]); }
                    Some(self.moves[self.idx - 1])
                } else {
                    if self.cutoff.is_some() {
                        return None;
                    }
                    self.bc_idx = self.idx;
                    self.idx = self.q_idx;
                    self.stage = MovePickingStage::Killers;
                    self.next(sh)
                }
            }
            MovePickingStage::Killers => {
                self.stage = MovePickingStage::ScoreQuiets;
                if self.killer != Move::ZERO
                    && sh.pos.is_legal(self.killer)
                    && Some(self.killer) != self.ttmove
                {
                    unsafe { self.searched.push_unchecked(self.killer); }
                    Some(self.killer)
                } else {
                    self.next(sh)
                }
            }
            MovePickingStage::ScoreQuiets => {
                self.score_quiets(sh);
                self.stage = MovePickingStage::Quiets;
                self.next(sh)
            }
            MovePickingStage::Quiets => {
                if let Some((idx, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    //.filter(|(_, m)| !m.is_capture())
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(idx, self.idx);
                    self.scores.swap(idx, self.idx);
                    self.idx += 1;
                    unsafe { self.searched.push_unchecked(self.moves[self.idx-1]); }
                    Some(self.moves[self.idx - 1])
                } else {
                    self.idx = self.bc_idx;
                    self.stage = MovePickingStage::LosingCaptures;
                    self.next(sh)
                }
            }
            MovePickingStage::LosingCaptures => {
                if let Some((i, _)) = self.moves[..self.q_idx]
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(self.idx, i);
                    self.scores.swap(self.idx, i);
                    self.idx += 1;
                    unsafe { self.searched.push_unchecked(self.moves[self.idx-1]); }
                    Some(self.moves[self.idx - 1])
                } else {
                    self.stage = MovePickingStage::Finished;
                    None
                }
            }
            MovePickingStage::Finished => None,
        }
    }

    pub fn searched_moves(&self) -> &[Move] {
        &self.searched
    }
}

#[cfg(test)]
mod tests {
    use super::super::thread::SearchHead;
    use crate::chess::Position;
    use crate::search::thread::SharedData;
    use std::sync::Arc;
    use std::time::Instant;

    fn perft_step(pos: &mut Position, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut count = 0;
        let mut sh = SearchHead::new(
            pos.clone(),
            Arc::new(SharedData::new()),
            crate::search::thread::TimeManager {
                start_time: Instant::now(),
                limit: None,
            },
        );
        let mut picker = super::MovePicker::new(&mut sh, 0, None, None);
        while let Some(m) = picker.next(&mut sh) {
            pos.do_move(m);
            count += perft_step(pos, depth - 1);
            pos.undo_move();
        }
        count
    }

    #[test]
    fn perft_movepicker() {
        let mut positions = [
            Position::new(),
            Position::from_fen(String::from(
                "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0",
            ))
            .unwrap(),
            Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 0")).unwrap(),
            Position::from_fen(String::from(
                "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            ))
            .unwrap(),
            Position::from_fen(String::from(
                "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            ))
            .unwrap(),
            Position::from_fen(String::from(
                "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            ))
            .unwrap(),
        ];
        let results: Vec<[u64; 7]> = vec![
            [1, 20, 400, 8902, 197281, 4865609, 119060324],
            [1, 48, 2039, 97862, 4085603, 193690690, 8031647685],
            [1, 14, 191, 2812, 43238, 674624, 11030083],
            [1, 6, 264, 9467, 422333, 15833292, 706045033],
            [1, 44, 1486, 62379, 2103487, 89941194, 3048196529],
            [1, 46, 2079, 89890, 3894594, 164075551, 6923051137],
        ];
        //Choose a test depth between 1 and 6, depth 6 takes about 40 minutes
        let depth: usize = 4;
        for (pos, res) in positions.iter_mut().zip(results.iter()) {
            assert_eq!(perft_step(pos, depth), res[depth]);
        }
    }
}
