use super::bitboard::Square;
use super::board::Board;
use crate::chess::{Color, Piece};
use crate::constants;
use std::fmt;
use std::hash::{Hash, Hasher};

pub type MoveList = arrayvec::ArrayVec<Move, 254>;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum MoveType {
    Normal,
    Capture,
    PromotionN,
    PromotionB,
    PromotionR,
    PromotionQ,
    PromotionCaptureN,
    PromotionCaptureB,
    PromotionCaptureR,
    PromotionCaptureQ,
    Enpassant,
    Castle,
}

impl MoveType {
    pub fn is_promotion(self) -> bool {
        matches!(self, Self::PromotionN | Self::PromotionB | Self::PromotionR | Self::PromotionQ | Self::PromotionCaptureN | Self::PromotionCaptureB | Self::PromotionCaptureR | Self::PromotionCaptureQ)
    }
    pub fn promotion_piece(self) -> Option<Piece> {
        match self {
            Self::PromotionN | Self::PromotionCaptureN => Some(Piece::Knight),
            Self::PromotionB | Self::PromotionCaptureB => Some(Piece::Bishop),
            Self::PromotionR | Self::PromotionCaptureR => Some(Piece::Rook),
            Self::PromotionQ | Self::PromotionCaptureQ => Some(Piece::Queen),
            _ => None,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Move {
    from: Square,
    to: Square,
    typ: MoveType,
}

impl Move {

    #[inline(always)]
    pub fn new(from: Square, to: Square, typ: MoveType) -> Move {
        Move {
            from,
            to,
            typ
        }
    }

    //read a str in the UCI format to a move
    pub fn from_str(s: &str, board: &Board) -> Move {
        let mut chars = s.chars();

        let mut from = chars.next().unwrap() as u8 - b'a';
        from += 8 * (chars.next().unwrap() as u8 - b'1');

        let mut to = chars.next().unwrap() as u8 - b'a';
        to += 8 * (chars.next().unwrap() as u8 - b'1');

        let prom = match chars.next() {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => None,
        };

        Move {
            from: from.into(),
            to: to.into(),
            typ: determine_move_type(board, from.into(), to.into(), prom),
        }
    }

    #[inline(always)]
    pub fn to(&self) -> Square {
        self.to
    }

    #[inline(always)]
    pub fn from(&self) -> Square {
        self.from
    }

    #[inline(always)]
    pub fn typ(&self) -> MoveType {
        self.typ
    }

    pub fn decompress(m: u16) -> Option<Move> {
        let from = m & 0b111111;
        let to = (m >> 6) & 0b111111;

        let typ: MoveType = unsafe { std::mem::transmute((m >> 12) as u8) };

        Some(Move {
            from: from.into(),
            to: to.into(),
            typ,
        })
    }

    pub fn compress(&self) -> u16 {
        ((self.typ as u16) << 12) ^ <Square as Into<u16>>::into(self.from) ^ (<Square as Into<u16>>::into(self.to) << 6)
    }

    //Get the zobrist number associated to the given move
    //This does not include castling and enpassant numbers, since these depend on the total state
    //of the position.
    #[inline]
    pub fn zobrist(&self, board: &Board, c: Color) -> u64 {
        match self.typ {
            MoveType::Castle => match self.to {
                Square::C1 => {
                    constants::piece_zobrist(Piece::King, Color::White, Square::E1)
                        ^ constants::piece_zobrist(Piece::King, Color::White, Square::C1)
                        ^ constants::piece_zobrist(Piece::Rook, Color::White, Square::A1)
                        ^ constants::piece_zobrist(Piece::Rook, Color::White, Square::D1)
                }
                Square::G1 => {
                    constants::piece_zobrist(Piece::King, Color::White, Square::E1)
                        ^ constants::piece_zobrist(Piece::King, Color::White, Square::G1)
                        ^ constants::piece_zobrist(Piece::Rook, Color::White, Square::H1)
                        ^ constants::piece_zobrist(Piece::Rook, Color::White, Square::F1)
                }
                Square::C8 => {
                    constants::piece_zobrist(Piece::King, Color::Black, Square::E8)
                        ^ constants::piece_zobrist(Piece::King, Color::Black, Square::C8)
                        ^ constants::piece_zobrist(Piece::Rook, Color::Black, Square::A8)
                        ^ constants::piece_zobrist(Piece::Rook, Color::Black, Square::D8)
                }
                Square::G8 => {
                    constants::piece_zobrist(Piece::King, Color::Black, Square::E8)
                        ^ constants::piece_zobrist(Piece::King, Color::Black, Square::G8)
                        ^ constants::piece_zobrist(Piece::Rook, Color::Black, Square::H8)
                        ^ constants::piece_zobrist(Piece::Rook, Color::Black, Square::F8)
                }
                _ => 0,
            },

            MoveType::Enpassant => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Pawn, c, self.to)
                    ^ constants::piece_zobrist(
                        Piece::Pawn,
                        c.other(),
                        self.to.file().ep_cap_square().relative(c.other()),
                    )
            }

            MoveType::PromotionN => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Knight, c, self.to)
            }

            MoveType::PromotionB => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Bishop, c, self.to)
            }

            MoveType::PromotionR => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Rook, c, self.to)
            }

            MoveType::PromotionQ => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Queen, c, self.to)
            }

            MoveType::PromotionCaptureN => {
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Knight, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::PromotionCaptureB => {
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Bishop, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::PromotionCaptureR => {
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(Piece::Rook, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::PromotionCaptureQ => {
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(Piece::Queen, c, self.from)
                    ^ constants::piece_zobrist(Piece::Bishop, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::Capture => {
                let p = board.piece_at(self.from).unwrap();
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(p, c, self.from)
                    ^ constants::piece_zobrist(p, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::Normal => {
                let p = board.piece_at(self.from).unwrap();
                constants::piece_zobrist(p, c, self.from)
                    ^ constants::piece_zobrist(p, c, self.to)
            }
        }
    }
}


// This hashing implementation aims to enable move voting.
// We can therefore assume that there is only one unique move between two squares.
// We thus do not hash any other information.
impl Hash for Move {
    fn hash<H: Hasher>(&self, state: &mut H) {
        u8::from(self.to).hash(state);
        u8::from(self.from).hash(state);
    }
}

fn determine_move_type(
    board: &Board,
    from: Square,
    to: Square,
    promote: Option<Piece>,
) -> MoveType {
    let piece = board.piece_at(from).unwrap();
    match piece {
        Piece::King => {
            if from == Square::E1 && (to == Square::G1 || to == Square::C1)
            {
                return MoveType::Castle;
            }
            if from == Square::E8 && (to == Square::G8 || to == Square::C8)
            {
                return MoveType::Castle;
            }
        }
        Piece::Pawn => {
            if !board.occupation().is_set(to) && to.file() != from.file() {
                return MoveType::Enpassant;
            } 
            else if let Some(p) = promote {
                if board.occupation().is_set(to) {
                    return match p {
                        Piece::Knight => MoveType::PromotionCaptureN,
                        Piece::Bishop => MoveType::PromotionCaptureB,
                        Piece::Rook   => MoveType::PromotionCaptureR,
                        Piece::Queen  => MoveType::PromotionCaptureQ,
                        _             => panic!("Invalid promotion.")
                    };
                } else {
                    return match p {
                        Piece::Knight => MoveType::PromotionN,
                        Piece::Bishop => MoveType::PromotionB,
                        Piece::Rook   => MoveType::PromotionR,
                        Piece::Queen  => MoveType::PromotionQ,
                        _             => panic!("Invalid promotion.")
                    };
                }
            }
        }
        _ => {}
    }

    if board.occupation().is_set(to) {
        MoveType::Capture
    } else {
        MoveType::Normal
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", self.from, self.to, display_promotion(self))
    }
}

fn display_promotion(m: &Move) -> String {
    match m.typ {
        MoveType::PromotionQ => "q".to_string(),
        MoveType::PromotionR => "r".to_string(),
        MoveType::PromotionB => "b".to_string(),
        MoveType::PromotionN => "n".to_string(),
        MoveType::PromotionCaptureQ => "q".to_string(),
        MoveType::PromotionCaptureR => "r".to_string(),
        MoveType::PromotionCaptureB => "b".to_string(),
        MoveType::PromotionCaptureN => "n".to_string(),
        _ => "".to_string(),
    }
}


#[inline]
fn u32_to_piece(p: u32) -> Piece {
    match p {
        0 => Piece::King,
        1 => Piece::Queen,
        2 => Piece::Bishop,
        3 => Piece::Knight,
        4 => Piece::Rook,
        5 => Piece::Pawn,
        _ => panic!("Tried to decompress invalid move."),
    }
}
