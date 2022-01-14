use crate::chess::{Move, MoveList, MoveType, Piece, Position};
use crate::constants::piece_value;

use std::iter::Iterator;

enum MovePickingStage {
    TtMove,
    WinningCaptures,
    EvenCaptures,
    Killers,
    Quiets,
    LosingCaptures,
    Finished,
}

//Threshold after which a trade is considered winning in SEE
const WINNING_THRESHOLD: i32 = piece_value(Piece::Pawn);
//Threshold after which a trade is considered even in SEE
const EVEN_THRESHOLD: i32 = piece_value(Piece::Knight) - piece_value(Piece::Bishop);

pub struct MovePicker {
    moves: MoveList,
    scores: Vec<i32>,
    ttmove: Option<Move>,
    killers: [Option<Move>; 2],
    stage: MovePickingStage,
    idx: usize,
}

impl MovePicker {
    pub fn new(pos: &mut Position, killers: [Option<Move>; 2], ttmove: Option<Move>) -> Self {
        let moves = pos.get_moves();
        let scores = Self::score_captures(&moves, pos);

        MovePicker {
            moves,
            scores,
            ttmove,
            killers,
            stage: MovePickingStage::TtMove,
            idx: 0,
        }
    }

    #[inline]
    fn score_captures(movelist: &MoveList, pos: &Position) -> Vec<i32> {
        let mut scores = vec![0; movelist.len()];

        for (i, m) in movelist.iter().enumerate() {
            if is_capture(m) {
                scores[i] = pos.see(*m);
            }
        }

        scores
    }
}

impl Iterator for MovePicker {
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
            MovePickingStage::WinningCaptures | MovePickingStage::EvenCaptures => {
                if let Some((i, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .filter(|(i, m)| is_capture(m) && self.scores[*i] >= EVEN_THRESHOLD)
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
                if let Some((idx, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .find(|&(_, m)| Some(*m) == self.killers[0])
                {
                    self.moves.swap(idx, self.idx);
                    self.scores.swap(idx, self.idx);
                    self.idx += 1;
                    self.killers[0]
                } else if let Some((idx, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .find(|&(_, m)| Some(*m) == self.killers[1])
                {
                    self.moves.swap(idx, self.idx);
                    self.scores.swap(idx, self.idx);
                    self.idx += 1;
                    self.killers[1]
                } else {
                    self.stage = MovePickingStage::Quiets;
                    self.next()
                }
            }
            MovePickingStage::Quiets => {
                if let Some((idx, _)) = self
                    .moves
                    .iter()
                    .enumerate()
                    .skip(self.idx)
                    .find(|(_, m)| !is_capture(m))
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
            MovePickingStage::Finished => {
                None
            }
        }
    }
}

#[inline]
fn is_capture(m: &Move) -> bool {
    matches!(
        m.typ,
        MoveType::Capture(_) | MoveType::PromotionCapture(_) | MoveType::Enpassant
    )
}

#[cfg(test)]
mod tests {
    use crate::chess::Position;

    fn perft_step(pos: &mut Position, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut count = 0;
        let picker = super::MovePicker::new(pos, [None, None], None);
        for m in picker {
            pos.do_move(m);
            count += perft_step(pos, depth - 1);
            pos.undo_move();
        }
        count
    }

    #[test]
    fn perft_movepicker() {
        let mut positions = vec![
            Position::new(),
            Position::from_fen(String::from(
                "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0",
            ))
            .unwrap(),
            Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 0"))
                .unwrap(),
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
