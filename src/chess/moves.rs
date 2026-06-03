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
    Promotion(Piece),
    PromotionCapture(Piece),
    Enpassant,
    Castle,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Move {
    pub piece: Piece,
    pub from: Square,
    pub to: Square,
    pub typ: MoveType,
}

impl Move {
    //read a str in the UCI format to a move
    pub fn from_str(s: &str, board: &Board) -> Move {
        let mut chars = s.chars();

        let mut from = chars.next().unwrap() as u8 - b'a';
        from += 8 * (chars.next().unwrap() as u8 - b'1');

        let mut to = chars.next().unwrap() as u8 - b'a';
        to += 8 * (chars.next().unwrap() as u8 - b'1');

        let piece = board.piece_at(from.into()).unwrap();

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
            piece,
            typ: determine_move_type(board, from.into(), to.into(), piece, prom),
        }
    }

    pub fn decompress(m: u32) -> Option<Move> {
        let from = m & 0b111111;
        let to = (m >> 6) & 0b111111;
        let piece_and_type = m >> 12;

        let piece = u32_to_piece(piece_and_type & 0b111);

        let typ = match piece_and_type >> 9 {
            0 => MoveType::Normal,
            1 => MoveType::Capture,
            2 => MoveType::Promotion(u32_to_piece((piece_and_type >> 3) & 0b111)),
            3 => MoveType::PromotionCapture(u32_to_piece((piece_and_type >> 3) & 0b111)),
            4 => MoveType::Castle,
            5 => MoveType::Enpassant,
            _ => return None,
        };

        Some(Move {
            from: from.into(),
            to: to.into(),
            piece,
            typ,
        })
    }

    pub fn compress(&self) -> u32 {
        let mut piece_and_type = self.piece as u32;

        piece_and_type |= match self.typ {
            MoveType::Normal => 0,
            MoveType::Capture => 1 << 9,
            MoveType::Promotion(p) => ((p as u32) << 3) | (2 << 9),
            MoveType::PromotionCapture(p) => ((p as u32) << 3) | (3 << 9),
            MoveType::Castle => 4 << 9,
            MoveType::Enpassant => 5 << 9,
        };

        (piece_and_type << 12) ^ <Square as Into<u32>>::into(self.from) ^ (<Square as Into<u32>>::into(self.to) << 6)
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

            MoveType::Promotion(p) => {
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(p, c, self.to)
            }

            MoveType::PromotionCapture(p) => {
                let q = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(Piece::Pawn, c, self.from)
                    ^ constants::piece_zobrist(p, c, self.to)
                    ^ constants::piece_zobrist(q, c.other(), self.to)
            }

            MoveType::Capture => {
                let p = board.piece_at(self.to).unwrap();
                constants::piece_zobrist(self.piece, c, self.from)
                    ^ constants::piece_zobrist(self.piece, c, self.to)
                    ^ constants::piece_zobrist(p, c.other(), self.to)
            }

            MoveType::Normal => {
                constants::piece_zobrist(self.piece, c, self.from)
                    ^ constants::piece_zobrist(self.piece, c, self.to)
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
    piece: Piece,
    promote: Option<Piece>,
) -> MoveType {
    match piece {
        Piece::King => {
            if piece == Piece::King && from == Square::E1 && (to == Square::G1 || to == Square::C1)
            {
                return MoveType::Castle;
            }
            if piece == Piece::King && from == Square::E8 && (to == Square::G8 || to == Square::C8)
            {
                return MoveType::Castle;
            }
        }
        Piece::Pawn => {
            if !board.occupation().is_set(to) && to.file() != from.file() {
                return MoveType::Enpassant;
            } else if promote.is_some() {
                if board.occupation().is_set(to) {
                    return MoveType::PromotionCapture(promote.unwrap());
                } else {
                    return MoveType::Promotion(promote.unwrap());
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
        MoveType::Promotion(Piece::Queen) => "q".to_string(),
        MoveType::Promotion(Piece::Rook) => "r".to_string(),
        MoveType::Promotion(Piece::Bishop) => "b".to_string(),
        MoveType::Promotion(Piece::Knight) => "n".to_string(),
        MoveType::PromotionCapture(Piece::Queen) => "q".to_string(),
        MoveType::PromotionCapture(Piece::Rook) => "r".to_string(),
        MoveType::PromotionCapture(Piece::Bishop) => "b".to_string(),
        MoveType::PromotionCapture(Piece::Knight) => "n".to_string(),
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
