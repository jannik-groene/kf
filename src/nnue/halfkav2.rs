use crate::chess::{ep_cap_square, Color, Move, MoveType, Piece, Position, Square};
use arrayvec::ArrayVec;
use nnue::features::{EnumerateFeatures, Feature, MoveFeatures, Perspective};

pub struct HalfKAv2Feature {
    pub kpos: Square,
    pub piece: Piece,
    pub same_color: bool,
    pub square: Square,
}

impl HalfKAv2Feature {
    pub fn new(kpos: Square, piece: Piece, square: Square, same_color: bool) -> HalfKAv2Feature {
        HalfKAv2Feature {
            kpos,
            piece,
            same_color,
            square,
        }
    }
}

impl Feature for HalfKAv2Feature {
    fn index(&self) -> usize {
        let index_base = [5, 4, 2, 1, 3, 0];
        let mut idx = 2 * index_base[self.piece as usize] + if self.same_color { 0 } else { 1 };
        if idx == 11 {
            idx -= 1;
        }
        self.kpos as usize * 64 * 11 + idx * 64 + self.square as usize
    }
}

impl MoveFeatures<HalfKAv2Feature> for Move {
    #[inline]
    fn changed_features(
        &self,
        perspective: Perspective,
        ksq: u8,
        our_piece: bool,
    ) -> (Vec<HalfKAv2Feature>, Vec<HalfKAv2Feature>) {
        let f = self.from.relative(perspective.into());
        let t = self.to.relative(perspective.into());
        let ksq = Square::from(ksq).relative(perspective.into());

        //select updated features
        match self.typ {
            MoveType::Capture(p) => (
                vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                vec![
                    HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                    HalfKAv2Feature::new(ksq, p, t, !our_piece),
                ],
            ),
            MoveType::Promotion(p) => (
                vec![HalfKAv2Feature::new(ksq, p, t, our_piece)],
                vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece)],
            ),
            MoveType::PromotionCapture((p_prom, p_cap)) => (
                vec![HalfKAv2Feature::new(ksq, p_prom, t, our_piece)],
                vec![
                    HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                    HalfKAv2Feature::new(ksq, p_cap, t, !our_piece),
                ],
            ),
            MoveType::Enpassant => {
                let cap_square = if our_piece {
                    ep_cap_square(t.file())
                } else {
                    ep_cap_square(t.file()).flipped()
                };
                (
                    vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                    vec![
                        HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                        HalfKAv2Feature::new(ksq, Piece::Pawn, cap_square, !our_piece),
                    ],
                )
            }
            MoveType::Castle => {
                let (rf, rt) = if t == Square::C8 {
                    (Square::A8, Square::D8)
                } else {
                    (Square::H8, Square::F8)
                };
                (
                    vec![
                        HalfKAv2Feature::new(ksq, Piece::King, t, false),
                        HalfKAv2Feature::new(ksq, Piece::Rook, rt, false),
                    ],
                    vec![
                        HalfKAv2Feature::new(ksq, Piece::King, f, false),
                        HalfKAv2Feature::new(ksq, Piece::Rook, rf, false),
                    ],
                )
            }
            _ => (
                vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece)],
            ),
        }
    }
}

impl EnumerateFeatures<HalfKAv2Feature> for Position {
    #[inline]
    fn features(&self, p: Perspective) -> ArrayVec<HalfKAv2Feature, 32> {
        let mut features = ArrayVec::new();
        let ksq = self.board.get_bb(p.into(), Piece::King).least_square();
        let c: Color = p.into();
        let pieces = [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ];
        //generate piece indices in order, for faster computations
        for piece in pieces {
            for sq in self.board.get_bb(c, piece) {
                features.push(HalfKAv2Feature::new(
                    ksq.relative(p.into()),
                    piece,
                    sq.relative(p.into()),
                    true,
                ));
            }
            for sq in self.board.get_bb(c.other(), piece) {
                features.push(HalfKAv2Feature::new(
                    ksq.relative(p.into()),
                    piece,
                    sq.relative(p.into()),
                    false,
                ));
            }
        }
        features
    }
}

impl From<Perspective> for Color {
    fn from(p: Perspective) -> Color {
        match p {
            Perspective::WHITE => Color::White,
            Perspective::BLACK => Color::Black,
        }
    }
}

impl From<Color> for Perspective {
    fn from(c: Color) -> Perspective {
        match c {
            Color::White => Perspective::WHITE,
            Color::Black => Perspective::BLACK,
        }
    }
}
