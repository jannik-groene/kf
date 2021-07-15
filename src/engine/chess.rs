use std::sync::Arc;
use bitintr::Pext;
use std::fmt;
use std::iter::Iterator;

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
    positions: [[Square; 7]; 2],
    pub occupation: Square,
}

//translate a fen symbol (kqbnrpKQBNRP) into a (Color, Piece) pair
fn fen_to_type(c: char) -> Option<(Color, Piece)> {
    match c {
        'k' => Some((Color::BLACK, Piece::KING)),
        'q' => Some((Color::BLACK, Piece::QUEEN)),
        'r' => Some((Color::BLACK, Piece::ROOK)),
        'n' => Some((Color::BLACK, Piece::KNIGHT)),
        'b' => Some((Color::BLACK, Piece::BISHOP)),
        'p' => Some((Color::BLACK, Piece::PAWN)),
        'K' => Some((Color::WHITE, Piece::KING)),
        'Q' => Some((Color::WHITE, Piece::QUEEN)),
        'R' => Some((Color::WHITE, Piece::ROOK)),
        'B' => Some((Color::WHITE, Piece::BISHOP)),
        'N' => Some((Color::WHITE, Piece::KNIGHT)),
        'P' => Some((Color::WHITE, Piece::PAWN)),
        _ => None,
    }
}

#[derive(Copy,Clone,Debug,PartialEq)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

#[derive(Copy,Clone,Debug,PartialEq)]
pub enum Rank {
    FIRST,
    SECOND,
    THIRD,
    FOURTH,
    FIFTH,
    SIXTH,
    SEVENTH,
    EIGHT,
    NINTH,
}

#[inline(always)]
fn get_zobrist_table(p: Piece, c: Color) -> &'static[u64] {
    match c {
        Color::WHITE => match p {
            Piece::KING => &constants::ZOBRIST_WHITE_KING_NUMBERS,
            Piece::QUEEN => &constants::ZOBRIST_WHITE_QUEEN_NUMBERS,
            Piece::BISHOP => &constants::ZOBRIST_WHITE_BISHOP_NUMBERS,
            Piece::KNIGHT => &constants::ZOBRIST_WHITE_KNIGHT_NUMBERS,
            Piece::ROOK => &constants::ZOBRIST_BLACK_ROOK_NUMBERS,
            Piece::PAWN => &constants::ZOBRIST_WHITE_PAWN_NUMBERS,
            Piece::ANY => panic!("Invalid Piece")
        },
        Color::BLACK => match p {
            Piece::KING => &constants::ZOBRIST_BLACK_KING_NUMBERS,
            Piece::QUEEN => &constants::ZOBRIST_BLACK_QUEEN_NUMBERS,
            Piece::BISHOP => &constants::ZOBRIST_BLACK_BISHOP_NUMBERS,
            Piece::KNIGHT => &constants::ZOBRIST_BLACK_KNIGHT_NUMBERS,
            Piece::ROOK => &constants::ZOBRIST_BLACK_ROOK_NUMBERS,
            Piece::PAWN => &constants::ZOBRIST_BLACK_PAWN_NUMBERS,
            Piece::ANY => panic!("Invalid Piece")
        }
    }
}

impl Board {
    pub const FILE: Square = 0b0000000100000001000000010000000100000001000000010000000100000001;
    #[inline(always)]
    pub fn file(f: File) -> Square {
        return Board::FILE << f as u64;
    }
    pub const RANK: Square = 0b11111111;
    #[inline(always)]
    pub fn rank(r: Rank) -> Square {
        return Board::RANK << r as u64;
    }
    #[inline(always)]
    pub fn get_file(sq: Square) -> File {
        match sq.trailing_zeros() % 8 {
            0 => File::A,
            1 => File::B,
            2 => File::C,
            3 => File::D,
            4 => File::E,
            5 => File::F,
            6 => File::G,
            _ => File::H,
        }
    }
    #[inline(always)]
    pub fn get_rank(sq: Square) -> Rank {
        match sq.trailing_zeros() / 8 {
            0 => Rank::FIRST,
            1 => Rank::SECOND,
            2 => Rank::THIRD,
            3 => Rank::FOURTH,
            4 => Rank::FIFTH,
            5 => Rank::SIXTH,
            6 => Rank::SEVENTH,
            _ => Rank::EIGHT,
        }
    }
    #[inline(always)]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        if (self[(Color::WHITE, Piece::PAWN)] | self[(Color::BLACK, Piece::PAWN)]) & sq != 0 {
            Some(Piece::PAWN)
        } else if (self[(Color::WHITE, Piece::BISHOP)] | self[(Color::BLACK, Piece::BISHOP)]) & sq != 0 {
            Some(Piece::BISHOP)
        } else if (self[(Color::WHITE, Piece::KNIGHT)] | self[(Color::BLACK, Piece::KNIGHT)]) & sq != 0 {
            Some(Piece::KNIGHT)
        } else if (self[(Color::WHITE, Piece::ROOK)] | self[(Color::BLACK, Piece::ROOK)]) & sq != 0 {
            Some(Piece::ROOK)
        } else if (self[(Color::WHITE, Piece::QUEEN)] | self[(Color::BLACK, Piece::QUEEN)]) & sq != 0 {
            Some(Piece::QUEEN)
        } else if (self[(Color::WHITE, Piece::KING)] | self[(Color::BLACK, Piece::KING)]) & sq != 0 {
            Some(Piece::KING)
        } else {
            None
        }
    }
    pub fn new() -> Board {
        Board{
            positions: [[1<<4, 1<<3, (1<<2)+(1<<5), (1<<1)+(1<<6), 1+(1<<7), 0b11111111<<8, 0b1111111111111111],
                        [1<<60, 1<<59, (1<<58)+(1<<61), (1<<57)+(1<<62), (1<<56)+(1<<63), 0b11111111<<48, 0b1111111111111111<<48]],
            occupation: (0b1111111111111111 << 48) + 0b1111111111111111,
        }
    }
    pub fn empty() -> Board{
        Board{
            positions: [[0,0,0,0,0,0,0],[0,0,0,0,0,0,0]],
            occupation:  0,
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
                            board[t] |= 1 << (x + 8*y);
                            board[(t.0, Piece::ANY)] |= 1 << (x + 8*y);
                            board.occupation |= 1 << (x + 8*y);
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
    #[inline(always)]
    fn do_move_for_color(&mut self, m: Move, color: Color) -> u64 {
        //Move the moving Piece
        let mut zobrist = get_zobrist_table(m.piece, color)[m.from.trailing_zeros() as usize];
        self[(color, m.piece)] ^= m.from;
        self[(color, Piece::ANY)] ^= m.from;
        self.occupation ^= m.from;
        self.occupation ^= m.to;
        self[(color, Piece::ANY)] ^= m.to;
        match m.typ {
            MoveType::MOVE => {
                self[(color, m.piece)] ^= m.to;
                zobrist ^= get_zobrist_table(m.piece, color)[m.to.trailing_zeros() as usize];
                if m.piece == Piece::PAWN && (m.from < 1 << 16 || m.from > 1 << 47) && (m.to > 1 << 23 || m.to < 1 << 40) {
                    zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.trailing_zeros() as usize % 8];
                }
            },
            MoveType::CAPTURE(p) => {
                self[(color.other(),p)] ^= m.to;
                self[(color.other(), Piece::ANY)] ^= m.to;
                self[(color, m.piece)] ^= m.to;
                self.occupation ^= m.to;
                zobrist ^= get_zobrist_table(m.piece, color)[m.to.trailing_zeros() as usize];
                zobrist ^= get_zobrist_table(p, color.other())[m.to.trailing_zeros() as usize];
            },
            MoveType::PROMOTION(p) => {
                self[(color, p)] ^= m.to;
                zobrist ^= get_zobrist_table(p, color)[m.to.trailing_zeros() as usize];
            },
            MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => {
                self[(color.other(),p_cap)] ^= m.to;
                self[(color.other(), Piece::ANY)] ^= m.to;
                self[(color, p_prom)] ^= m.to;
                self.occupation ^= m.to;
                zobrist ^= get_zobrist_table(p_prom, color)[m.to.trailing_zeros() as usize];
                zobrist ^= get_zobrist_table(p_cap, color.other())[m.to.trailing_zeros() as usize];
            },
            MoveType::ENPASSANT => {
                let cap_square = match color {
                    Color::WHITE =>  {
                        m.to.go_s()
                    }
                    Color::BLACK => {
                        m.to.go_n()
                    }
                };
                self[(color.other(),Piece::PAWN)] ^= cap_square;
                self[(color.other(),Piece::ANY)] ^= cap_square;
                self.occupation ^= cap_square;
                self[(color, m.piece)] ^= m.to;
                zobrist ^= get_zobrist_table(Piece::PAWN, color)[m.to.trailing_zeros() as usize];
                zobrist ^= get_zobrist_table(Piece::PAWN, color.other())[cap_square.trailing_zeros() as usize];
            },
            MoveType::CASTLE => {
                self[(color, m.piece)] ^= m.to;
                zobrist ^= get_zobrist_table(Piece::KING, color)[m.to.trailing_zeros() as usize];
                match color {
                    Color::WHITE => {
                        if m.to == 1 << 2 {
                            self[(color, Piece::ROOK)] ^= 1+(1<<3);
                            self[(color, Piece::ANY)] ^= 1+(1<<3);
                            self.occupation ^= 1+(1<<3);
                            zobrist ^= get_zobrist_table(Piece::ROOK, Color::WHITE)[0] ^ get_zobrist_table(Piece::ROOK, Color::WHITE)[3];
                        } else if m.to == 1 << 6 {
                            self[(color, Piece::ROOK)] ^= (1<<7)+(1<<5);
                            self[(color, Piece::ANY)] ^= (1<<7)+(1<<5);
                            self.occupation ^= (1<<7)+(1<<5);
                            zobrist ^= get_zobrist_table(Piece::ROOK, Color::WHITE)[5] ^ get_zobrist_table(Piece::ROOK, Color::WHITE)[7];
                        }
                    }
                    Color::BLACK => {
                        if m.to == 1 << 58 {
                            self[(color, Piece::ROOK)] ^= (1<<56)+(1<<59);
                            self[(color, Piece::ANY)] ^= (1<<56)+(1<<59);
                            self.occupation ^= (1<<56)+(1<<59);
                            zobrist ^= get_zobrist_table(Piece::ROOK, Color::BLACK)[56] ^ get_zobrist_table(Piece::ROOK, Color::BLACK)[59];
                        } else if m.to == 1 << 62 {
                            self[(color, Piece::ROOK)] ^= (1<<63)+(1<<61);
                            self[(color, Piece::ANY)] ^= (1<<63)+(1<<61);
                            self.occupation ^= (1<<63)+(1<<61);
                            zobrist ^= get_zobrist_table(Piece::ROOK, Color::BLACK)[61] ^ get_zobrist_table(Piece::ROOK, Color::BLACK)[63];
                        }
                    }
                }
            },
        }
        zobrist
    }
    #[inline(always)]
    pub fn do_move(&mut self, m: Move) -> u64 {
        let color = if self[(Color::WHITE, m.piece)] & m.from != 0 {
            Color::WHITE
        } else {
            Color:: BLACK
        };
        self.do_move_for_color(m,color)
    }
    #[inline(always)]
    pub fn undo_move(&mut self, m: Move) -> u64 {
        let color = if self[(Color::WHITE, Piece::ANY)] & m.to != 0 {
            Color::WHITE
        } else {
            Color:: BLACK
        };
        self.do_move_for_color(m,color)
    }
    fn determine_move_type(&self, from: Square, to: Square, piece: Piece, promote: Option<Piece>) -> MoveType {
        match piece {
            Piece::KING => {
                if piece == Piece::KING && from == 1 << 4 && (to == 1<<6 || to == 1<<2) {
                    return MoveType::CASTLE;
                }
                if piece == Piece::KING && from == 1 << 60 && (to == 1<<62 || to == 1<<58) {
                    return MoveType::CASTLE;
                }

            },
            Piece::PAWN => {
                if self.occupation & to == 0 && Self::get_file(to) != Self::get_file(from) {
                    return MoveType::ENPASSANT;
                } else if promote.is_some() {
                    if self.occupation & to != 0 {
                        return MoveType::PROMOTIONCAPTURE((promote.unwrap(), self.piece_at(to).unwrap()));
                    } else {
                        return MoveType::PROMOTION(promote.unwrap());
                    }
                }
            },
            _ => {},
        }
        if self.occupation & to != 0 {
            MoveType::CAPTURE(self.piece_at(to).unwrap())
        } else {
            MoveType::MOVE
        }
    }
    fn get_zobrist(&self) -> u64 {
        let mut zobrist = 0;
        for p in self[(Color::WHITE, Piece::KING)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_KING_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::WHITE, Piece::QUEEN)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_QUEEN_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::WHITE, Piece::BISHOP)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_BISHOP_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::WHITE, Piece::KNIGHT)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_KNIGHT_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::WHITE, Piece::ROOK)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_ROOK_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::WHITE, Piece::PAWN)].iter() {
            zobrist ^= constants::ZOBRIST_WHITE_PAWN_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::KING)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_KING_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::QUEEN)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_QUEEN_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::BISHOP)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_BISHOP_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::KNIGHT)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_KNIGHT_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::ROOK)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_ROOK_NUMBERS[p.trailing_zeros() as usize];
        }
        for p in self[(Color::BLACK, Piece::PAWN)].iter() {
            zobrist ^= constants::ZOBRIST_BLACK_PAWN_NUMBERS[p.trailing_zeros() as usize];
        }
        zobrist
    }
    pub fn get_neighbours(s: Square) -> Square {
        constants::NEIGHBOURS[s.trailing_zeros() as usize]
    }
}

pub fn square_to_string(sq: Square) -> String{
    const FILES: [char; 8] = ['a','b','c','d','e','f','g','h'];
    const RANKS: [char; 8] = ['1','2','3','4','5','6','7','8'];
    return FILES[(sq.trailing_zeros() % 8) as usize].to_string() +
        RANKS[(sq.trailing_zeros() / 8) as usize].to_string().as_str();
}

fn write_piece_to_position(c: char, pos: Square, cboard: &mut [[char;8];8]) {
        for p in pos.iter() {
            let q = p.trailing_zeros() as usize;
            cboard[7 - (q / 8)][q % 8] = c;
        }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut board: [[char; 8]; 8] = [['.'; 8]; 8];
        write_piece_to_position('K', self[(Color::WHITE, Piece::KING)], &mut board);
        write_piece_to_position('Q', self[(Color::WHITE, Piece::QUEEN)], &mut board);
        write_piece_to_position('R', self[(Color::WHITE, Piece::ROOK)], &mut board);
        write_piece_to_position('B', self[(Color::WHITE, Piece::BISHOP)], &mut board);
        write_piece_to_position('N', self[(Color::WHITE, Piece::KNIGHT)], &mut board);
        write_piece_to_position('P', self[(Color::WHITE, Piece::PAWN)], &mut board);
        write_piece_to_position('k', self[(Color::BLACK, Piece::KING)], &mut board);
        write_piece_to_position('q', self[(Color::BLACK, Piece::QUEEN)], &mut board);
        write_piece_to_position('r', self[(Color::BLACK, Piece::ROOK)], &mut board);
        write_piece_to_position('b', self[(Color::BLACK, Piece::BISHOP)], &mut board);
        write_piece_to_position('n', self[(Color::BLACK, Piece::KNIGHT)], &mut board);
        write_piece_to_position('p', self[(Color::BLACK, Piece::PAWN)], &mut board);
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
    type Output = Square;

    fn index(&self, i: (Color, Piece)) -> &Square {
        &self.positions[i.0 as usize][i.1 as usize]
    }
}

impl std::ops::IndexMut<(Color, Piece)> for Board {
    fn index_mut(&mut self, i: (Color, Piece)) -> &mut Square {
        &mut self.positions[i.0 as usize][i.1 as usize]
    }
}

pub type Square = u64;

pub struct SquareIterator {
    state: u64
}

impl Iterator for SquareIterator {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        if self.state == 0 {
            None
        } else {
            let res = self.state & !(self.state - 1);
            self.state ^= res;
            Some(res)
        }
    }
}

pub trait SquareMethods {
    fn go_n(self) -> Square;
    fn go_s(self) -> Square;
    fn go_w(self) -> Square;
    fn go_e(self) -> Square;
    fn go_nw(self) -> Square;
    fn go_ne(self) -> Square;
    fn go_sw(self) -> Square;
    fn go_se(self) -> Square;
    fn is_at_west_border(self) -> bool;
    fn is_at_east_border(self) -> bool;
    fn is_at_north_border(self) -> bool;
    fn is_at_south_border(self) -> bool;
    fn iter(self) -> SquareIterator;
}

impl SquareMethods for Square {
    #[inline(always)]
    fn go_n(self) -> Square {
        self << 8
    }
    #[inline(always)]
    fn go_s(self) -> Square {
        self >> 8
    }
    #[inline(always)]
    fn go_w(self) -> Square {
        self >> 1
    }
    #[inline(always)]
    fn go_e(self) -> Square {
        self << 1
    }
    #[inline(always)]
    fn go_nw(self) -> Square {
        self << 7
    }
    #[inline(always)]
    fn go_ne(self) -> Square {
        self << 9
    }
    #[inline(always)]
    fn go_se(self) -> Square {
        self >> 7
    }
    #[inline(always)]
    fn go_sw(self) -> Square {
        self >> 9
    }
    #[inline(always)]
    fn is_at_west_border(self) -> bool {
        self & WEST_BORDER != 0
    }
    #[inline(always)]
    fn is_at_east_border(self) -> bool {
        self & EAST_BORDER != 0
    }
    #[inline(always)]
    fn is_at_north_border(self) -> bool {
        self > 1 << 55
    }
    #[inline(always)]
    fn is_at_south_border(self) -> bool {
        self < 1 << 8
    }
    #[inline(always)]
    fn iter(self) -> SquareIterator {
        SquareIterator { state: self }
    }
}

const EAST_BORDER: Square = 0x8080808080808080;
const WEST_BORDER: Square = 0x0101010101010101;

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum MoveType {
    MOVE,
    CAPTURE(Piece),
    PROMOTION(Piece),
    PROMOTIONCAPTURE((Piece,Piece)),
    ENPASSANT,
    CASTLE,
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
        let mut from = 1 << (chars.next().unwrap() as u64 - 'a' as u64);
        from <<= 8*(chars.next().unwrap() as u64 - '1' as u64);
        let mut to = 1 << (chars.next().unwrap() as u64 - 'a' as u64);
        to <<= 8*(chars.next().unwrap() as u64 - '1' as u64);
        let piece = pos.board.piece_at(from).unwrap();
        let prom = match chars.next() {
            Some('q') => Some(Piece::QUEEN),
            Some('r') => Some(Piece::ROOK),
            Some('b') => Some(Piece::BISHOP),
            Some('n') => Some(Piece::KNIGHT),
            _ => None,
        };
        Move {
            from,
            to,
            piece,
            typ: pos.board.determine_move_type(from,to,piece,prom),
        }
    }
    pub fn compress(&self) -> CompressedMove {
        let mut piece_and_type = self.piece as u16;
        piece_and_type |= match self.typ {
            MoveType::MOVE => 0,
            MoveType::CAPTURE(p) => ((p as u16) << 3) | (1 << 9),
            MoveType::PROMOTION(p) => ((p as u16) << 3) | (2 << 9),
            MoveType::PROMOTIONCAPTURE((p,q)) => ((p as u16) << 3) | ((q as u16) << 6) | (3 << 9),
            MoveType::CASTLE => 4 << 9,
            MoveType::ENPASSANT => 5 << 9,
        };
        CompressedMove {
            piece_and_type,
            from: self.from.trailing_zeros() as u8,
            to: self.to.trailing_zeros() as u8,
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
            0 => MoveType::MOVE,
            1 => MoveType::CAPTURE(u16_to_piece((self.piece_and_type >> 3) & 0b111)),
            2 => MoveType::PROMOTION(u16_to_piece((self.piece_and_type >> 3) & 0b111)),
            3 => MoveType::PROMOTIONCAPTURE((u16_to_piece((self.piece_and_type >> 3) & 0b111), u16_to_piece((self.piece_and_type >> 6) & 0b111))),
            4 => MoveType::CASTLE,
            5 => MoveType::ENPASSANT,
            _ => return None,
        };
        Some(Move {from: 1 << self.from as u64, to: 1 << self.to as u64, piece, typ,})
    }
}

fn display_promotion(m: &Move) -> String {
    match m.typ {
        MoveType::PROMOTION(Piece::QUEEN) => "q".to_string(),
        MoveType::PROMOTION(Piece::ROOK) => "r".to_string(),
        MoveType::PROMOTION(Piece::BISHOP) => "b".to_string(),
        MoveType::PROMOTION(Piece::KNIGHT) => "n".to_string(),
        MoveType::PROMOTIONCAPTURE((_,Piece::QUEEN)) => "q".to_string(),
        MoveType::PROMOTIONCAPTURE((_,Piece::ROOK)) => "r".to_string(),
        MoveType::PROMOTIONCAPTURE((_,Piece::BISHOP)) => "b".to_string(),
        MoveType::PROMOTIONCAPTURE((_,Piece::KNIGHT)) => "n".to_string(),
        _ => "".to_string()
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", square_to_string(self.from), square_to_string(self.to),display_promotion(self))
    }
}

fn u16_to_piece(p: u16) -> Piece {
     match p {
        0 => Piece::KING,
        1 => Piece::QUEEN,
        2 => Piece::BISHOP,
        3 => Piece::KNIGHT,
        4 => Piece::ROOK,
        5 => Piece::PAWN,
        _ => panic!("Tried to decompress invalid move."),
    }
}

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum Piece {
    KING,
    QUEEN,
    BISHOP,
    KNIGHT,
    ROOK,
    PAWN,
    ANY
}

impl Piece {
    pub fn value(self) -> i32 {
        match self {
            Self::PAWN => 100,
            Self::BISHOP => 300,
            Self::KNIGHT => 300,
            Self::ROOK => 500,
            Self::QUEEN => 900,
            Self::KING => 10000,
            Self::ANY => 0,
        }
    }
}

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum Color {
    WHITE,
    BLACK,
}

impl Color {
    pub fn other(self) -> Color {
        match self {
            Self::BLACK => Self::WHITE,
            Self::WHITE => Self::BLACK,
        }
    }
}

#[derive(Clone)]
pub struct Position {
    pub board: Board,
    last_move: Option<Move>,
    to_move: Color,
    // castling contains info which types of castling are currently allowed
    //layout [[White Kingside, White Queenside],
    //        [Black Kingside, Black Queenside]]
    castling_legal: [[bool; 2]; 2],
    rule_50_count: u16,
    rep_history: Option<Arc<Position>>,
    attacked_squares: Square,
    king_attackers: Square,
    pinned_pieces: Square,
    zobrist: u64, //Zobrist-Hash
    history: Vec<u64>, //zobrist hashes of all positions reached BEFORE the current
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
            last_move: None,
            to_move: Color::WHITE,
            castling_legal: [[true, true], [true, true]],
            rule_50_count: 0,
            rep_history: None,
            attacked_squares: 0,
            king_attackers: 0,
            pinned_pieces: 0,
            zobrist,
            history: Vec::new(),
        }
    }
    pub fn get_board(&self) -> &Board {
        return &self.board;
    }
    pub fn from_fen(fen: String) -> Option<Position> {
        //First set up the pieces
        let mut fen_parts = fen.split_whitespace();
        let b = Board::from_fen(fen_parts.next().unwrap());
        if b.is_none() {return None;}
        let mut pos = Position {
            board: b.unwrap(),
            last_move: None,
            to_move: Color::WHITE,
            castling_legal: [[false, false], [false, false]],
            rule_50_count: 0,
            rep_history: None,
            attacked_squares: 0,
            king_attackers: 0,
            pinned_pieces: 0,
            zobrist: 0,
            history: Vec::new(),
        };
        pos.zobrist = pos.board.get_zobrist();
        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {pos.to_move = Color::WHITE;}
                else if p == "b" {
                    pos.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
                    pos.to_move = Color::BLACK;
                }
                else {return None;}
            },
            None => return None,
        }
        //Set castling rights
        match fen_parts.next() {
            Some(p) => {
                if p.contains('K') {
                    pos.castling_legal[0][0]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
                }
                if p.contains('Q') {
                    pos.castling_legal[0][1]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
                }
                if p.contains('k') {
                    pos.castling_legal[1][0]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
                }
                if p.contains('q') {
                    pos.castling_legal[1][1]=true;
                    pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
                }
            },
            None => return None,
        }
        //Set en passant if necessary
        match fen_parts.next() {
            Some(p) => {
                match p {
                    "-" => {},
                    "a3" => pos.last_move = Some(Move {
                        from: 1 << 8,
                        to: 1 << 24,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "b3" => pos.last_move = Some(Move {
                        from: 1 << 9,
                        to: 1 << 25,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "c3" => pos.last_move = Some(Move {
                        from: 1 << 10,
                        to: 1 << 26,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "d3" => pos.last_move = Some(Move {
                        from: 1 << 11,
                        to: 1 << 27,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "e3" => pos.last_move = Some(Move {
                        from: 1 << 12,
                        to: 1 << 28,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "f3" => pos.last_move = Some(Move {
                        from: 1 << 13,
                        to: 1 << 29,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "g3" => pos.last_move = Some(Move {
                        from: 1 << 14,
                        to: 1 << 30,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "h3" => pos.last_move = Some(Move {
                        from: 1 << 15,
                        to: 1 << 31,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "a6" => pos.last_move = Some(Move {
                        from: 1 << 48,
                        to: 1 << 32,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "b6" => pos.last_move = Some(Move {
                        from: 1 << 49,
                        to: 1 << 33,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "c6" => pos.last_move = Some(Move {
                        from: 1 << 50,
                        to: 1 << 34,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "d6" => pos.last_move = Some(Move {
                        from: 1 << 51,
                        to: 1 << 35,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "e6" => pos.last_move = Some(Move {
                        from: 1 << 52,
                        to: 1 << 36,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "f6" => pos.last_move = Some(Move {
                        from: 1 << 53,
                        to: 1 << 37,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "g6" => pos.last_move = Some(Move {
                        from: 1 << 54,
                        to: 1 << 38,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    "h6" => pos.last_move = Some(Move {
                        from: 1 << 55,
                        to: 1 << 39,
                        piece: Piece::PAWN,
                        typ: MoveType::ENPASSANT,
                    }),
                    _ => return None,
                }
            },
            None => return None,
        }
        if pos.last_move.is_some() {
            pos.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[pos.last_move.unwrap().from.trailing_zeros() as usize % 8];
        }
        match fen_parts.next() {
            Some(p) => {
                match u16::from_str_radix(p,10) {
                    Ok(n) => pos.rule_50_count = n,
                    Err(_) => return None,
                }
            },
            None => return None,
        }
        Some(pos)
    }
    // Compute the knight moves from a given square using the lookup table
    // Computes the possible moves from the least significant bit in the Square
    #[inline(always)]
    fn knight_moves(&self, sq: Square) -> Square {
        constants::KNIGHT_MOVES[sq.trailing_zeros() as usize]
    }
    // Compute the rook moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline(always)]
    fn rook_moves_for_occupation(&self, sq: Square, occ: Square) -> Square {
        assert!(sq != 0);
        let moves = constants::ROOK_MOVES[sq.trailing_zeros() as usize];
        constants::ROOK_MMASK[(occ.pext(moves) + constants::ROOK_MMASK_OFFSETS[sq.trailing_zeros() as usize]) as usize]
    }
    #[inline(always)]
    fn rook_moves(&self, sq: Square) -> Square {
        self.rook_moves_for_occupation(sq, self.board.occupation)
    }
    // Compute the bishop moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline(always)]
    fn bishop_moves_for_occupation(&self, sq: Square, occ: Square) -> Square {
        assert!(sq != 0);
        let moves = constants::BISHOP_MOVES[sq.trailing_zeros() as usize];
        constants::BISHOP_MMASK[(occ.pext(moves) + constants::BISHOP_MMASK_OFFSETS[sq.trailing_zeros() as usize]) as usize]
    }
    #[inline(always)]
    fn bishop_moves(&self, sq: Square) -> Square {
        self.bishop_moves_for_occupation(sq, self.board.occupation)
    }
    // Compute the king moves from a given square using the lookup table
    #[inline(always)]
    fn king_moves(&self, sq: Square) -> Square {
        assert!(sq != 0);
        constants::NEIGHBOURS[sq.trailing_zeros() as usize]
    }
    #[inline(always)]
    fn pawn_moves(&self, sq: Square, c: Color) -> Square {
        match c {
            Color::WHITE => {
                if self.board.occupation & sq.go_n() != 0 {return 0;}
                //How that happened, we have no clue..
                if sq.is_at_north_border() {0}
                else if sq > (1 << 7) && sq < (1 << 16) && self.board.occupation & (sq << 16) == 0 {
                    (sq << 8) | (sq << 16)
                }
                else {sq << 8}
            },
            Color::BLACK => {
                if self.board.occupation & sq.go_s() != 0 {0}
                else if sq.is_at_south_border() {0}
                else if sq < (1<<56) && sq > (1 << 47) && self.board.occupation & (sq >> 16) == 0 {
                    (sq >> 8) | (sq >> 16)
                }
                else {sq >> 8}
            }
        }
    }
    #[inline(always)]
    fn pawn_attacks(&self, sq: Square, c: Color) -> Square {
        let mut attacked = 0;
        if c == Color::WHITE {
            if !sq.is_at_north_border() && !sq.is_at_west_border() {
                attacked |= sq.go_nw();
            }
            if !sq.is_at_north_border() && !sq.is_at_east_border() {
                attacked |= sq.go_ne();
            }
        }
        if c == Color::BLACK {
            if !sq.is_at_south_border() && !sq.is_at_west_border() {
                attacked |= sq.go_sw();
            }
            if !sq.is_at_south_border() && !sq.is_at_east_border() {
                attacked |= sq.go_se();
            }
        }
        attacked
    }
    #[inline(always)]
    pub fn generate_attack_table(&mut self) {
        let opp = self.to_move.other();
        let king_pos = self.board[(self.to_move, Piece::KING)];
        self.board.occupation ^= king_pos;
        //make sure we cause no collisions..
        self.attacked_squares = 0;
        self.pinned_pieces = 0;
        self.king_attackers = 0;
        //calculate squares attacked by each pawn
        let pawns = self.board[(opp, Piece::PAWN)];
        for pawn in pawns.iter() {
            let attacked = self.pawn_attacks(pawn, opp);
            if attacked & king_pos != 0 {
                self.king_attackers |= pawn;
            }
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by knights
        let knights = self.board[(opp, Piece::KNIGHT)];
        for knight in knights.iter() {
            let attacked = self.knight_moves(knight);
            if attacked & king_pos != 0 {
                self.king_attackers |= knight;
            }
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by king
        self.attacked_squares |= self.king_moves(self.board[(opp, Piece::KING)]);
        //calculate squares attacked by rooks
        let rook_moves_from_king = self.rook_moves(king_pos);
        let rooks = self.board[(opp, Piece::ROOK)];
        for rook in rooks.iter() {
            let attacked = self.rook_moves(rook);
            if attacked & king_pos != 0 {
                self.king_attackers |= rook;
            }
            self.pinned_pieces |= attacked & rook_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [rook.trailing_zeros() as usize];
            self.attacked_squares |= attacked;
        }
        //calculate squares attacked by bishops
        let bish_moves_from_king = self.bishop_moves(king_pos);
        let bishops = self.board[(opp, Piece::BISHOP)];
        for bishop in bishops.iter() {
            let attacked = self.bishop_moves(bishop);
            if attacked & king_pos != 0 {
                self.king_attackers |= bishop;
            }
            self.pinned_pieces |= attacked & bish_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [bishop.trailing_zeros() as usize];
            self.attacked_squares |= attacked;
        }
        let queens = self.board[(opp, Piece::QUEEN)];
        //calculate squares attacked by queens
        for queen in queens.iter() {
            let attacked_r = self.rook_moves(queen);
            let attacked_b = self.bishop_moves(queen);
            if (attacked_r | attacked_b) & king_pos != 0 {
                self.king_attackers |= queen;
            }
            self.pinned_pieces |= attacked_r & rook_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [queen.trailing_zeros() as usize];
            self.pinned_pieces |= attacked_b & bish_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [queen.trailing_zeros() as usize];
            self.attacked_squares |= attacked_b | attacked_r;
        }
        self.board.occupation ^= king_pos;
    }
    //Calculate possible moves of a piece on a given square, using the provide move gen closure
    //
    #[inline(always)]
    fn get_piece_moves(&self, move_getter: impl Fn(Square) -> Square, moves: &mut Vec<Move>, p: Piece) {
        for pos in self.board[(self.to_move, p)].iter() {
            let mut pmoves = move_getter(pos);
            pmoves &= pmoves ^ self.board[(self.to_move, Piece::ANY)];
            if pos & self.pinned_pieces != 0 {
                pmoves &= constants::RAYS[pos.trailing_zeros() as usize][self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize];
            }
            for m in pmoves.iter() {
                moves.push(Move{
                    from: pos,
                    to: m,
                    piece: p,
                    typ: if self.board.occupation & m == 0 {
                        MoveType::MOVE
                    } else {
                        MoveType::CAPTURE(self.board.piece_at(m).unwrap())
                    },
                });
            }
        }
    }
    #[inline(always)]
    fn handle_en_passant(&mut self, moves: &mut Vec<Move>, check_mask: Square) {
        match self.last_move {
            Some(m) => {
                if m.piece == Piece::PAWN {
                    match self.to_move {
                        Color::WHITE => {
                            if m.from > (1 << 47) && m.to < (1 << 40) &&
                                (m.from.go_s() & check_mask != 0 || m.to == check_mask) {
                                let mut cands = 0;
                                if !m.from.is_at_west_border() {
                                    cands |= m.to.go_w();
                                }
                                if !m.from.is_at_east_border() {
                                    cands |= m.to.go_e();
                                }
                                cands &= self.board[(Color::WHITE, Piece::PAWN)];
                                for cand in cands.iter() {
                                    let kpos = self.board[(self.to_move, Piece::KING)];
                                    let pin_ray = constants::RAYS[cand.trailing_zeros() as usize][kpos.trailing_zeros() as usize];
                                    //Check if we expose the king by taking en passant
                                    //See if our pawn is pinned
                                    if cand & self.pinned_pieces != 0 && m.from.go_s() & pin_ray == 0 {
                                        continue;
                                    //See if the pawn we take is pinned, we can only do so, if our
                                    //en passant pawn blocks
                                    } else if m.to & self.pinned_pieces != 0 && constants::RAYS[m.to.trailing_zeros() as usize][kpos.trailing_zeros() as usize] & m.from == 0 {
                                        continue;
                                    //Check for a double pin by a rook or queen.
                                    } else if m.to & pin_ray != 0 {
                                        self.board.occupation ^= m.to | cand;
                                        let k_ray = pin_ray & self.rook_moves(kpos);
                                        self.board.occupation ^= m.to | cand;
                                        if k_ray & self.board[(self.to_move.other(), Piece::ROOK)] != 0 {
                                            continue;
                                        } else if k_ray & self.board[(self.to_move.other(), Piece::QUEEN)] != 0 {
                                            continue;
                                        }
                                    }
                                    moves.push(Move {
                                        from: cand,
                                        to: m.from.go_s(),
                                        piece: Piece::PAWN,
                                        typ: MoveType::ENPASSANT,
                                    });
                                }
                            }
                        },
                        Color::BLACK => {
                            if m.from < (1 << 16) && m.to > (1 << 23) &&
                                (m.from.go_n() & check_mask != 0 || m.to == check_mask) {
                                let mut cands = 0;
                                if !m.from.is_at_west_border() {
                                    cands |= m.to.go_w();
                                }
                                if !m.from.is_at_east_border() {
                                    cands |= m.to.go_e();
                                }
                                cands &= self.board[(Color::BLACK, Piece::PAWN)];
                                for cand in cands.iter() {
                                    let kpos = self.board[(self.to_move, Piece::KING)];
                                    let pin_ray = constants::RAYS[cand.trailing_zeros() as usize][kpos.trailing_zeros() as usize];
                                    //Check if we expose the king by taking en passant
                                    //See if our pawn is pinned
                                    if cand & self.pinned_pieces != 0 && m.from.go_n() & pin_ray == 0 {
                                        continue;
                                    //See if the pawn we take is pinned, we can only do so, if our
                                    //en passant pawn blocks
                                    } else if m.to & self.pinned_pieces != 0 && constants::RAYS[m.to.trailing_zeros() as usize][kpos.trailing_zeros() as usize] & m.from == 0 {
                                        continue;
                                    //Check for a double pin by a rook or queen.
                                    } else if m.to & pin_ray != 0 {
                                        self.board.occupation ^= m.to | cand;
                                        let k_ray = pin_ray & self.rook_moves(kpos);
                                        self.board.occupation ^= m.to | cand;
                                        if k_ray & self.board[(self.to_move.other(), Piece::ROOK)] != 0 {
                                            continue;
                                        } else if k_ray & self.board[(self.to_move.other(), Piece::QUEEN)] != 0 {
                                            continue;
                                        }
                                    }
                                    moves.push(Move {
                                        from: cand,
                                        to: m.from.go_n(),
                                        piece: Piece::PAWN,
                                        typ: MoveType::ENPASSANT,
                                    });
                                }

                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    #[inline(always)]
    fn handle_castling(&self, moves: &mut Vec<Move>) {
        if self.king_attackers.count_ones() != 0 {return;}
        match self.to_move {
            Color::WHITE => {
                const WHITE_KING_CASTLE_MASK: Square  = 0b01100000;
                const WHITE_QUEEN_CASTLE_CHECK_MASK: Square = 0b00001100;
                const WHITE_QUEEN_CASTLE_MATERIAL_MASK: Square = 0b00001110;
                //Check for kingside castling.
                if self.castling_legal[0][0] {
                    if (self.attacked_squares | self.board.occupation) & WHITE_KING_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000,
                            to:   0b01000000,
                            piece: Piece::KING,
                            typ: MoveType::CASTLE,
                        });
                    }
                }
                if self.castling_legal[0][1] {
                    if self.attacked_squares & WHITE_QUEEN_CASTLE_CHECK_MASK == 0 &&
                        self.board.occupation & WHITE_QUEEN_CASTLE_MATERIAL_MASK == 0{
                        moves.push(Move {
                            from: 0b00010000,
                            to:   0b00000100,
                            piece: Piece::KING,
                            typ: MoveType::CASTLE,
                        });
                    }
                }
            },
            Color::BLACK => {
                const BLACK_KING_CASTLE_MASK: Square  = 0b01100000 << 56;
                const BLACK_QUEEN_CASTLE_CHECK_MASK: Square = 0b00001100 << 56;
                const BLACK_QUEEN_CASTLE_MATERIAL_MASK: Square = 0b00001110 << 56;
                //Check for kingside castling.
                if self.castling_legal[1][0] {
                    if (self.attacked_squares | self.board.occupation) & BLACK_KING_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000 << 56,
                            to:   0b01000000 << 56,
                            piece: Piece::KING,
                            typ: MoveType::CASTLE,
                        });
                    }
                }
                if self.castling_legal[1][1] {
                    if self.attacked_squares & BLACK_QUEEN_CASTLE_CHECK_MASK == 0 &&
                        self.board.occupation & BLACK_QUEEN_CASTLE_MATERIAL_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000 << 56,
                            to:   0b00000100 << 56,
                            piece: Piece::KING,
                            typ: MoveType::CASTLE,
                        });
                    }
                }
            },
        }
    }
    #[inline(always)]
    fn get_pawn_moves(&self, moves: &mut Vec<Move>, check_mask: Square) {
            let pawns = self.board[(self.to_move, Piece::PAWN)];
            for pawn in pawns.iter() {
                let mut pmoves = (self.pawn_moves(pawn, self.to_move) |
                                  (self.pawn_attacks(pawn, self.to_move) &
                                   self.board[(self.to_move.other(), Piece::ANY)]))
                                 & check_mask;
                if self.pinned_pieces & pawn != 0 {
                    pmoves &= constants::RAYS[self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize][pawn.trailing_zeros() as usize];
                }
                for m in pmoves.iter() {
                    if (self.to_move == Color::WHITE && m.is_at_north_border()) ||
                    (self.to_move == Color::BLACK && m.is_at_south_border()) {
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::PAWN,
                            typ: if m & self.board.occupation == 0 {
                                MoveType::PROMOTION(Piece::KNIGHT)
                            } else {
                                MoveType::PROMOTIONCAPTURE((Piece::KNIGHT,self.board.piece_at(m).unwrap()))
                            }
                        });
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::PAWN,
                            typ: if m & self.board.occupation == 0 {
                                MoveType::PROMOTION(Piece::BISHOP)
                            } else {
                                MoveType::PROMOTIONCAPTURE((Piece::BISHOP,self.board.piece_at(m).unwrap()))
                            }
                        });
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::PAWN,
                            typ: if m & self.board.occupation == 0 {
                                MoveType::PROMOTION(Piece::ROOK)
                            } else {
                                MoveType::PROMOTIONCAPTURE((Piece::ROOK,self.board.piece_at(m).unwrap()))
                            }
                        });
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::PAWN,
                            typ: if m & self.board.occupation == 0 {
                                MoveType::PROMOTION(Piece::QUEEN)
                            } else {
                                MoveType::PROMOTIONCAPTURE((Piece::QUEEN,self.board.piece_at(m).unwrap()))
                            }
                        });
                    } else {
                        moves.push(Move{
                            from: pawn,
                            to: m,
                            piece: Piece::PAWN,
                            typ: if m & self.board.occupation == 0 {
                                MoveType::MOVE
                            } else {
                                MoveType::CAPTURE(self.board.piece_at(m).unwrap())
                            }
                        });
                    }
                }
            }
    }
    pub fn get_opponent_moves(&mut self) -> Vec<Move> {
        self.to_move = self.to_move.other();
        self.attacked_squares = 0;
        let moves = self.get_moves();
        self.to_move = self.to_move.other();
        self.attacked_squares = 0;
        moves
    }
    pub fn get_moves(&mut self) -> Vec<Move> {
        //We expect about 35 moves in the average position
        let mut moves = Vec::with_capacity(50);
        if self.rule_50_count == 50 || self.board.occupation.count_ones() == 2 {
            return moves;
        }
        if self.attacked_squares == 0 {
            self.generate_attack_table();
        }
        //Generate King moves first (except castling)
        let mut king_moves = self.king_moves(self.board[(self.to_move, Piece::KING)]);
        king_moves &= king_moves ^ self.board[(self.to_move, Piece::ANY)];
        king_moves &= king_moves ^ self.attacked_squares;
        for km in king_moves.iter() {
            moves.push(Move{
                from: self.board[(self.to_move, Piece::KING)],
                to: km,
                piece: Piece::KING,
                typ: if self.board.occupation & km == 0 {
                    MoveType::MOVE
                } else {
                    MoveType::CAPTURE(self.board.piece_at(km).unwrap())
                }
            });
        }
        //optimize double or better check
        if self.king_attackers.count_ones() > 1 {
            return moves
        }
        //If we are in check, we may only take the checker, or block it
        //If the checking piece is a knight, we can only take (or move the king)
        let check_mask = if self.board[(self.to_move.other(), Piece::KNIGHT)]
                            & self.king_attackers != 0 {
            self.king_attackers
        //For any other piece we may also try to block
        } else if self.king_attackers != 0 {
            constants::CONNECTING_RAYS[self.king_attackers.trailing_zeros() as usize][self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize] ^ self.board[(self.to_move, Piece::KING)]
        } else {
            u64::MAX
        };
        //generate queen moves
        self.get_piece_moves(|sq: Square| -> Square
                             {(self.rook_moves(sq) | self.bishop_moves(sq)) & check_mask},
                             &mut moves,
                             Piece::QUEEN);
        //generate rook moves
        self.get_piece_moves(|sq: Square| -> Square {self.rook_moves(sq) & check_mask},
                             &mut moves,
                             Piece::ROOK);
        //generate bishop moves
        self.get_piece_moves(|sq: Square| -> Square {self.bishop_moves(sq) & check_mask},
                             &mut moves,
                             Piece::BISHOP);
        //generate knight moves
        self.get_piece_moves(|sq: Square| -> Square {self.knight_moves(sq) & check_mask},
                             &mut moves,
                             Piece::KNIGHT);
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
        if m.to & (self.board[(Color::WHITE,Piece::KING)] | self.board[(Color::BLACK,Piece::KING)]) != 0 {
            panic!("Invalid move {} in position\n{}\n(previos move {})", m, self.board, self.last_move.unwrap_or(Move {from: 1, to: 2, piece:Piece::KING, typ:MoveType::MOVE}));
        }
        //Commit zobrist hash to history stack
        self.history.push(self.zobrist);
        //Unset the Zobrist en passant flag, if necessary
        if self.last_move.is_some() {
            let m = self.last_move.unwrap();
            if m.piece == Piece::PAWN && (m.from < 1 << 16 || m.from > 1 << 47) && (m.to > 1 << 23 || m.to < 1 << 40) {
                self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.trailing_zeros() as usize % 8];
            }
        }
        self.last_move = Some(m);
        self.zobrist ^= self.board.do_move(m);
        self.attacked_squares = 0;
        if m.piece == Piece::KING {
            if self.castling_legal[self.to_move as usize][0] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2*self.to_move as usize];
            }
            if self.castling_legal[self.to_move as usize][1] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2*self.to_move as usize+1];
            }
            self.castling_legal[self.to_move as usize] = [false,false];
        } else if self.castling_legal[0][1] && (m.to == 1 || m.from == 1) {
            self.castling_legal[0][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
        } else if self.castling_legal[0][0] && (m.to == 1 << 7 || m.from == 1 << 7) {
            self.castling_legal[0][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
        } else if self.castling_legal[1][0] && (m.to == 1 << 63 || m.from == 1 << 63) {
            self.castling_legal[1][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
        } else if self.castling_legal[1][1] && (m.to == 1 << 56 || m.from == 1 << 56) {
            self.castling_legal[1][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
        }
        self.pinned_pieces = 0;
        self.king_attackers = 0;
        self.to_move = self.to_move.other();
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
    }
    pub fn get_castling_rights(&self) -> [[bool;2];2] {
        self.castling_legal
    }
    //TODO: find better solution for castling rights?
    pub fn undo_move(&mut self, m: Move, castling: [[bool;2];2], lm: Option<Move>) {
        //remove move from history stack
        self.history.pop();

        self.last_move = lm;
        self.zobrist ^= self.board.undo_move(m);
        //Reconstruct castling flags
        if self.castling_legal[0][0] ^ castling[0][0] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
        }
        if self.castling_legal[0][1] ^ castling[0][1] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
        }
        if self.castling_legal[1][0] ^ castling[1][0] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
        }
        if self.castling_legal[1][1] ^ castling[1][1] {
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
        }
        self.castling_legal = castling;
        self.attacked_squares = 0;
        self.pinned_pieces = 0;
        self.king_attackers = 0;
        self.to_move = self.to_move.other();
        //switch Zobrist color flag.
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        //Set the Zobrist en passant flag, if necessary
        if self.last_move.is_some() {
            let m = self.last_move.unwrap();
            if m.piece == Piece::PAWN && (m.from < 1 << 16 || m.from > 1 << 47) && (m.to > 1 << 23 || m.to < 1 << 40) {
                self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.trailing_zeros() as usize % 8];
            }
        }
    }
    pub fn do_null_move(&mut self) {
        self.last_move = None;
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        self.to_move = self.to_move.other();
        self.attacked_squares = 0;
        self.pinned_pieces = 0;
        self.king_attackers = 0;
    }
    pub fn undo_null_move(&mut self, lm: Option<Move>) {
        self.last_move = lm;
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        self.to_move = self.to_move.other();
        self.attacked_squares = 0;
        self.pinned_pieces = 0;
        self.king_attackers = 0;
    }
    pub fn in_check(&mut self) -> bool {
        if self.attacked_squares == 0 {
            self.generate_attack_table();
        }
        self.king_attackers != 0
    }
    pub fn piece_count(&self, c: Color, p: Piece) -> i32 {
        self.board[(c,p)].count_ones() as i32
    }
    pub fn total_piece_count(&self) -> i32 {
        self.board.occupation.count_ones() as i32
    }
    fn material_count(&self, c: Color) -> i32 {
        self.piece_count(c, Piece::PAWN) + 3*(self.piece_count(c, Piece::BISHOP)+self.piece_count(c,Piece::KNIGHT)) + 5*self.piece_count(c, Piece::ROOK)+9*self.piece_count(c, Piece::QUEEN)
    }
    pub fn material_balance(&self) -> i32 {
        return self.material_count(self.to_move) - self.material_count(self.to_move.other());
    }
    pub fn is_attacked(&self, sq: Square) -> bool {
        return self.attacked_squares & sq != 0;
    }
    pub fn piece_attacks(&self, p: Piece, c: Color, from: Square, target: Square) -> bool {
        match p {
            Piece::PAWN => self.pawn_attacks(from, c) & target != 0,
            Piece::KNIGHT => self.knight_moves(from) & target != 0,
            Piece::BISHOP => self.bishop_moves(from) & target != 0,
            Piece::ROOK => self.rook_moves(from) & target != 0,
            Piece::QUEEN => (self.rook_moves(from) | self.bishop_moves(from)) != 0,
            //TODO: King..
            _ => false,
        }
    }
    pub fn get_last_move(&self) -> Option<Move> {
        self.last_move
    }
    pub fn zobrist_hash(&self) -> u64 {
        self.zobrist
    }
    pub fn color_factor(&self) -> i32 {
        match self.to_move {
            Color::WHITE => 1,
            Color::BLACK => -1,
        }
    }
    pub fn hard_pins(&mut self) -> Square {
        if self.attacked_squares == 0 {
            self.generate_attack_table();
        }
        self.pinned_pieces
    }
    //Switch color for analysis
    //Does NOT update Zobrist hash
    pub fn switch_color(&mut self) {
        self.to_move = self.to_move.other();
        self.attacked_squares = 0;
    }
    //TODO: Should this _really_ be here? But where else to put it?
    //Helper for SEE
    #[inline(always)]
    fn least_valuable_attacker(&self, mut attackers: Square, c: Color) -> Option<(Square, Piece)> {
        attackers = attackers & self.board[(c, Piece::ANY)];
        attackers.iter().map(|a| match self.board.piece_at(a) {
                                            Some(p) => Some((a,p)),
                                            None => None,
                        })
                        .filter(|x| x.is_some())
                        .min_by_key(|x| x.unwrap().1.value()).flatten()
    }
    //X-ray attacks.
    //We ignore en passant here! Attackers are sorted by value!
    pub fn see(&self, m: Move) -> i32 {
        let target_piece = match m.typ {
            MoveType::CAPTURE(p) => p,
            _ => return 0,
        };
        let target = m.to;
        let mut color = self.to_move;
        let mut occupation = self.board.occupation;
        let mut attackers = (self.pawn_attacks(target, Color::WHITE) & self.board[(Color::BLACK, Piece::PAWN)])
            | (self.pawn_attacks(target, Color::WHITE) & self.board[(Color::BLACK, Piece::PAWN)]);
        attackers |= self.knight_moves(target) & (self.board[(Color::BLACK, Piece::KNIGHT)] | self.board[(Color::BLACK, Piece::KNIGHT)]);
        attackers |= m.from;
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
            occupation ^= from;
            attackers ^= from;
            color = color.other();
            attackers |= self.bishop_moves_for_occupation(target, occupation)
                        & (self.board[(Color::BLACK, Piece::BISHOP)] | self.board[(Color::WHITE, Piece::BISHOP)]
                          | self.board[(Color::BLACK, Piece::QUEEN)] | self.board[(Color::WHITE, Piece::QUEEN)])
                        & occupation;
            attackers |= self.rook_moves_for_occupation(target, occupation)
                        & (self.board[(Color::BLACK, Piece::ROOK)] | self.board[(Color::WHITE, Piece::ROOK)]
                          | self.board[(Color::BLACK, Piece::QUEEN)] | self.board[(Color::WHITE, Piece::QUEEN)])
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
    pub fn pos_in_history(&self) -> bool {
        self.history.contains(&self.zobrist)
    }
}

//Tests
#[test]
fn simple_sse() {
    let pos = Position::from_fen(String::from("1k1r4/1pp4p/p7/4p3/8/P5P1/1PP4P/2K1R3 w - - 0 0")).unwrap();
    let mov = Move {from: 1 << 4, to: 1 << 36, typ: MoveType::CAPTURE(Piece::PAWN), piece: Piece::ROOK};
    assert!(pos.see(mov) == 100);
    let pos2 = Position::from_fen(String::from("1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 0")).unwrap();
    let mov2 = Move {from: 1 << 19, to: 1 << 36, typ: MoveType::CAPTURE(Piece::PAWN), piece: Piece::KNIGHT};
    assert!(pos2.see(mov2) == -200);
}

#[test]
fn compress_and_decompress_move() {
    let pieces = vec![Piece::PAWN, Piece::KING, Piece::QUEEN, Piece::BISHOP, Piece::KNIGHT, Piece::ROOK];
    for p in pieces.iter() {
        let m = Move {from: 1, to: 2, piece: *p, typ: MoveType::MOVE};
        let m2 = m.compress().decompress();
        assert!(m == m2.unwrap());
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {from:1<<54,to:1<<63,piece: Piece::PAWN, typ: MoveType::PROMOTIONCAPTURE((*p,*q))};
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {from:1<<54,to:1<<63,piece: *p, typ: MoveType::CAPTURE(*q)};
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
}
