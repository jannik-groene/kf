use crate::chess::{Move, MoveList, Piece, Position};
use crate::constants::piece_value;

use std::iter::Iterator;

use super::history::History;

enum MovePickingStage {
    TtMove,
    WinningCaptures,
    Killers,
    Quiets,
    LosingCaptures,
    Finished,
}

//Threshold after which a trade is considered winning in SEE
//const WINNING_THRESHOLD: i32 = piece_value(Piece::Pawn);
//Threshold after which a trade is considered even in SEE
const EVEN_THRESHOLD: i32 = piece_value(Piece::Knight) - piece_value(Piece::Bishop);

pub struct MovePicker<'a> {
    moves: &'a mut MoveList,
    scores: Vec<i32>,
    ttmove: Option<Move>,
    killer: Move,
    stage: MovePickingStage,
    idx: usize,
}

impl<'a> MovePicker<'a> {
    pub fn from_move_list(
        moves: &'a mut MoveList,
        pos: &Position,
        d: i16,
        history: &History,
        ttmove: Option<Move>,
    ) -> Self {
        let mut scores = Self::score_captures(moves, pos, history);
        Self::score_quiets(pos, &mut scores, moves, history);
        let killer = history.killer.get(d as usize);

        MovePicker {
            moves,
            scores,
            ttmove,
            killer,
            stage: MovePickingStage::TtMove,
            idx: 0,
        }
    }

    #[inline]
    fn score_captures(movelist: &MoveList, pos: &Position, history: &History) -> Vec<i32> {
        let mut scores = vec![0; movelist.len()];

        for (i, m) in movelist.iter().enumerate() {
            if m.is_capture() {
                let cap = pos.get_board().piece_at(m.to()).unwrap_or(Piece::Pawn);
                let p = pos.get_board().piece_at(m.from()).unwrap();
                scores[i] = pos.see(*m) + history.capture.get(p, *m, cap) / 8;
            }
        }

        scores
    }

    #[inline]
    fn score_quiets(pos: &Position, scores: &mut [i32], movelist: &MoveList, history: &History) {
        let last_move = pos.last_move();
        let last_piece = if last_move.typ().is_promotion() {
            Some(Piece::Pawn)
        } else {
            pos.get_board().piece_at(last_move.to())
        };
        for (m, s) in movelist.iter().zip(scores.iter_mut()) {
            if !m.is_capture() {
                *s += history.quiet.get_score(pos.color(), *m);
                if let Some(p) = last_piece {
                    let piece = pos.get_board().piece_at(m.from()).unwrap();
                    *s += history
                        .continuation
                        .get_score(pos.color(), p, last_move, piece, *m);
                }
                if pos.gives_check(m) {
                    *s += 20_000;
                }
            }
        }
    }
}

impl<'a> Iterator for MovePicker<'a> {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        match self.stage {
            MovePickingStage::TtMove => {
                self.stage = MovePickingStage::WinningCaptures;
                if let Some(idx) = self.moves.iter().position(|m| Some(*m) == self.ttmove) {
                    self.moves.swap(0, idx);
                    self.scores.swap(0, idx);
                    self.idx += 1;
                    self.ttmove
                } else {
                    self.next()
                }
            }
            MovePickingStage::WinningCaptures => {
                if let Some((i, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .filter(|(i, m)| m.is_capture() && self.scores[*i] >= EVEN_THRESHOLD)
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(self.idx, i);
                    self.scores.swap(self.idx, i);
                    self.idx += 1;
                    Some(self.moves[self.idx - 1])
                } else {
                    self.stage = MovePickingStage::Killers;
                    self.next()
                }
            }
            MovePickingStage::Killers => {
                self.stage = MovePickingStage::Quiets;
                if let Some(idx) = self
                    .moves
                    .iter()
                    .skip(self.idx)
                    .position(|&m| m == self.killer)
                {
                    self.moves.swap(0, idx);
                    self.scores.swap(0, idx);
                    self.idx += 1;
                    Some(self.killer)
                } else {
                    self.next()
                }
            }
            MovePickingStage::Quiets => {
                if let Some((idx, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .filter(|(_, m)| !m.is_capture())
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(idx, self.idx);
                    self.scores.swap(idx, self.idx);
                    self.idx += 1;
                    Some(self.moves[self.idx - 1])
                } else {
                    self.stage = MovePickingStage::LosingCaptures;
                    self.next()
                }
            }
            MovePickingStage::LosingCaptures => {
                if let Some((i, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .max_by_key(|(i, _)| self.scores[*i])
                {
                    self.moves.swap(self.idx, i);
                    self.scores.swap(self.idx, i);
                    self.idx += 1;
                    Some(self.moves[self.idx - 1])
                } else {
                    self.stage = MovePickingStage::Finished;
                    None
                }
            }
            MovePickingStage::Finished => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chess::Position;
    use crate::search::history::History;

    fn perft_step(pos: &mut Position, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut count = 0;
        let mut moves = pos.get_moves::<true>();
        let picker = super::MovePicker::from_move_list(&mut moves, pos, 0, &History::new(), None);
        for m in picker {
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
