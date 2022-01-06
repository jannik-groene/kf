use crate::bitboard::{BitBoard, Square};
use crate::chess::{ep_cap_square, Color, Move, MoveType, Piece};
use std::fmt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Board {
    piece_boards: [BitBoard; 6],
    color_boards: [BitBoard; 2],
}

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

impl Board {
    pub const BLACK_SQUARES: BitBoard =
        BitBoard::new(0b1010101001010101101010100101010110101010010101011010101001010101);
    pub const WHITE_SQUARES: BitBoard =
        BitBoard::new(0b0101010110101010010101011010101001010101101010100101010110101010);

    pub const fn new() -> Board {
        Board {
            piece_boards: [
                BitBoard::new(0x1000000000000010), //KINGS
                BitBoard::new(0x0800000000000008), //QUEENS
                BitBoard::new(0x2400000000000024), //BISHOPS
                BitBoard::new(0x4200000000000042), //KNIGHTS
                BitBoard::new(0x8100000000000081), //ROOKS
                BitBoard::new(0x00ff00000000ff00), //PAWNS
            ],
            color_boards: [
                BitBoard::new(0x000000000000ffff), //WHITE
                BitBoard::new(0xffff000000000000), //BLACK
            ],
        }
    }
    pub fn empty() -> Board {
        Board {
            piece_boards: [BitBoard::EMPTY; 6],
            color_boards: [BitBoard::EMPTY; 2],
        }
    }
    #[inline]
    pub fn set(&mut self, sq: Square, c: Color, p: Piece) {
        self.color_boards[c as usize].set(sq);
        self.piece_boards[p as usize].set(sq);
    }
    #[inline]
    pub fn unset(&mut self, sq: Square, c: Color, p: Piece) {
        self.color_boards[c as usize].unset(sq);
        self.piece_boards[p as usize].unset(sq);
    }
    #[inline]
    pub fn get_color_bb(&self, c: Color) -> BitBoard {
        self.color_boards[c as usize]
    }
    #[inline]
    pub fn get_piece_bb(&self, p: Piece) -> BitBoard {
        self.piece_boards[p as usize]
    }
    #[inline]
    pub fn get_bb(&self, c: Color, p: Piece) -> BitBoard {
        self.color_boards[c as usize] & self.piece_boards[p as usize]
    }
    #[inline]
    pub fn occupation(&self) -> BitBoard {
        self.color_boards[0] | self.color_boards[1]
    }
    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        const PIECES: [Piece; 6] = [
            Piece::King,
            Piece::Queen,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
            Piece::Pawn,
        ];
        for p in PIECES {
            if self.piece_boards[p as usize].is_set(sq) {
                return Some(p);
            }
        }
        None
    }
    #[inline]
    pub fn king_square(&self, c: Color) -> Square{
        self.get_bb(c, Piece::King).least_square()
    }
    #[inline]
    fn move_castling_rooks(&mut self, ksq: Square) {
        match ksq {
            Square::C1 => {
                self.unset(Square::A1, Color::White, Piece::Rook);
                self.set(Square::D1, Color::White, Piece::Rook);
            }
            Square::G1 => {
                self.unset(Square::H1, Color::White, Piece::Rook);
                self.set(Square::F1, Color::White, Piece::Rook);
            }
            Square::C8 => {
                self.unset(Square::A8, Color::Black, Piece::Rook);
                self.set(Square::D8, Color::Black, Piece::Rook);
            }
            Square::G8 => {
                self.unset(Square::H8, Color::Black, Piece::Rook);
                self.set(Square::F8, Color::Black, Piece::Rook);
            }
            _ => panic!("Invalid castling attempt."),
        }
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        let (us, them) = if self.color_boards[0].is_set(m.from) {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::White)
        };
        let our_piece = match m.typ {
            MoveType::Promotion(p) | MoveType::PromotionCapture((p, _)) => p,
            _ => m.piece,
        };
        self.unset(m.from, us, m.piece);
        match m.typ {
            MoveType::Capture(p) | MoveType::PromotionCapture((_, p)) => self.unset(m.to, them, p),
            MoveType::Enpassant => {
                self.unset(ep_cap_square(m.to.file()).relative(them), them, Piece::Pawn)
            }
            MoveType::Castle => self.move_castling_rooks(m.to),
            _ => {}
        }
        self.set(m.to, us, our_piece);
    }
    #[inline]
    fn undo_move_castling_rooks(&mut self, ksq: Square) {
        match ksq {
            Square::C1 => {
                self.set(Square::A1, Color::White, Piece::Rook);
                self.unset(Square::D1, Color::White, Piece::Rook);
            }
            Square::G1 => {
                self.set(Square::H1, Color::White, Piece::Rook);
                self.unset(Square::F1, Color::White, Piece::Rook);
            }
            Square::C8 => {
                self.set(Square::A8, Color::Black, Piece::Rook);
                self.unset(Square::D8, Color::Black, Piece::Rook);
            }
            Square::G8 => {
                self.set(Square::H8, Color::Black, Piece::Rook);
                self.unset(Square::F8, Color::Black, Piece::Rook);
            }
            _ => panic!("Invalid castling attempt."),
        }
    }
    #[inline]
    pub fn undo_move(&mut self, m: Move) {
        let (us, them) = if self.color_boards[0].is_set(m.to) {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::White)
        };
        let our_piece = match m.typ {
            MoveType::Promotion(p) | MoveType::PromotionCapture((p, _)) => p,
            _ => m.piece,
        };
        self.set(m.from, us, m.piece);
        self.unset(m.to, us, our_piece);
        match m.typ {
            MoveType::Capture(p) | MoveType::PromotionCapture((_, p)) => self.set(m.to, them, p),
            MoveType::Enpassant => {
                self.set(ep_cap_square(m.to.file()).relative(them), them, Piece::Pawn)
            }
            MoveType::Castle => self.undo_move_castling_rooks(m.to),
            _ => {}
        }
    }
}

fn write_piece_to_position(c: char, pos: BitBoard, cboard: &mut [[char; 8]; 8]) {
    for p in pos {
        cboard[7 - p.rank() as usize][p.file() as usize] = c;
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut board: [[char; 8]; 8] = [['.'; 8]; 8];
        write_piece_to_position('K', self.get_bb(Color::White, Piece::King), &mut board);
        write_piece_to_position('Q', self.get_bb(Color::White, Piece::Queen), &mut board);
        write_piece_to_position('R', self.get_bb(Color::White, Piece::Rook), &mut board);
        write_piece_to_position('B', self.get_bb(Color::White, Piece::Bishop), &mut board);
        write_piece_to_position('N', self.get_bb(Color::White, Piece::Knight), &mut board);
        write_piece_to_position('P', self.get_bb(Color::White, Piece::Pawn), &mut board);
        write_piece_to_position('k', self.get_bb(Color::Black, Piece::King), &mut board);
        write_piece_to_position('q', self.get_bb(Color::Black, Piece::Queen), &mut board);
        write_piece_to_position('r', self.get_bb(Color::Black, Piece::Rook), &mut board);
        write_piece_to_position('b', self.get_bb(Color::Black, Piece::Bishop), &mut board);
        write_piece_to_position('n', self.get_bb(Color::Black, Piece::Knight), &mut board);
        write_piece_to_position('p', self.get_bb(Color::Black, Piece::Pawn), &mut board);
        let line: String = board
            .iter()
            .map(|&l| -> String {
                l.iter()
                    .map(|c| -> String { String::from(*c) + " " })
                    .collect()
            })
            .map(|l| l + "\n")
            .collect();
        write!(f, "{}", line)
    }
}
