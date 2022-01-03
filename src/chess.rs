use std::fmt;
use std::iter::Iterator;

pub use crate::bitboard::{Square, Rank, File, BitBoard};

mod constants;

// Use the following bit layout (looking from standard orientation, so 0 is A1 and 63 is H()
// |----|----|----|----|----|----|----|----|
// | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63 |
// |----|----|----|----|----|----|----|----|
// | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 |
// |----|----|----|----|----|----|----|----|
// | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 |
// |----|----|----|----|----|----|----|----|
// | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 |
// |----|----|----|----|----|----|----|----|
// | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 |
// |----|----|----|----|----|----|----|----|
// | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 |
// |----|----|----|----|----|----|----|----|
// |  8 |  9 | 10 | 11 | 12 | 13 | 14 | 15 |
// |----|----|----|----|----|----|----|----|
// |  0 |  1 |  2 |  3 |  4 |  5 |  6 |  7 |
// |----|----|----|----|----|----|----|----|

#[derive(PartialEq,Clone,Copy,Debug)]
pub struct Board {
    //Layout is
    //[[White King, White Queen, White Bishop, White Knight, White Rook, White Total],
    // [Black King, Black Queen, Black Bishop, Black Knight, Black Rook, Black Total]]
    // Can be accessed as
    //          position[(Color, Piece)]
    positions: [[BitBoard; 7]; 2],
    pub occupation: BitBoard,
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

#[inline]
fn get_zobrist_table(p: Piece, c: Color) -> &'static[u64] {
    match c {
        Color::White => match p {
            Piece::King => &constants::ZOBRIST_WHITE_KING_NUMBERS,
            Piece::Queen => &constants::ZOBRIST_WHITE_QUEEN_NUMBERS,
            Piece::Bishop => &constants::ZOBRIST_WHITE_BISHOP_NUMBERS,
            Piece::Knight => &constants::ZOBRIST_WHITE_KNIGHT_NUMBERS,
            Piece::Rook => &constants::ZOBRIST_WHITE_ROOK_NUMBERS,
            Piece::Pawn => &constants::ZOBRIST_WHITE_PAWN_NUMBERS,
            Piece::Any => panic!("Invalid Piece")
        },
        Color::Black => match p {
            Piece::King => &constants::ZOBRIST_BLACK_KING_NUMBERS,
            Piece::Queen => &constants::ZOBRIST_BLACK_QUEEN_NUMBERS,
            Piece::Bishop => &constants::ZOBRIST_BLACK_BISHOP_NUMBERS,
            Piece::Knight => &constants::ZOBRIST_BLACK_KNIGHT_NUMBERS,
            Piece::Rook => &constants::ZOBRIST_BLACK_ROOK_NUMBERS,
            Piece::Pawn => &constants::ZOBRIST_BLACK_PAWN_NUMBERS,
            Piece::Any => panic!("Invalid Piece")
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
pub fn ep_cap_square(file: File) -> Square {
    match file {
        File::A => Square::A4,
        File::B => Square::B4,
        File::C => Square::C4,
        File::D => Square::D4,
        File::E => Square::E4,
        File::F => Square::F4,
        File::G => Square::G4,
        File::H => Square::H4,
    }
}

#[inline]
fn ep_square(file: File) -> Square {
    match file {
        File::A => Square::A3,
        File::B => Square::B3,
        File::C => Square::C3,
        File::D => Square::D3,
        File::E => Square::E3,
        File::F => Square::F3,
        File::G => Square::G3,
        File::H => Square::H3,
    }
}

impl Board {
    pub const BLACK_SQUARES: BitBoard = BitBoard::new(0b1010101001010101101010100101010110101010010101011010101001010101);
    pub const WHITE_SQUARES: BitBoard = BitBoard::new(0b0101010110101010010101011010101001010101101010100101010110101010);
    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        if (self[(Color::White, Piece::Pawn)] | self[(Color::Black, Piece::Pawn)]).is_set(sq) {
            Some(Piece::Pawn)
        } else if (self[(Color::White, Piece::Bishop)] | self[(Color::Black, Piece::Bishop)]).is_set(sq) {
            Some(Piece::Bishop)
        } else if (self[(Color::White, Piece::Knight)] | self[(Color::Black, Piece::Knight)]).is_set(sq) {
            Some(Piece::Knight)
        } else if (self[(Color::White, Piece::Rook)] | self[(Color::Black, Piece::Rook)]).is_set(sq) {
            Some(Piece::Rook)
        } else if (self[(Color::White, Piece::Queen)] | self[(Color::Black, Piece::Queen)]).is_set(sq) {
            Some(Piece::Queen)
        } else if (self[(Color::White, Piece::King)] | self[(Color::Black, Piece::King)]).is_set(sq) {
            Some(Piece::King)
        } else {
            None
        }
    }
    pub fn new() -> Board {
        Board{
            positions: [[1<<4, 1<<3, (1<<2)+(1<<5), (1<<1)+(1<<6), 1+(1<<7), 0b11111111<<8, 0b1111111111111111].map(BitBoard::from),
                        [1<<60, 1<<59, (1<<58)+(1<<61), (1<<57)+(1<<62), (1<<56)+(1<<63), 0b11111111<<48, 0b1111111111111111<<48].map(BitBoard::from)],
            occupation: BitBoard::from((0b1111111111111111 << 48) + 0b1111111111111111),
        }
    }
    pub fn empty() -> Board{
        Board{
            positions: [[BitBoard::EMPTY; 7],[BitBoard::EMPTY; 7]],
            occupation:  BitBoard::EMPTY,
        }
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
                '/' => {x-=8; y -= 1;},
                _ => {
                    let typ = fen_to_type(c.unwrap());
                    match typ {
                        Some(t) => {
                            board[t] |= BitBoard::new(1 << (x + 8*y));
                            board[(t.0, Piece::Any)] |= BitBoard::new(1 << (x + 8*y));
                            board.occupation |= BitBoard::new(1 << (x + 8*y));
                        },
                        None => return None,
                    }
                    x += 1;
                }
            }
            c = chrs.next();
        }
        Some(board)
    }
    //returns the zobrist xor factor after applying a move to the board
    #[inline]
    fn do_move_for_color(&mut self, m: Move, color: Color) -> u64 {
        //Move the moving Piece
        let mut zobrist = get_zobrist_table(m.piece, color)[m.from as usize];
        let from = m.from.into();
        let to = m.to.into();
        self[(color, m.piece)] ^= from;
        self[(color, Piece::Any)] ^= from;
        self.occupation ^= from;
        self.occupation ^= to;
        self[(color, Piece::Any)] ^= to;
        match m.typ {
            MoveType::Normal => {
                self[(color, m.piece)] ^= to;
                zobrist ^= get_zobrist_table(m.piece, color)[m.to as usize];
                if m.piece == Piece::Pawn && (m.from <= Square::H2 || m.from >= Square::A7) && m.to >= Square::A4 && m.to <= Square::H5 {
                    zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.file() as usize];
                }
            },
            MoveType::Capture(p) => {
                self[(color.other(),p)] ^= to;
                self[(color.other(), Piece::Any)] ^= to;
                self[(color, m.piece)] ^= to;
                self.occupation ^= to;
                zobrist ^= get_zobrist_table(m.piece, color)[m.to as usize];
                zobrist ^= get_zobrist_table(p, color.other())[m.to as usize];
            },
            MoveType::Promotion(p) => {
                self[(color, p)] ^= to;
                zobrist ^= get_zobrist_table(p, color)[m.to as usize];
            },
            MoveType::PromotionCapture((p_prom,p_cap)) => {
                self[(color.other(),p_cap)] ^= to;
                self[(color.other(), Piece::Any)] ^= to;
                self[(color, p_prom)] ^= to;
                self.occupation ^= to;
                zobrist ^= get_zobrist_table(p_prom, color)[m.to as usize];
                zobrist ^= get_zobrist_table(p_cap, color.other())[m.to as usize];
            },
            MoveType::Enpassant => {
                let cap_square = match color {
                    Color::White =>  {
                        ep_cap_square(m.to.file()).flipped()
                    }
                    Color::Black => {
                        ep_cap_square(m.to.file())
                    }
                };
                self[(color.other(),Piece::Pawn)] ^= cap_square.into();
                self[(color.other(),Piece::Any)] ^= cap_square.into();
                self.occupation ^= cap_square.into();
                self[(color, m.piece)] ^= to;
                zobrist ^= get_zobrist_table(Piece::Pawn, color)[m.to as usize];
                zobrist ^= get_zobrist_table(Piece::Pawn, color.other())[cap_square as usize];
            },
            MoveType::Castle => {
                self[(color, m.piece)] ^= to;
                zobrist ^= get_zobrist_table(Piece::King, color)[m.to as usize];
                match color {
                    Color::White => {
                        if m.to == Square::C1 {
                            self[(color, Piece::Rook)] ^= BitBoard::from(1+(1<<3));
                            self[(color, Piece::Any)] ^= BitBoard::from(1+(1<<3));
                            self.occupation ^= BitBoard::from(1+(1<<3));
                            zobrist ^= get_zobrist_table(Piece::Rook, Color::White)[0] ^ get_zobrist_table(Piece::Rook, Color::White)[3];
                        } else if m.to == Square::G1 {
                            self[(color, Piece::Rook)] ^= BitBoard::from((1<<7)+(1<<5));
                            self[(color, Piece::Any)] ^= BitBoard::from((1<<7)+(1<<5));
                            self.occupation ^= BitBoard::from((1<<7)+(1<<5));
                            zobrist ^= get_zobrist_table(Piece::Rook, Color::White)[5] ^ get_zobrist_table(Piece::Rook, Color::White)[7];
                        }
                    }
                    Color::Black => {
                        if m.to == Square::C8 {
                            self[(color, Piece::Rook)] ^= BitBoard::from((1<<56)+(1<<59));
                            self[(color, Piece::Any)] ^= BitBoard::from((1<<56)+(1<<59));
                            self.occupation ^= BitBoard::from((1<<56)+(1<<59));
                            zobrist ^= get_zobrist_table(Piece::Rook, Color::Black)[56] ^ get_zobrist_table(Piece::Rook, Color::Black)[59];
                        } else if m.to == Square::G8 {
                            self[(color, Piece::Rook)] ^= BitBoard::from((1<<63)+(1<<61));
                            self[(color, Piece::Any)] ^= BitBoard::from((1<<63)+(1<<61));
                            self.occupation ^= BitBoard::from((1<<63)+(1<<61));
                            zobrist ^= get_zobrist_table(Piece::Rook, Color::Black)[61] ^ get_zobrist_table(Piece::Rook, Color::Black)[63];
                        }
                    }
                }
            },
            MoveType::Null => panic!("Illegal Move"),
        }
        zobrist
    }
    #[inline]
    pub fn do_move(&mut self, m: Move) -> u64 {
        let color = if self[(Color::White, m.piece)].is_set(m.from) {
            Color::White
        } else {
            Color::Black
        };
        self.do_move_for_color(m,color)
    }
    #[inline]
    pub fn undo_move(&mut self, m: Move) -> u64 {
        let color = if self[(Color::White, Piece::Any)].is_set(m.to) {
            Color::White
        } else {
            Color::Black
        };
        self.do_move_for_color(m,color)
    }
    fn determine_move_type(&self, from: Square, to: Square, piece: Piece, promote: Option<Piece>) -> MoveType {
        match piece {
            Piece::King => {
                if piece == Piece::King && from == Square::E1 && (to == Square::G1 || to == Square::C1) {
                    return MoveType::Castle;
                }
                if piece == Piece::King && from == Square::E8 && (to == Square::G8 || to == Square::C8) {
                    return MoveType::Castle;
                }

            },
            Piece::Pawn => {
                if !self.occupation.is_set(to) && to.file() != from.file() {
                    return MoveType::Enpassant;
                } else if promote.is_some() {
                    if self.occupation.is_set(to) {
                        return MoveType::PromotionCapture((promote.unwrap(), self.piece_at(to).unwrap()));
                    } else {
                        return MoveType::Promotion(promote.unwrap());
                    }
                }
            },
            _ => {},
        }
        if self.occupation.is_set(to) {
            MoveType::Capture(self.piece_at(to).unwrap())
        } else {
            MoveType::Normal
        }
    }
    fn get_zobrist(&self) -> u64 {
        let mut zobrist: u64 = 0;
        for p in self[(Color::White, Piece::King)] {
            zobrist ^= constants::ZOBRIST_WHITE_KING_NUMBERS[p] as u64;
        }
        for p in self[(Color::White, Piece::Queen)] {
            zobrist ^= constants::ZOBRIST_WHITE_QUEEN_NUMBERS[p];
        }
        for p in self[(Color::White, Piece::Bishop)] {
            zobrist ^= constants::ZOBRIST_WHITE_BISHOP_NUMBERS[p];
        }
        for p in self[(Color::White, Piece::Knight)] {
            zobrist ^= constants::ZOBRIST_WHITE_KNIGHT_NUMBERS[p];
        }
        for p in self[(Color::White, Piece::Rook)] {
            zobrist ^= constants::ZOBRIST_WHITE_ROOK_NUMBERS[p];
        }
        for p in self[(Color::White, Piece::Pawn)] {
            zobrist ^= constants::ZOBRIST_WHITE_PAWN_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::King)] {
            zobrist ^= constants::ZOBRIST_BLACK_KING_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::Queen)] {
            zobrist ^= constants::ZOBRIST_BLACK_QUEEN_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::Bishop)] {
            zobrist ^= constants::ZOBRIST_BLACK_BISHOP_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::Knight)] {
            zobrist ^= constants::ZOBRIST_BLACK_KNIGHT_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::Rook)] {
            zobrist ^= constants::ZOBRIST_BLACK_ROOK_NUMBERS[p];
        }
        for p in self[(Color::Black, Piece::Pawn)] {
            zobrist ^= constants::ZOBRIST_BLACK_PAWN_NUMBERS[p];
        }
        zobrist
    }
    pub fn get_neighbours(s: Square) -> BitBoard {
        BitBoard::new(constants::NEIGHBOURS[s])
    }
    pub fn get_next_neighbours(s: Square) -> BitBoard {
        BitBoard::new(constants::NEXT_NEIGHBOURS[s])
    }
}

fn write_piece_to_position(c: char, pos: BitBoard, cboard: &mut [[char;8];8]) {
        for p in pos {
            let q: usize = p.into();
            cboard[7 - (q / 8)][q % 8] = c;
        }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut board: [[char; 8]; 8] = [['.'; 8]; 8];
        write_piece_to_position('K', self[(Color::White, Piece::King)], &mut board);
        write_piece_to_position('Q', self[(Color::White, Piece::Queen)], &mut board);
        write_piece_to_position('R', self[(Color::White, Piece::Rook)], &mut board);
        write_piece_to_position('B', self[(Color::White, Piece::Bishop)], &mut board);
        write_piece_to_position('N', self[(Color::White, Piece::Knight)], &mut board);
        write_piece_to_position('P', self[(Color::White, Piece::Pawn)], &mut board);
        write_piece_to_position('k', self[(Color::Black, Piece::King)], &mut board);
        write_piece_to_position('q', self[(Color::Black, Piece::Queen)], &mut board);
        write_piece_to_position('r', self[(Color::Black, Piece::Rook)], &mut board);
        write_piece_to_position('b', self[(Color::Black, Piece::Bishop)], &mut board);
        write_piece_to_position('n', self[(Color::Black, Piece::Knight)], &mut board);
        write_piece_to_position('p', self[(Color::Black, Piece::Pawn)], &mut board);
        let line: String = board.iter()
                                    .map(|&l| -> String{l.iter()
                                                         .map(|c| -> String {String::from(*c) + " "})
                                                         .collect()})
                                    .map(|l| l + "\n")
                                    .collect();
        write!(f, "{}", line)
    }
}

impl std::ops::Index<(Color, Piece)> for Board {
    type Output = BitBoard;

    fn index(&self, i: (Color, Piece)) -> &BitBoard {
        &self.positions[i.0 as usize][i.1 as usize]
    }
}

impl std::ops::IndexMut<(Color, Piece)> for Board {
    fn index_mut(&mut self, i: (Color, Piece)) -> &mut BitBoard {
        &mut self.positions[i.0 as usize][i.1 as usize]
    }
}

//const EAST_BORDER: BitBoard = BitBoard::new(0x8080808080808080);
//const WEST_BORDER: BitBoard = BitBoard::new(0x0101010101010101);

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum MoveType {
    Normal,
    Capture(Piece),
    Promotion(Piece),
    PromotionCapture((Piece,Piece)),
    Enpassant,
    Castle,
    Null,
}

#[derive(PartialEq,Clone,Copy)]
pub struct Move {
    pub piece: Piece,
    pub from: Square,
    pub to: Square,
    pub typ: MoveType,
}

impl Move {
    pub fn from_str(s: &str, pos: &Position) -> Move {
        let mut chars = s.chars();
        let mut from = chars.next().unwrap() as u8 - b'a';
        from += 8*(chars.next().unwrap() as u8 - b'1');
        let mut to = chars.next().unwrap() as u8 - b'a';
        to += 8*(chars.next().unwrap() as u8 - b'1');
        let piece = pos.board.piece_at(from.into()).unwrap();
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
            typ: pos.board.determine_move_type(from.into(),to.into(),piece,prom),
        }
    }
    pub fn compress(&self) -> CompressedMove {
        let mut piece_and_type = self.piece as u16;
        piece_and_type |= match self.typ {
            MoveType::Normal => 0,
            MoveType::Capture(p) => ((p as u16) << 3) | (1 << 9),
            MoveType::Promotion(p) => ((p as u16) << 3) | (2 << 9),
            MoveType::PromotionCapture((p,q)) => ((p as u16) << 3) | ((q as u16) << 6) | (3 << 9),
            MoveType::Castle => 4 << 9,
            MoveType::Enpassant => 5 << 9,
            MoveType::Null => panic!("Cannot compress null move."),
        };
        CompressedMove {
            piece_and_type,
            from: self.from.into(),
            to: self.to.into(),
        }
    }
}

#[derive(Copy,Clone,PartialEq,Default)]
pub struct CompressedMove {
    pub piece_and_type: u16,
    pub to: u8,
    pub from: u8,
}

impl CompressedMove {
    pub fn decompress(&self) -> Option<Move> {
        let piece = u16_to_piece(self.piece_and_type & 0b111);
        let typ = match self.piece_and_type >> 9 {
            0 => MoveType::Normal,
            1 => MoveType::Capture(u16_to_piece((self.piece_and_type >> 3) & 0b111)),
            2 => MoveType::Promotion(u16_to_piece((self.piece_and_type >> 3) & 0b111)),
            3 => MoveType::PromotionCapture((u16_to_piece((self.piece_and_type >> 3) & 0b111), u16_to_piece((self.piece_and_type >> 6) & 0b111))),
            4 => MoveType::Castle,
            5 => MoveType::Enpassant,
            _ => return None,
        };
        Some(Move {from: self.from.into(), to: self.to.into(), piece, typ,})
    }
}

pub type MoveList = arrayvec::ArrayVec<Move,256>;

fn display_promotion(m: &Move) -> String {
    match m.typ {
        MoveType::Promotion(Piece::Queen) => "q".to_string(),
        MoveType::Promotion(Piece::Rook) => "r".to_string(),
        MoveType::Promotion(Piece::Bishop) => "b".to_string(),
        MoveType::Promotion(Piece::Knight) => "n".to_string(),
        MoveType::PromotionCapture((Piece::Queen,_)) => "q".to_string(),
        MoveType::PromotionCapture((Piece::Rook,_)) => "r".to_string(),
        MoveType::PromotionCapture((Piece::Bishop,_)) => "b".to_string(),
        MoveType::PromotionCapture((Piece::Knight,_)) => "n".to_string(),
        _ => "".to_string()
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", self.from, self.to, display_promotion(self))
    }
}

fn u16_to_piece(p: u16) -> Piece {
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

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum Piece {
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
    Any
}

impl Piece {
    pub fn value(self) -> i32 {
        match self {
            Self::Pawn => 100,
            Self::Bishop => 300,
            Self::Knight => 300,
            Self::Rook => 500,
            Self::Queen => 900,
            Self::King => 10000,
            Self::Any => 0,
        }
    }
}

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }
}

#[derive(Clone)]
pub struct Position {
    pub board: Board,
    move_history: Vec<Move>,
    to_move: Color,
    // castling contains info which types of castling are currently allowed
    //layout [[White Kingside, White Queenside],
    //        [Black Kingside, Black Queenside]]
    castling_legal: Vec<[[bool; 2]; 2]>,
    rule_50_counts: Vec<u8>,
    attacked_squares: BitBoard,
    king_attackers: BitBoard,
    pinned_pieces: BitBoard,
    zobrist: u64, //Zobrist-Hash
    history: Vec<u64>, //zobrist hashes of all positions reached BEFORE the current
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub fn new() -> Position {
        let board = Board::new();
        let mut zobrist = board.get_zobrist();
        for i in 0..4 {
            zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[i];
        }
        Position {
            board,
            move_history: Vec::with_capacity(20),
            to_move: Color::White,
            castling_legal: vec![[[true, true], [true, true]]],
            rule_50_counts: vec![0],
            attacked_squares: BitBoard::EMPTY,
            king_attackers: BitBoard::EMPTY,
            pinned_pieces: BitBoard::EMPTY,
            zobrist,
            history: Vec::new(),
        }
    }
    pub fn get_board(&self) -> &Board {
        &self.board
    }
    pub fn from_fen(fen: String) -> Option<Position> {
        //First set up the pieces
        let mut fen_parts = fen.split_whitespace();
        let b = Board::from_fen(fen_parts.next().unwrap())?;
        let mut pos = Position {
            board: b,
            move_history: Vec::with_capacity(20),
            to_move: Color::White,
            castling_legal: Vec::with_capacity(20),
            rule_50_counts: Vec::with_capacity(20),
            attacked_squares: BitBoard::EMPTY,
            king_attackers: BitBoard::EMPTY,
            pinned_pieces: BitBoard::EMPTY,
            zobrist: 0,
            history: Vec::new(),
        };
        pos.zobrist = pos.board.get_zobrist();
        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {pos.to_move = Color::White;}
                else if p == "b" {
                    pos.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
                    pos.to_move = Color::Black;
                }
                else {return None;}
            },
            None => return None,
        }
        //Set castling rights
        let mut castling_legal = [[false,false],[false,false]];
        match fen_parts.next() {
            Some(p) => {
                if p.contains('K') {
                    castling_legal[0][0]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
                }
                if p.contains('Q') {
                    castling_legal[0][1]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
                }
                if p.contains('k') {
                    castling_legal[1][0]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
                }
                if p.contains('q') {
                    castling_legal[1][1]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
                }
            },
            None => return None,
        }
        pos.castling_legal.push(castling_legal);
        //Set en passant if necessary
        const EP_MOVE_SQUARES: [(Square, Square); 8] = [(Square::A2, Square::A4), (Square::B2, Square::B4),
                                                        (Square::C2, Square::C4), (Square::D2, Square::D4),
                                                        (Square::E2, Square::E4), (Square::F2, Square::F4),
                                                        (Square::G2, Square::G4), (Square::H2, Square::H4)];
        match fen_parts.next() {
            Some(p) => {
                match p {
                    "-" => {},
                    _ => {
                        let sq = Square::from_string(p);
                        if sq.rank() == Rank::Third {
                            pos.move_history.push(Move {
                                from: EP_MOVE_SQUARES[sq.file() as usize].0,
                                to:   EP_MOVE_SQUARES[sq.file() as usize].1,
                                piece: Piece::Pawn,
                                typ: MoveType::Enpassant});
                        } else if sq.rank() == Rank::Sixth {
                            pos.move_history.push(Move {
                                from: EP_MOVE_SQUARES[sq.file() as usize].0.flipped(),
                                to:   EP_MOVE_SQUARES[sq.file() as usize].1.flipped(),
                                piece: Piece::Pawn,
                                typ: MoveType::Enpassant});
                        }
                    }
                }
            },
            None => return None,
        }
        if !pos.move_history.is_empty() {
            pos.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[pos.move_history.last()
                                                                                .unwrap()
                                                                                .from
                                                                                .file()
                                                                                as usize];
        }
        match fen_parts.next() {
            Some(p) => {
                match p.parse::<u8>() {
                    Ok(n) => pos.rule_50_counts.push(n),
                    Err(_) => return None,
                }
            },
            None => return None,
        }
        Some(pos)
    }
    // Compute the knight moves from a given square using the lookup table
    // Computes the possible moves from the least significant bit in the Square
    #[inline]
    fn knight_moves(&self, sq: Square) -> BitBoard {
        BitBoard::new(constants::KNIGHT_MOVES[sq])
    }
    // Compute the rook moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline]
    fn rook_moves_for_occupation(&self, sq: Square, occ: BitBoard) -> BitBoard {
        let moves = constants::ROOK_MOVES[sq];
        BitBoard::new(constants::ROOK_MMASK[occ.pext(moves)
                                            + constants::ROOK_MMASK_OFFSETS[sq] as usize])
    }
    #[inline]
    fn rook_moves(&self, sq: Square) -> BitBoard {
        self.rook_moves_for_occupation(sq, self.board.occupation)
    }
    // Compute the bishop moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline]
    fn bishop_moves_for_occupation(&self, sq: Square, occ: BitBoard) -> BitBoard {
        let moves = constants::BISHOP_MOVES[sq];
        BitBoard::new(constants::BISHOP_MMASK[occ.pext(moves) + constants::BISHOP_MMASK_OFFSETS[sq] as usize])
    }
    #[inline]
    fn bishop_moves(&self, sq: Square) -> BitBoard {
        self.bishop_moves_for_occupation(sq, self.board.occupation)
    }
    // Compute the king moves from a given square using the lookup table
    #[inline]
    fn king_moves(&self, sq: Square) -> BitBoard {
        BitBoard::new(constants::NEIGHBOURS[sq])
    }
    #[inline]
    fn pawn_moves(&self, c: Color) -> (BitBoard, BitBoard) {
        let relative_third_rank = BitBoard::from_rank(Rank::Third.relative(c));
        let relative_eighth_rank = BitBoard::from_rank(Rank::Eighth.relative(c));
        let single = self.board[(c, Piece::Pawn)].shifted_forward(c) & !relative_eighth_rank
                                                                     & !self.board.occupation;
        let double = (single & relative_third_rank).shifted_forward(c) & !self.board.occupation;
        (single, double)
    }
    #[inline]
    fn pawn_promotions(&self, c: Color) -> BitBoard {
        let relative_seventh_rank = BitBoard::from_rank(Rank::Seventh.relative(c));
        (relative_seventh_rank & self.board[(c, Piece::Pawn)]).shifted_forward(c) & !self.board.occupation
    }
    #[inline]
    pub fn pawn_attacks(&self, sq: Square, c: Color) -> BitBoard {
        BitBoard::new(constants::PAWN_ATTACKS[c as usize][sq as usize])
    }
    #[inline]
    pub fn generate_attack_table(&mut self) {
        let opp = self.to_move.other();
        let king_pos = self.board[(self.to_move, Piece::King)];
        let king_square = king_pos.least_square();
        self.board.occupation ^= king_pos;
        //make sure we cause no collisions..
        self.attacked_squares = BitBoard::EMPTY;
        self.pinned_pieces = BitBoard::EMPTY;
        self.king_attackers = BitBoard::EMPTY;
        //calculate squares attacked by each pawn
        let pawns = self.board[(opp, Piece::Pawn)];
        for pawn in pawns {
            let attacked = self.pawn_attacks(pawn, opp);
            if !(attacked & king_pos).is_empty() {
                self.king_attackers |= pawn.into();
            }
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by knights
        let knights = self.board[(opp, Piece::Knight)];
        for knight in knights {
            let attacked = self.knight_moves(knight);
            if !(attacked & king_pos).is_empty() {
                self.king_attackers |= knight.into();
            }
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by king
        self.attacked_squares |= self.king_moves(self.board[(opp, Piece::King)].least_square());
        //calculate squares attacked by rooks
        let rook_moves_from_king = self.rook_moves(king_square);
        let rooks = self.board[(opp, Piece::Rook)];
        for rook in rooks {
            let attacked = self.rook_moves(rook);
            if !(attacked & king_pos).is_empty() {
                self.king_attackers |= rook.into();
            }
            self.pinned_pieces |= attacked & rook_moves_from_king
                                           & self.board.occupation
                                           & BitBoard::new(constants::RAYS[king_square][rook]);
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by bishops
        let bish_moves_from_king = self.bishop_moves(king_square);
        let bishops = self.board[(opp, Piece::Bishop)];
        for bishop in bishops {
            let attacked = self.bishop_moves(bishop);
            if !(attacked & king_pos).is_empty() {
                self.king_attackers |= bishop.into();
            }
            self.pinned_pieces |= attacked & bish_moves_from_king
                                           & self.board.occupation
                                           & BitBoard::new(constants::RAYS[king_square][bishop]);
            self.attacked_squares |= attacked;
        }
        let queens = self.board[(opp, Piece::Queen)];
        //calculate squares attacked by queens
        for queen in queens {
            let attacked_r = self.rook_moves(queen);
            let attacked_b = self.bishop_moves(queen);
            if !((attacked_r | attacked_b) & king_pos).is_empty() {
                self.king_attackers |= queen.into();
            }
            self.pinned_pieces |= attacked_r & rook_moves_from_king
                                           & self.board.occupation
                                           & BitBoard::new(constants::RAYS[king_square][queen]);
            self.pinned_pieces |= attacked_b & bish_moves_from_king
                                           & self.board.occupation
                                           & BitBoard::new(constants::RAYS[king_square][queen]);
            self.attacked_squares |= attacked_b | attacked_r;
        }
        self.board.occupation ^= king_pos;
    }
    //Calculate possible moves of a piece on a given square, using the provide move gen closure
    //
    #[inline]
    fn get_piece_moves(&self, move_getter: impl Fn(Square) -> BitBoard, moves: &mut MoveList, p: Piece) {
        for pos in self.board[(self.to_move, p)] {
            let mut pmoves = move_getter(pos);
            pmoves &= pmoves ^ self.board[(self.to_move, Piece::Any)];
            if !(self.pinned_pieces & pos.into()).is_empty() {
                pmoves &= BitBoard::new(constants::RAYS[pos][self.board[(self.to_move, Piece::King)].least_square()]);
            }
            for m in pmoves {
                moves.push(Move{
                    from: pos,
                    to: m,
                    piece: p,
                    typ: if !self.board.occupation.is_set(m) {
                        MoveType::Normal
                    } else {
                        MoveType::Capture(self.board.piece_at(m).unwrap())
                    },
                });
            }
        }
    }
    #[inline]
    fn handle_en_passant(&mut self, moves: &mut MoveList, check_mask: BitBoard) {
        if let Some(m) = self.move_history.last() {
            let esq = ep_square(m.to.file()).relative(self.to_move.other());
            if m.piece == Piece::Pawn && m.from.relative(self.to_move).rank() == Rank::Seventh
                    && m.to.relative(self.to_move).rank() == Rank::Fifth &&
                    (check_mask.is_set(esq) || check_mask.is_set(m.to)) {
                let cands = self.pawn_attacks(esq, self.to_move.other()) & self.board[(self.to_move, Piece::Pawn)];
                for cand in cands {
                    let kpos = self.board[(self.to_move, Piece::King)];
                    let pin_ray = BitBoard::new(constants::RAYS[cand][kpos.least_square()]);
                    //Check if we expose the king by taking en passant
                    //See if our pawn is pinned
                    if (self.pinned_pieces.is_set(cand) && !pin_ray.is_set(esq)) ||
                        (self.pinned_pieces.is_set(m.to)
                         && !BitBoard::new(constants::RAYS[m.to][kpos.least_square()]).is_set(esq)) {
                        continue;
                    //Check for a double pin by a rook or queen.
                    } else if pin_ray.is_set(m.to) {
                        self.board.occupation.unset(m.to);
                        self.board.occupation.unset(cand);
                        let k_ray = pin_ray & self.rook_moves(kpos.least_square());
                        self.board.occupation.set(m.to);
                        self.board.occupation.set(cand);
                        if !(k_ray & self.board[(self.to_move.other(), Piece::Rook)]).is_empty()
                        || !(k_ray & self.board[(self.to_move.other(), Piece::Queen)]).is_empty() {
                            continue;
                        }
                    }
                    moves.push(Move {
                    from: cand,
                    to: esq,
                    piece: Piece::Pawn,
                    typ: MoveType::Enpassant,
                    });
                }
            }
        }
    }
    #[inline]
    fn handle_castling(&self, moves: &mut MoveList) {
        if self.king_attackers.count() != 0 {return;}
        match self.to_move {
            Color::White => {
                const WHITE_KING_CASTLE_MASK: BitBoard  = BitBoard::new(0b01100000);
                const WHITE_QUEEN_CASTLE_CHECK_MASK: BitBoard = BitBoard::new(0b00001100);
                const WHITE_QUEEN_CASTLE_MATERIAL_MASK: BitBoard = BitBoard::new(0b00001110);
                //Check for kingside castling.
                if self.castling_legal.last().unwrap()[0][0]
                    && ((self.attacked_squares | self.board.occupation) & WHITE_KING_CASTLE_MASK).is_empty() {
                        moves.push(Move {
                            from: Square::E1,
                            to:   Square::G1,
                            piece: Piece::King,
                            typ: MoveType::Castle,
                        });
                }
                if self.castling_legal.last().unwrap()[0][1]
                    && (self.attacked_squares & WHITE_QUEEN_CASTLE_CHECK_MASK).is_empty()
                    && (self.board.occupation & WHITE_QUEEN_CASTLE_MATERIAL_MASK).is_empty() {
                        moves.push(Move {
                            from: Square::E1,
                            to:   Square::C1,
                            piece: Piece::King,
                            typ: MoveType::Castle,
                        });
                }
            },
            Color::Black => {
                const BLACK_KING_CASTLE_MASK: BitBoard  = BitBoard::new(0b01100000 << 56);
                const BLACK_QUEEN_CASTLE_CHECK_MASK: BitBoard = BitBoard::new(0b00001100 << 56);
                const BLACK_QUEEN_CASTLE_MATERIAL_MASK: BitBoard = BitBoard::new(0b00001110 << 56);
                //Check for kingside castling.
                if self.castling_legal.last().unwrap()[1][0]
                    && ((self.attacked_squares | self.board.occupation) & BLACK_KING_CASTLE_MASK).is_empty() {
                        moves.push(Move {
                            from: Square::E8,
                            to:   Square::G8,
                            piece: Piece::King,
                            typ: MoveType::Castle,
                        });
                }
                if self.castling_legal.last().unwrap()[1][1]
                    && (self.attacked_squares & BLACK_QUEEN_CASTLE_CHECK_MASK).is_empty()
                    && (self.board.occupation & BLACK_QUEEN_CASTLE_MATERIAL_MASK).is_empty() {
                        moves.push(Move {
                            from: Square::E8,
                            to:   Square::C8,
                            piece: Piece::King,
                            typ: MoveType::Castle,
                        });
                }
            },
        }
    }
    #[inline]
    fn get_pawn_moves(&self, moves: &mut MoveList, check_mask: BitBoard) {
            const PROMOTION_PIECES: [Piece; 4] = [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];
            let pawns = self.board[(self.to_move, Piece::Pawn)];
            let ksq = self.board[(self.to_move, Piece::King)].least_square();
            let (advances, double_advances) = self.pawn_moves(self.to_move);
            let direction = match self.to_move {Color::White => 1, Color::Black => -1};
            for to in advances & check_mask {
                let from = to.shifted_by(-direction * 8);
                if self.pinned_pieces.is_set(from)
                    && !BitBoard::new(constants::RAYS[ksq][from]).is_set(to) {continue;}
                moves.push(Move { piece: Piece::Pawn, from, to, typ: MoveType::Normal });
            }
            for to in double_advances & check_mask {
                let from = to.shifted_by(-direction * 16);
                if self.pinned_pieces.is_set(from)
                    && !BitBoard::new(constants::RAYS[ksq][from]).is_set(to) {continue;}
                moves.push(Move { piece: Piece::Pawn, from, to, typ: MoveType::Normal });
            }
            for to in self.pawn_promotions(self.to_move) & check_mask {
                let from = to.shifted_by(-direction * 8);
                if self.pinned_pieces.is_set(from) {continue;}
                for p in PROMOTION_PIECES {
                    moves.push(Move { piece: Piece::Pawn, from, to, typ: MoveType::Promotion(p) });
                }
            }
            for pawn in pawns {
                let mut attacks = (self.pawn_attacks(pawn, self.to_move) &
                                     self.board[(self.to_move.other(), Piece::Any)])
                                   & check_mask;
                if self.pinned_pieces.is_set(pawn) {
                    attacks &= BitBoard::new(constants::RAYS[ksq][pawn]);
                }
                for m in attacks {
                    let cap = self.board.piece_at(m).unwrap();
                    if m.rank().relative(self.to_move) == Rank::Eighth {
                        for p in PROMOTION_PIECES {
                            moves.push(Move{
                                from: pawn,
                                to: m,
                                piece: Piece::Pawn,
                                typ: MoveType::PromotionCapture((p,cap))
                            });
                        }
                    } else {
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::Capture(cap)
                        });
                    }
                }
            }
    }
    #[allow(dead_code)]
    pub fn get_opponent_moves(&mut self) -> MoveList {
        self.to_move = self.to_move.other();
        self.attacked_squares = BitBoard::EMPTY;
        let moves = self.get_moves();
        self.to_move = self.to_move.other();
        self.attacked_squares = BitBoard::EMPTY;
        moves
    }
    pub fn get_moves(&mut self) -> MoveList {
        //We expect about 35 moves in the average position
        let mut moves = MoveList::new();
        if *self.rule_50_counts.last().unwrap_or_else(|| panic!()) == 100
            || self.board.occupation.count() == 2 {
            return moves;
        }
        if self.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        //Generate King moves first (except castling)
        let mut king_moves = self.king_moves(self.board[(self.to_move, Piece::King)].least_square());
        king_moves &= king_moves ^ self.board[(self.to_move, Piece::Any)];
        king_moves &= king_moves ^ self.attacked_squares;
        for km in king_moves {
            moves.push(Move{
                from: self.board[(self.to_move, Piece::King)].least_square(),
                to: km,
                piece: Piece::King,
                typ: if !self.board.occupation.is_set(km) {
                    MoveType::Normal
                } else {
                    MoveType::Capture(self.board.piece_at(km).unwrap())
                }
            });
        }
        //optimize double or better check
        if self.king_attackers.count() > 1 {
            return moves
        }
        //If we are in check, we may only take the checker, or block it
        //If the checking piece is a knight, we can only take (or move the king)
        let check_mask = if !(self.board[(self.to_move.other(), Piece::Knight)]
                              & self.king_attackers).is_empty() {
            self.king_attackers
        //For any other piece we may also try to block
        } else if !self.king_attackers.is_empty() {
            BitBoard::new(constants::CONNECTING_RAYS[self.king_attackers.least_square()][self.board[(self.to_move, Piece::King)].least_square()]) ^ self.board[(self.to_move, Piece::King)]
        } else {
            BitBoard::FULL
        };
        //generate queen moves
        self.get_piece_moves(|sq: Square| -> BitBoard
                             {(self.rook_moves(sq) | self.bishop_moves(sq)) & check_mask},
                             &mut moves,
                             Piece::Queen);
        //generate rook moves
        self.get_piece_moves(|sq: Square| -> BitBoard {self.rook_moves(sq) & check_mask},
                             &mut moves,
                             Piece::Rook);
        //generate bishop moves
        self.get_piece_moves(|sq: Square| -> BitBoard {self.bishop_moves(sq) & check_mask},
                             &mut moves,
                             Piece::Bishop);
        //generate knight moves
        self.get_piece_moves(|sq: Square| -> BitBoard {self.knight_moves(sq) & check_mask},
                             &mut moves,
                             Piece::Knight);
        //generate pawn moves without en passant
        self.get_pawn_moves(&mut moves, check_mask);
        //Handle castling and en passant.
        self.handle_castling(&mut moves);
        self.handle_en_passant(&mut moves, check_mask);
        moves
    }
    //tells us to score this as + or - one for white/black
    pub fn color(&self) -> Color {
        self.to_move
    }
    pub fn from_move(&self, m: Move) -> Position {
        let mut pos = self.clone();
        pos.do_move(m);
        pos
    }
    pub fn do_move(&mut self, m: Move) {
        if (self.board[(Color::White,Piece::King)] | self.board[(Color::Black,Piece::King)]).is_set(m.to) {
            panic!("Invalid move {} in position\n{}\n(previos move {})", m, self.board, self.move_history.last().unwrap_or(&Move {from: Square::A1, to: Square::A1, piece:Piece::King, typ:MoveType::Normal}));
        }
        //Commit zobrist hash to history stack
        self.history.push(self.zobrist);
        //Unset the Zobrist en passant flag, if necessary
        if !self.move_history.is_empty() {
            let m = self.move_history.last().unwrap();
            if m.piece == Piece::Pawn && m.from.relative(self.to_move.other()).rank() == Rank::Second
                                      && m.to.relative(self.to_move.other()).rank() == Rank::Fourth {
                self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.file() as usize];
            }
        }
        self.move_history.push(m);

        self.zobrist ^= self.board.do_move(m);
        let mut castling_legal = *self.castling_legal.last().unwrap_or_else(|| panic!());
        if m.piece == Piece::King {
            if castling_legal[self.to_move as usize][0] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2*self.to_move as usize];
            }
            if castling_legal[self.to_move as usize][1] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2*self.to_move as usize+1];
            }
            castling_legal[self.to_move as usize] = [false,false];
        }
        if castling_legal[0][1] && (m.to == Square::A1 || m.from == Square::A1) {
            castling_legal[0][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
        } else if castling_legal[0][0] && (m.to == Square::H1 || m.from == Square::H1) {
            castling_legal[0][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
        }
        if castling_legal[1][0] && (m.to == Square::H8 || m.from == Square::H8) {
            castling_legal[1][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
        } else if castling_legal[1][1] && (m.to == Square::A8 || m.from == Square::A8) {
            castling_legal[1][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
        }
        self.castling_legal.push(castling_legal);
        self.attacked_squares = BitBoard::EMPTY;
        self.pinned_pieces = BitBoard::EMPTY;
        self.king_attackers = BitBoard::EMPTY;
        self.to_move = self.to_move.other();
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        if m.piece == Piece::Pawn || matches!(m.typ,MoveType::Capture(_)) {
            self.rule_50_counts.push(0);
        } else {
            self.rule_50_counts.push(*self.rule_50_counts.last().unwrap_or(&0)+1);
        }
    }
    #[allow(dead_code)]
    pub fn get_castling_rights(&self) -> [[bool;2];2] {
        *self.castling_legal.last().unwrap_or_else(|| panic!("No castling rights specified."))
    }
    //TODO: find better solution for castling rights?
    pub fn undo_move(&mut self) {
        //remove move from history stack
        self.history.pop();
        self.rule_50_counts.pop();
        let m = self.move_history.pop().unwrap_or_else(|| panic!("No move to undo!"));
        let castling = self.castling_legal.pop().unwrap_or_else(|| panic!("No castling rights specified."));

        self.zobrist ^= self.board.undo_move(m);
        //Reconstruct castling flags
        if self.castling_legal.last().unwrap()[0][0] ^ castling[0][0] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
        }
        if self.castling_legal.last().unwrap()[0][1] ^ castling[0][1] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
        }
        if self.castling_legal.last().unwrap()[1][0] ^ castling[1][0] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
        }
        if self.castling_legal.last().unwrap()[1][1] ^ castling[1][1] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
        }
        self.attacked_squares = BitBoard::EMPTY;
        self.pinned_pieces = BitBoard::EMPTY;
        self.king_attackers = BitBoard::EMPTY;
        self.to_move = self.to_move.other();
        //switch Zobrist color flag.
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        //Set the Zobrist en passant flag, if necessary
        if !self.move_history.is_empty() {
            let m = self.move_history.last().unwrap();
            if m.piece == Piece::Pawn && m.from.relative(self.to_move.other()).rank() == Rank::Second
                                      && m.to.relative(self.to_move.other()).rank() == Rank::Fourth {
                self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.file() as usize];
            }
        }
    }
    pub fn do_null_move(&mut self) {
        self.move_history.push(Move {typ: MoveType::Null, piece: Piece::Any, to: Square::A1, from: Square::A1});
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        self.to_move = self.to_move.other();
        self.attacked_squares = BitBoard::EMPTY;
        self.pinned_pieces = BitBoard::EMPTY;
        self.king_attackers = BitBoard::EMPTY;
    }
    pub fn undo_null_move(&mut self) {
        self.move_history.pop();
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        self.to_move = self.to_move.other();
        self.attacked_squares = BitBoard::EMPTY;
        self.pinned_pieces = BitBoard::EMPTY;
        self.king_attackers = BitBoard::EMPTY;
    }
    pub fn in_check(&mut self) -> bool {
        if self.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        !self.king_attackers.is_empty()
    }
    pub fn piece_count(&self, c: Color, p: Piece) -> i32 {
        self.board[(c,p)].count() as i32
    }
    #[allow(dead_code)]
    pub fn total_piece_count(&self) -> i32 {
        self.board.occupation.count() as i32
    }
    fn material_count(&self, c: Color) -> i32 {
        self.piece_count(c, Piece::Pawn) + 3*(self.piece_count(c, Piece::Bishop)+self.piece_count(c,Piece::Knight)) + 5*self.piece_count(c, Piece::Rook)+9*self.piece_count(c, Piece::Queen)
    }
    pub fn material_balance(&self) -> i32 {
        self.material_count(self.to_move) - self.material_count(self.to_move.other())
    }
    #[allow(dead_code)]
    pub fn is_attacked(&self, sq: Square) -> bool {
        self.attacked_squares.is_set(sq)
    }
    #[allow(dead_code)]
    pub fn piece_attacks(&self, p: Piece, c: Color, from: Square, target: Square) -> bool {
        match p {
            Piece::Pawn => self.pawn_attacks(from, c).is_set(target),
            Piece::Knight => self.knight_moves(from).is_set(target),
            Piece::Bishop => self.bishop_moves(from).is_set(target),
            Piece::Rook => self.rook_moves(from).is_set(target),
            Piece::Queen => (self.rook_moves(from) | self.bishop_moves(from)).is_set(target),
            //TODO: King..
            _ => false,
        }
    }
    #[allow(dead_code)]
    pub fn get_last_move(&self) -> Option<Move> {
        self.move_history.last().copied()
    }
    pub fn zobrist_hash(&self) -> u64 {
        self.zobrist
    }
    #[allow(dead_code)]
    pub fn color_factor(&self) -> i32 {
        match self.to_move {
            Color::White => 1,
            Color::Black => -1,
        }
    }
    #[allow(dead_code)]
    pub fn hard_pins(&mut self) -> BitBoard {
        if self.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        self.pinned_pieces
    }
    //Switch color for analysis
    //Does NOT update Zobrist hash
    pub fn switch_color(&mut self) {
        self.to_move = self.to_move.other();
        self.attacked_squares = BitBoard::EMPTY;
    }
    //TODO: Should this _really_ be here? But where else to put it?
    //Helper for SEE
    #[inline]
    fn least_valuable_attacker(&self, mut attackers: BitBoard, c: Color) -> Option<(Square, Piece)> {
        attackers &= self.board[(c, Piece::Any)];
        attackers.into_iter().map(|a| self.board.piece_at(a).map(|p| (a,p)))
                             .filter(|x| x.is_some())
                             .min_by_key(|x| x.unwrap().1.value()).flatten()
    }
    //X-ray attacks.
    //We ignore en passant here! Attackers are sorted by value!
    pub fn see(&self, m: Move) -> i32 {
        let target_piece = match m.typ {
            MoveType::Capture(p) => p,
            _ => return 0,
        };
        let target = m.to;
        let mut color = self.to_move;
        let mut occupation = self.board.occupation;
        //We initialize pawn and knight attacks, since they do not depend on the occupation.
        let mut attackers = (self.pawn_attacks(target, Color::White) & self.board[(Color::Black, Piece::Pawn)])
            | (self.pawn_attacks(target, Color::Black) & self.board[(Color::White, Piece::Pawn)]);
        attackers |= self.knight_moves(target) & (self.board[(Color::White, Piece::Knight)] | self.board[(Color::Black, Piece::Knight)]);
        attackers |= m.from.into();
        //We guess that most exchanges will feature less than ten pieces, which seems a safe
        //assumption
        let mut gain = Vec::with_capacity(10);
        let mut taker = m.piece;
        gain.push(target_piece.value());
        let mut from = m.from;
        loop {
            let last_gain = *gain.last().unwrap_or(&0);
            gain.push(taker.value() - last_gain);
            if std::cmp::max(-last_gain, taker.value()-last_gain) < 0 {break;}
            occupation ^= from.into();
            attackers ^= from.into();
            color = color.other();
            attackers |= self.bishop_moves_for_occupation(target, occupation)
                        & (self.board[(Color::Black, Piece::Bishop)] | self.board[(Color::White, Piece::Bishop)]
                          | self.board[(Color::Black, Piece::Queen)] | self.board[(Color::White, Piece::Queen)])
                        & occupation;
            attackers |= self.rook_moves_for_occupation(target, occupation)
                        & (self.board[(Color::Black, Piece::Rook)] | self.board[(Color::White, Piece::Rook)]
                          | self.board[(Color::Black, Piece::Queen)] | self.board[(Color::White, Piece::Queen)])
                        & occupation; //Do not accidentally include already used pieces again
            let next = match self.least_valuable_attacker(attackers, color) {
                Some(a) => a,
                None => break,
            };
            taker = next.1;
            from = next.0;
        }
        gain.reverse();
        for d in 1..gain.len()-1 {
            let g = -std::cmp::max(-gain[d+1],gain[d]);
            gain[d+1] = g;
        }
        return *gain.last().unwrap();
    }
    pub fn is_threefold(&self) -> bool {
        self.history.iter().filter(|x| **x == self.zobrist).count() > 1
    }
    pub fn is_repetition(&self) -> bool {
        self.history.iter().filter(|x| **x == self.zobrist).count() > 0
    }
    pub fn rule_50_count(&self) -> u8 {
        *self.rule_50_counts.last().unwrap()
    }
    pub fn gives_check(&self, m: &Move) -> bool {
        let kpos = self.board[(self.color().other(), Piece::King)].least_square();
        let final_piece = match m.typ {
            MoveType::Promotion(p) | MoveType::PromotionCapture((p,_)) => p,
            _ => m.piece,
        };
        match final_piece {
            Piece::Knight => self.knight_moves(kpos).is_set(m.to),
            Piece::Bishop => self.bishop_moves(kpos).is_set(m.to),
            Piece::Rook => self.rook_moves(kpos).is_set(m.to),
            Piece::Queen => (self.rook_moves(kpos) | self.bishop_moves(kpos)).is_set(m.to),
            Piece::Pawn => {
                let attacked_sqs = BitBoard::new(constants::PAWN_ATTACKS[self.to_move as usize][m.to]);
                attacked_sqs.is_set(kpos)
            },
            _ => false,
        }
    }
}

//Tests
#[test]
fn simple_sse() {
    let pos = Position::from_fen(String::from("1k1r4/1pp4p/p7/4p3/8/P5P1/1PP4P/2K1R3 w - - 0 0")).unwrap();
    let mov = Move {from: Square::E1, to: Square::E5, typ: MoveType::Capture(Piece::Pawn), piece: Piece::Rook};
    assert!(pos.see(mov) == 100);
    let pos2 = Position::from_fen(String::from("1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 0")).unwrap();
    let mov2 = Move {from: Square::D3, to: Square::E5, typ: MoveType::Capture(Piece::Pawn), piece: Piece::Knight};
    assert!(pos2.see(mov2) == -200);
}

#[test]
fn compress_and_decompress_move() {
    let pieces = vec![Piece::Pawn, Piece::King, Piece::Queen, Piece::Bishop, Piece::Knight, Piece::Rook];
    for p in pieces.iter() {
        let m = Move {from: Square::A1, to: Square::B1, piece: *p, typ: MoveType::Normal};
        let m2 = m.compress().decompress();
        assert!(m == m2.unwrap());
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {from: Square::G7,to: Square::H8,piece: Piece::Pawn, typ: MoveType::PromotionCapture((*p,*q))};
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {from: Square::G7, to: Square::H8, piece: *p, typ: MoveType::Capture(*q)};
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
}
