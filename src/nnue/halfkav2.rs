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
        ksq ^= flip;

        //select updated features
        match self.typ {
            MoveType::Capture(p) => {
                (vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                      HalfKAv2Feature::new(ksq, p, t, !our_piece)])
            },
            MoveType::Promotion(p) => {
                (vec![HalfKAv2Feature::new(ksq, p, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece)])
            },
            MoveType::PromotionCapture((p_prom,p_cap)) => {
                    (vec![HalfKAv2Feature::new(ksq, p_prom, t, our_piece)],
                     vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                          HalfKAv2Feature::new(ksq, p_cap, t, !our_piece)])
            },
            MoveType::Enpassant => {
                let cap_square = if our_piece {t - 8} else {t + 8};
                (vec![HalfKAv2Feature::new(ksq, self.piece, t, our_piece)],
                 vec![HalfKAv2Feature::new(ksq, self.piece, f, our_piece),
                      HalfKAv2Feature::new(ksq, Piece::Pawn, cap_square, !our_piece)])
            },
            MoveType::Castle => {
                let (rf,rt) = if t == 58 {
                    (56,59)
                } else {
                    (63,61)
                };
                (vec![HalfKAv2Feature::new(ksq, Piece::King, t, false),
                      HalfKAv2Feature::new(ksq, Piece::Rook, rt, false)],
                 vec![HalfKAv2Feature::new(ksq, Piece::King, f, false),
                      HalfKAv2Feature::new(ksq, Piece::Rook, rf, false)])
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
        let ksq = SquareIndex::from_square(self.board[(p.into(), Piece::King)]);
        let c: Color = p.into();
        let pieces = [Piece::Pawn, Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen, Piece::King];
        //generate piece indices in order, for faster computations
        for p in pieces {
            for sq in self.board[(c,p)].iter() {
                features.push(HalfKAv2Feature::new(ksq ^ flip, p, SquareIndex::from_square(sq) ^ flip, true));
            }
            for sq in self.board[(c.other(),p)].iter() {
                features.push(HalfKAv2Feature::new(ksq ^ flip, p, SquareIndex::from_square(sq) ^ flip, false));
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
