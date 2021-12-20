use arrayvec::ArrayVec;
use nnue::features::{Feature, EnumerateFeatures, MoveFeatures, Perspective};
use crate::chess::{Move, MoveType, Position, SquareMethods, SquareIndex, SquareIndexMethods, Piece, Color};


pub struct HalfKAv2Feature {
    pub kpos: SquareIndex,
    pub piece: Piece,
    pub same_color: bool,
    pub square: SquareIndex,
}

impl HalfKAv2Feature {
    pub fn new(kpos: SquareIndex, piece: Piece, square: SquareIndex, same_color: bool) -> HalfKAv2Feature {
        HalfKAv2Feature {kpos, piece, same_color, square}
    }
}

impl Feature for HalfKAv2Feature {
    fn index(&self) -> usize {
        let index_base = [5,4,2,1,3,0];
        let mut idx = 2 * index_base[self.piece as usize] + if self.same_color {0} else {1};
        if idx == 11 {
            idx -= 1;
        }
        self.kpos.index() * 64 * 11 + idx * 64 + self.square.index()
    }
}


impl MoveFeatures<HalfKAv2Feature> for Move {
    #[inline]
    fn changed_features(&self, perspective: Perspective, mut ksq: SquareIndex, our_piece: bool)
                                                        -> (Vec<HalfKAv2Feature>, Vec<HalfKAv2Feature>) {
        let flip = if perspective == Perspective::WHITE {0} else {56};
        let f = self.from ^ flip;
        let t = self.to ^ flip;
        ksq = ksq ^ flip;

        //select updated features
        match self.typ {
            MoveType::CAPTURE(p) => {
                (vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                      HalfKAv2Feature::new(ksq, p, t, !our_piece)])
            },
            MoveType::PROMOTION(p) => {
                (vec![HalfKAv2Feature::new(ksq, p, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece)])
            },
            MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => {
                    (vec![HalfKAv2Feature::new(ksq, p_prom, t, our_piece)],
                     vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                          HalfKAv2Feature::new(ksq, p_cap, t, !our_piece)])
            },
            MoveType::ENPASSANT => {
                let cap_square = if our_piece {t - 8} else {t + 8};
                (vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                      HalfKAv2Feature::new(ksq, Piece::PAWN, cap_square, !our_piece)])
            },
            MoveType::CASTLE => {
                let (rf,rt) = if t == 58 {
                    (56,59)
                } else {
                    (63,61)
                };
                (vec![HalfKAv2Feature::new(ksq, Piece::KING, t, false),
                      HalfKAv2Feature::new(ksq, Piece::ROOK, rt, false)],
                 vec![HalfKAv2Feature::new(ksq, Piece::KING, f, false),
                      HalfKAv2Feature::new(ksq, Piece::ROOK, rf, false)])
            }
            _ => {
                (vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece)])
            }
        }
    }
}

impl EnumerateFeatures<HalfKAv2Feature> for Position {
    #[inline]
    fn features(&self, p: Perspective) -> ArrayVec<HalfKAv2Feature, 32> {
        let mut features = ArrayVec::new();
        let flip = if p == Perspective::WHITE {0} else {56};
        let ksq = SquareIndex::from_square(self.board[(p.into(), Piece::KING)]);
        let c: Color = p.into();
        for (p,c_p,sq) in self.board.iter() {
            features.push(HalfKAv2Feature::new(ksq ^ flip, p, sq as u8 ^ flip, c == c_p));
        }
        features
    }
}

impl Into<Color> for Perspective {
    fn into(self) -> Color {
        match self {
            Perspective::WHITE => Color::WHITE,
            Perspective::BLACK => Color::BLACK,
        }
    }
}

impl Into<Perspective> for Color {
    fn into(self) -> Perspective {
        match self {
            Color::WHITE => Perspective::WHITE,
            Color::BLACK => Perspective::BLACK,
        }
    }
}
