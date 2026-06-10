use super::bitboard::{BitBoard, Square};
use crate::chess::{Color, Move, MoveType, Piece};
use crate::constants::piece_zobrist;
use std::fmt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Board {
    piece_boards: [BitBoard; 6],
    color_boards: [BitBoard; 2],
    piece_table:  [Option<Piece>; 64],
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

    pub fn new() -> Board {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap()
    }
    pub fn from_fen(s: &str) -> Option<Board> {
        let mut board = Board::empty();
        let mut x = 0;
        let mut y = 7;
        let mut chrs = s.trim().chars();
        let mut c = chrs.next();
        while c.is_some() && (y > 0 || x < 8) {
            match c.unwrap() {
                '1'..='8' => x += c.unwrap().to_digit(10).unwrap(),
                '/' => {
                    x -= 8;
                    y -= 1;
                }
                _ => {
                    let typ = fen_to_type(c.unwrap());
                    match typ {
                        Some(t) => board.set(Square::from(x + 8 * y), t.0, t.1),
                        None => return None,
                    }
                    x += 1;
                }
            }
            c = chrs.next();
        }
        Some(board)
    }

    pub fn zobrist(&self) -> u64 {
        let mut zobrist: u64 = 0;
        let pieces = [
            Piece::King,
            Piece::Queen,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
            Piece::Pawn,
        ];
        for piece in pieces {
            for sq in self.get_bb(Color::White, piece) {
                zobrist ^= piece_zobrist(piece, Color::White, sq);
            }
            for sq in self.get_bb(Color::Black, piece) {
                zobrist ^= piece_zobrist(piece, Color::Black, sq);
            }
        }
        zobrist
    }

    pub fn empty() -> Board {
        Board {
            piece_boards: [BitBoard::EMPTY; 6],
            color_boards: [BitBoard::EMPTY; 2],
            piece_table:  [None; 64],
        }
    }
    #[inline]
    pub fn set(&mut self, sq: Square, c: Color, p: Piece) {
        self.color_boards[c as usize].set(sq);
        self.piece_boards[p as usize].set(sq);
        self.piece_table[sq as usize] = Some(p);
    }
    #[inline]
    pub fn unset(&mut self, sq: Square, c: Color) {
        if let Some(p) = self.piece_table[sq as usize] {
            self.color_boards[c as usize].unset(sq);
            self.piece_boards[p as usize].unset(sq);
            self.piece_table[sq as usize] = None;
        }
    }
    #[inline]
    pub fn unset_piece(&mut self, sq: Square, c: Color, p: Piece) {
        self.color_boards[c as usize].unset(sq);
        self.piece_boards[p as usize].unset(sq);
        self.piece_table[sq as usize] = None;
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
        self.piece_table[sq as usize]
    }
    #[inline]
    pub fn king_square(&self, c: Color) -> Square {
        self.get_bb(c, Piece::King).least_square()
    }
    #[inline]
    fn move_castling_rooks(&mut self, ksq: Square) {
        match ksq {
            Square::C1 => {
                self.unset_piece(Square::A1, Color::White, Piece::Rook);
                self.set(Square::D1, Color::White, Piece::Rook);
            }
            Square::G1 => {
                self.unset_piece(Square::H1, Color::White, Piece::Rook);
                self.set(Square::F1, Color::White, Piece::Rook);
            }
            Square::C8 => {
                self.unset_piece(Square::A8, Color::Black, Piece::Rook);
                self.set(Square::D8, Color::Black, Piece::Rook);
            }
            Square::G8 => {
                self.unset_piece(Square::H8, Color::Black, Piece::Rook);
                self.set(Square::F8, Color::Black, Piece::Rook);
            }
            _ => panic!("Invalid castling attempt."),
        }
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) {
        let (us, them) = if self.color_boards[0].is_set(m.from()) {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::White)
        };
        let piece = self.piece_at(m.from()).unwrap();
        let our_piece = if let Some(p) = m.typ().promotion_piece() {p} else {piece};
        self.unset(m.from(), us);
        match m.typ() {
            MoveType::Capture | MoveType::PromotionCaptureN 
                              | MoveType::PromotionCaptureB 
                              | MoveType::PromotionCaptureR 
                              | MoveType::PromotionCaptureQ => self.unset(m.to(), them),
            MoveType::Enpassant => self.unset(
                m.to().file().ep_cap_square().relative(them),
                them
            ),
            MoveType::Castle => self.move_castling_rooks(m.to()),
            _ => {}
        }
        self.set(m.to(), us, our_piece);
    }
    #[inline]
    fn undo_move_castling_rooks(&mut self, ksq: Square) {
        match ksq {
            Square::C1 => {
                self.set(Square::A1, Color::White, Piece::Rook);
                self.unset_piece(Square::D1, Color::White, Piece::Rook);
            }
            Square::G1 => {
                self.set(Square::H1, Color::White, Piece::Rook);
                self.unset_piece(Square::F1, Color::White, Piece::Rook);
            }
            Square::C8 => {
                self.set(Square::A8, Color::Black, Piece::Rook);
                self.unset_piece(Square::D8, Color::Black, Piece::Rook);
            }
            Square::G8 => {
                self.set(Square::H8, Color::Black, Piece::Rook);
                self.unset_piece(Square::F8, Color::Black, Piece::Rook);
            }
            _ => panic!("Invalid castling attempt."),
        }
    }
    #[inline]
    pub fn undo_move(&mut self, m: Move, cap: Option<Piece>) {
        let (us, them) = if self.color_boards[0].is_set(m.to()) {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::White)
        };
        let our_piece = if m.typ().is_promotion() {
            Piece::Pawn
        } else {
            self.piece_at(m.to()).unwrap()
        };
        self.set(m.from(), us, our_piece);
        self.unset(m.to(), us);
        if m.is_capture() {
            let cap_square = if m.typ() == MoveType::Enpassant {
                m.to().file().ep_cap_square().relative(them)
            } else {
                m.to()
            };
            self.set(cap_square, them, cap.unwrap());
        }
        if m.typ() == MoveType::Castle {
            self.undo_move_castling_rooks(m.to());
        }
    }
}


//translate a fen symbol (kqbnrpKQBNRP) into a (Color, Piece) pair
fn fen_to_type(c: char) -> Option<(Color, Piece)> {
    match c {
        'k' => Some((Color::Black, Piece::King)),
        'q' => Some((Color::Black, Piece::Queen)),
        'r' => Some((Color::Black, Piece::Rook)),
        'n' => Some((Color::Black, Piece::Knight)),
        'b' => Some((Color::Black, Piece::Bishop)),
        'p' => Some((Color::Black, Piece::Pawn)),
        'K' => Some((Color::White, Piece::King)),
        'Q' => Some((Color::White, Piece::Queen)),
        'R' => Some((Color::White, Piece::Rook)),
        'B' => Some((Color::White, Piece::Bishop)),
        'N' => Some((Color::White, Piece::Knight)),
        'P' => Some((Color::White, Piece::Pawn)),
        _ => None,
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
