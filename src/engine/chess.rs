use std::sync::Arc;
use bitintr::Pext;
use std::fmt;

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
    positions: [[u64; 7]; 2],
    occupation: u64,
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

impl Board {
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
    pub fn do_move(&mut self, m: Move) {
        let color = if self[(Color::WHITE, m.piece)] & m.from != 0 {
            Color::WHITE
        } else {
            Color:: BLACK
        };
        let target_piece = m.promote.unwrap_or(m.piece);
        self[(color, m.piece)] &= self[(color, m.piece)] ^ m.from;
        self[(color, Piece::ANY)] &= self[(color, Piece::ANY)] ^ m.from;
        self[(color, target_piece)] |= m.to;
        self[(color, Piece::ANY)] |= m.to;
        self.occupation &= self.occupation ^ m.from;
        self.occupation |= m.to;
        //remove capture if necessary
        for i in 0..7 {
            self.positions[OTHER_COLOR[color as usize] as usize][i] &=
                self.positions[OTHER_COLOR[color as usize] as usize][i] ^ m.to;
        }
        //deal with castling
        if m.piece == Piece::KING && m.from == 1 << 4 {
            if m.to == 1 << 6 {
                self[(color, Piece::ROOK)] &= self[(color, Piece::ROOK)] ^ (1 << 7);
                self[(color, Piece::ROOK)] |= 1 << 5;
                self[(color, Piece::ANY)] &= self[(color, Piece::ANY)] ^ (1 << 7);
                self[(color, Piece::ANY)] |= 1 << 5;
                self.occupation &= self.occupation ^ (1 << 7);
                self.occupation |= 1 << 5;
            } else if m.to == 1 << 2 {
                self[(color, Piece::ROOK)] &= self[(color, Piece::ROOK)] ^ 1;
                self[(color, Piece::ROOK)] |= 1 << 3;
                self[(color, Piece::ANY)] &= self[(color, Piece::ANY)] ^ 1;
                self[(color, Piece::ANY)] |= 1 << 3;
                self.occupation &= self.occupation ^ 1;
                self.occupation |= 1 << 3;
            }
        } else if m.piece == Piece::KING && m.from == 1 << 60 {
            if m.to == 1 << 62 {
                self[(color, Piece::ROOK)] &= self[(color, Piece::ROOK)] ^ (1 << 63);
                self[(color, Piece::ROOK)] |= 1 << 61;
                self[(color, Piece::ANY)] &= self[(color, Piece::ANY)] ^ (1 << 63);
                self[(color, Piece::ANY)] |= 1 << 61;
                self.occupation &= self.occupation ^ (1 << 63);
                self.occupation |= 1 << 61;
            } else if m.to == 1 << 58 {
                self[(color, Piece::ROOK)] &= self[(color, Piece::ROOK)] ^ (1 << 56);
                self[(color, Piece::ROOK)] |= 1 << 59;
                self[(color, Piece::ANY)] &= self[(color, Piece::ANY)] ^ (1 << 56);
                self[(color, Piece::ANY)] |= 1 << 59;
                self.occupation &= self.occupation ^ (1 << 56);
                self.occupation |= 1 << 59;
            }
        }
        //deal with enpassant
        if m.piece == Piece::PAWN && m.to > 1 << 39 && m.to < 1 << 48 &&
            (m.to == m.from << 7 || m.to == m.from << 9) &&
            self[(OTHER_COLOR[color as usize], Piece::ANY)] & m.to == 0 {
                    self[(OTHER_COLOR[color as usize], Piece::PAWN)] &=
                        self[(OTHER_COLOR[color as usize], Piece::PAWN)] ^ m.to.go_s();
                    self[(OTHER_COLOR[color as usize], Piece::ANY)] &=
                        self[(OTHER_COLOR[color as usize], Piece::ANY)] ^ m.to.go_s();
                    self.occupation &= self.occupation ^ m.to.go_s();
        }
        if m.piece == Piece::PAWN && m.to > 1 << 15 && m.to < 1 << 24 &&
            (m.to == m.from >> 7 || m.to == m.from >> 9) &&
            self[(OTHER_COLOR[color as usize], Piece::ANY)] & m.to == 0 {
                    self[(OTHER_COLOR[color as usize], Piece::PAWN)] &=
                        self[(OTHER_COLOR[color as usize], Piece::PAWN)] ^ m.to.go_n();
                    self[(OTHER_COLOR[color as usize], Piece::ANY)] &=
                        self[(OTHER_COLOR[color as usize], Piece::ANY)] ^ m.to.go_n();
                    self.occupation &= self.occupation ^ m.to.go_n();
        }
    }
}

fn write_piece_to_position(p: char, mut pos: Square, cboard: &mut [[char;8];8]) {
        while pos != 0 {
            let q = pos.trailing_zeros() as usize;
            cboard[7 - (q / 8)][q % 8] = p;
            pos &= pos - 1;
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
//#[derive(PartialEq,PartialOrd,Clone,Copy)]
// we index by 2^pos, generally
pub type Square = u64;

trait SquareMethods {
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
}

const EAST_BORDER: Square = 0x8080808080808080;
const WEST_BORDER: Square = 0x0101010101010101;

#[derive(PartialEq,Clone,Copy)]
pub struct Move {
    pub piece: Piece,
    pub from: Square,
    pub to: Square,
    pub promote: Option<Piece>,
}

#[derive(PartialEq,Clone,Copy)]
pub enum Piece {
    KING,
    QUEEN,
    BISHOP,
    KNIGHT,
    ROOK,
    PAWN,
    ANY
}

#[derive(PartialEq,Clone,Copy,Debug)]
pub enum Color {
    WHITE,
    BLACK,
}

const OTHER_COLOR: [Color; 2] = [Color::BLACK, Color::WHITE];

#[derive(Clone)]
pub struct Position {
    board: Board,
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
}

impl Position {
    pub fn new() -> Position {
        Position {
            board: Board::new(),
            last_move: None,
            to_move: Color::WHITE,
            castling_legal: [[true, true], [true, true]],
            rule_50_count: 0,
            rep_history: None,
            attacked_squares: 0,
            king_attackers: 0,
            pinned_pieces: 0
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
            pinned_pieces: 0
        };
        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {pos.to_move = Color::WHITE;}
                else if p == "b" {pos.to_move = Color::BLACK;}
                else {return None;}
            },
            None => return None,
        }
        //Set castling rights
        match fen_parts.next() {
            Some(p) => {
                if p.contains('K') {pos.castling_legal[0][0]=true;}
                if p.contains('Q') {pos.castling_legal[0][1]=true;}
                if p.contains('k') {pos.castling_legal[1][0]=true;}
                if p.contains('q') {pos.castling_legal[1][1]=true;}
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
                        promote: None,
                    }),
                    "b3" => pos.last_move = Some(Move {
                        from: 1 << 9,
                        to: 1 << 25,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "c3" => pos.last_move = Some(Move {
                        from: 1 << 10,
                        to: 1 << 26,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "d3" => pos.last_move = Some(Move {
                        from: 1 << 11,
                        to: 1 << 27,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "e3" => pos.last_move = Some(Move {
                        from: 1 << 12,
                        to: 1 << 28,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "f3" => pos.last_move = Some(Move {
                        from: 1 << 13,
                        to: 1 << 29,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "g3" => pos.last_move = Some(Move {
                        from: 1 << 14,
                        to: 1 << 30,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "h3" => pos.last_move = Some(Move {
                        from: 1 << 15,
                        to: 1 << 31,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "a6" => pos.last_move = Some(Move {
                        from: 1 << 48,
                        to: 1 << 32,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "b6" => pos.last_move = Some(Move {
                        from: 1 << 49,
                        to: 1 << 33,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "c6" => pos.last_move = Some(Move {
                        from: 1 << 50,
                        to: 1 << 34,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "d6" => pos.last_move = Some(Move {
                        from: 1 << 51,
                        to: 1 << 35,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "e6" => pos.last_move = Some(Move {
                        from: 1 << 52,
                        to: 1 << 36,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "f6" => pos.last_move = Some(Move {
                        from: 1 << 53,
                        to: 1 << 37,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "g6" => pos.last_move = Some(Move {
                        from: 1 << 54,
                        to: 1 << 38,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    "h6" => pos.last_move = Some(Move {
                        from: 1 << 55,
                        to: 1 << 39,
                        piece: Piece::PAWN,
                        promote: None,
                    }),
                    _ => return None,
                }
            },
            None => return None,
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
    fn rook_moves(&self, sq: Square) -> Square {
        if (sq == 0) {
            println!("{}", self.board);
        }
        assert!(sq != 0);
        let moves = constants::ROOK_MOVES[sq.trailing_zeros() as usize];
        constants::ROOK_MMASK[((self.board.occupation).pext(moves) + constants::ROOK_MMASK_OFFSETS[sq.trailing_zeros() as usize]) as usize]
    }
    // Compute the bishop moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline(always)]
    fn bishop_moves(&self, sq: Square) -> Square {
        if (sq == 0) {
            println!("{}", self.board);
        }
        assert!(sq != 0);
        let moves = constants::BISHOP_MOVES[sq.trailing_zeros() as usize];
        constants::BISHOP_MMASK[((self.board.occupation).pext(moves) + constants::BISHOP_MMASK_OFFSETS[sq.trailing_zeros() as usize]) as usize]
    }
    // Compute the king moves from a given square using the lookup table
    #[inline(always)]
    fn king_moves(&self, sq: Square) -> Square {
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
    pub fn generate_attack_table(&mut self) {
        let opp = OTHER_COLOR[self.to_move as usize];
        let king_pos = self.board[(self.to_move, Piece::KING)];
        self.board.occupation ^= king_pos;
        //make sure we cause no collisions..
        self.attacked_squares = 0;
        self.pinned_pieces = 0;
        //calculate squares attacked by each pawn
        let mut pawns = self.board[(opp, Piece::PAWN)];
        while pawns != 0 {
            let attacked = self.pawn_attacks(pawns & !(pawns-1), opp);
            if attacked & king_pos != 0 {
                self.king_attackers |= pawns & !(pawns - 1);
            }
            self.attacked_squares |= attacked;
            pawns &= pawns - 1;
        }
        //calculate squares attacked by knights
        let mut knights = self.board[(opp, Piece::KNIGHT)];
        while knights != 0 {
            let attacked = self.knight_moves(knights);
            if attacked & king_pos != 0 {
                self.king_attackers |= knights & !(knights - 1)
            }
            self.attacked_squares |= attacked;
            knights &= knights - 1;
        }
        //calculate squares attacked by king
        self.attacked_squares |= self.king_moves(self.board[(opp, Piece::KING)]);
        //calculate squares attacked by rooks
        let rook_moves_from_king = self.rook_moves(king_pos);
        let mut rooks = self.board[(opp, Piece::ROOK)];
        while rooks != 0 {
            let attacked = self.rook_moves(rooks);
            if attacked & king_pos != 0 {
                self.king_attackers |= rooks & !(rooks -1);
            }
            self.pinned_pieces |= attacked & rook_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [rooks.trailing_zeros() as usize];
            self.attacked_squares |= attacked;
            rooks &= rooks - 1;
        }
        //calculate squares attacked by bishops
        let bish_moves_from_king = self.bishop_moves(king_pos);
        let mut bishops = self.board[(opp, Piece::BISHOP)];
        while bishops != 0 {
            let attacked = self.bishop_moves(bishops);
            if attacked & king_pos != 0 {
                self.king_attackers |= bishops & !(bishops - 1);
            }
            self.pinned_pieces |= attacked & bish_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [bishops.trailing_zeros() as usize];
            self.attacked_squares |= attacked;
            bishops &= bishops - 1;
        }
        let mut queens = self.board[(opp, Piece::QUEEN)];
        //calculate squares attacked by queens
        while queens != 0 {
            let attacked_r = self.rook_moves(queens);
            let attacked_b = self.bishop_moves(queens);
            if (attacked_r | attacked_b) & king_pos != 0 {
                self.king_attackers |= queens & !(queens - 1);
            }
            self.pinned_pieces |= attacked_r & rook_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [queens.trailing_zeros() as usize];
            self.pinned_pieces |= attacked_b & bish_moves_from_king
                                           & self.board.occupation
                                           & constants::RAYS[king_pos.trailing_zeros() as usize]
                                                            [queens.trailing_zeros() as usize];
            self.attacked_squares |= attacked_b | attacked_r;
            queens &= queens - 1;
        }
        self.board.occupation ^= king_pos;
    }
    //Calculate possible moves of a piece on a given square, using the provide move gen closure
    //
    #[inline(always)]
    fn get_piece_moves(&self, move_getter: impl Fn(Square) -> Square, moves: &mut Vec<Move>, p: Piece) {
        let mut pos = self.board[(self.to_move, p)];
        while pos != 0 {
            let mut pmoves = move_getter(pos);
            pmoves &= pmoves ^ self.board[(self.to_move, Piece::ANY)];
            if pos & !(pos-1) & self.pinned_pieces != 0 {
                pmoves &= constants::RAYS[pos.trailing_zeros() as usize][self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize];
            }
            while pmoves != 0 {
                moves.push(Move{
                    from: pos & !(pos - 1),
                    to: pmoves & !(pmoves - 1),
                    piece: p,
                    promote: None,
                });
                pmoves &= pmoves - 1;
            }
            pos &= pos - 1;
        }
    }
    #[inline(always)]
    fn handle_en_passant(&mut self, moves: &mut Vec<Move>, check_mask: Square) {
        match self.last_move {
            Some(m) => {
                if m.piece == Piece::PAWN {
                    match self.to_move {
                        Color::WHITE => {
                            if m.from > (1 << 47) && m.to < (1 << 40) && (m.from.go_s()) & check_mask != 0{
                                let mut cands = 0;
                                if !m.from.is_at_west_border() {
                                    cands |= m.to.go_w();
                                }
                                if !m.from.is_at_east_border() {
                                    cands |= m.to.go_e();
                                }
                                cands &= self.board[(Color::WHITE, Piece::PAWN)];
                                while cands != 0 {
                                    let cand = cands & !(cands-1);
                                    let kpos = self.board[(self.to_move, Piece::KING)];
                                    let pin_ray = constants::RAYS[cand.trailing_zeros() as usize][kpos.trailing_zeros() as usize];
                                    //Check if we expose the king by taking en passant
                                    //See if our pawn is pinned
                                    if cand & self.pinned_pieces != 0 && m.from.go_s() & pin_ray == 0 {
                                        cands &= cands - 1;
                                        continue;
                                    //See if the pawn we take is pinned, we can only do so, if our
                                    //en passant pawn blocks
                                    } else if m.to & self.pinned_pieces != 0 && constants::RAYS[m.to.trailing_zeros() as usize][kpos.trailing_zeros() as usize] & m.from == 0 {
                                        cands &= cands - 1;
                                        continue;
                                    //Check for a double pin by a rook or queen.
                                    } else if m.to & pin_ray != 0 {
                                        self.board.occupation ^= m.to | cand;
                                        let k_ray = pin_ray & self.rook_moves(kpos);
                                        self.board.occupation ^= m.to | cand;
                                        if k_ray & self.board[(OTHER_COLOR[self.to_move as usize], Piece::ROOK)] != 0 {
                                            cands &= cands - 1;
                                            continue;
                                        } else if k_ray & self.board[(OTHER_COLOR[self.to_move as usize], Piece::QUEEN)] != 0 {
                                            cands &= cands - 1;
                                            continue;
                                        }
                                    }
                                    moves.push(Move {
                                        from: cand,
                                        to: m.from.go_s(),
                                        piece: Piece::PAWN,
                                        promote: None
                                    });
                                    cands &= cands - 1;
                                }
                            }
                        },
                        Color::BLACK => {
                            if m.from < (1 << 16) && m.to > (1 << 23) && (m.from.go_n()) & check_mask != 0 {
                                let mut cands = 0;
                                if !m.from.is_at_west_border() {
                                    cands |= m.to.go_w();
                                }
                                if !m.from.is_at_east_border() {
                                    cands |= m.to.go_e();
                                }
                                cands &= self.board[(Color::BLACK, Piece::PAWN)];
                                while cands != 0 {
                                    let cand = cands & !(cands-1);
                                    let kpos = self.board[(self.to_move, Piece::KING)];
                                    let pin_ray = constants::RAYS[cand.trailing_zeros() as usize][kpos.trailing_zeros() as usize];
                                    //Check if we expose the king by taking en passant
                                    //See if our pawn is pinned
                                    if cand & self.pinned_pieces != 0 && m.from.go_n() & pin_ray == 0 {
                                        cands &= cands - 1;
                                        continue;
                                    //See if the pawn we take is pinned, we can only do so, if our
                                    //en passant pawn blocks
                                    } else if m.to & self.pinned_pieces != 0 && constants::RAYS[m.to.trailing_zeros() as usize][kpos.trailing_zeros() as usize] & m.from == 0 {
                                        cands &= cands - 1;
                                        continue;
                                    //Check for a double pin by a rook or queen.
                                    } else if m.to & pin_ray != 0 {
                                        self.board.occupation ^= m.to | cand;
                                        let k_ray = pin_ray & self.rook_moves(kpos);
                                        self.board.occupation ^= m.to | cand;
                                        if k_ray & self.board[(OTHER_COLOR[self.to_move as usize], Piece::ROOK)] != 0 {
                                            cands &= cands - 1;
                                            continue;
                                        } else if k_ray & self.board[(OTHER_COLOR[self.to_move as usize], Piece::QUEEN)] != 0 {
                                            cands &= cands - 1;
                                            continue;
                                        }
                                    }
                                    moves.push(Move {
                                        from: cand,
                                        to: m.from.go_n(),
                                        piece: Piece::PAWN,
                                        promote: None
                                    });
                                    cands &= cands - 1;
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
                const WHITE_QUEEN_CASTLE_MASK: Square = 0b00001100;
                //Check for kingside castling.
                if self.castling_legal[0][0] {
                    if (self.attacked_squares | self.board.occupation) & WHITE_KING_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000,
                            to:   0b01000000,
                            piece: Piece::KING,
                            promote: None,
                        });
                    }
                }
                if self.castling_legal[0][1] {
                    if (self.attacked_squares | self.board.occupation) & WHITE_QUEEN_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000,
                            to:   0b00000100,
                            piece: Piece::KING,
                            promote: None,
                        });
                    }
                }
            },
            Color::BLACK => {
                const BLACK_KING_CASTLE_MASK: Square  = 0b01100000 << 56;
                const BLACK_QUEEN_CASTLE_MASK: Square = 0b00001100 << 56;
                //Check for kingside castling.
                if self.castling_legal[1][0] {
                    if (self.attacked_squares | self.board.occupation) & BLACK_KING_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000 << 56,
                            to:   0b01000000 << 56,
                            piece: Piece::KING,
                            promote: None,
                        });
                    }
                }
                if self.castling_legal[1][1] {
                    if (self.attacked_squares | self.board.occupation) & BLACK_QUEEN_CASTLE_MASK == 0 {
                        moves.push(Move {
                            from: 0b00010000 << 56,
                            to:   0b00000100 << 56,
                            piece: Piece::KING,
                            promote: None,
                        });
                    }
                }
            },
        }
    }
    #[inline(always)]
    fn get_pawn_moves(&self, moves: &mut Vec<Move>, check_mask: Square) {
            let mut pawns = self.board[(self.to_move, Piece::PAWN)];
            while pawns != 0 {
                let pawn = pawns & !(pawns - 1);
                let mut pmoves = (self.pawn_moves(pawn, self.to_move) |
                                  (self.pawn_attacks(pawn, self.to_move) &
                                   self.board[(OTHER_COLOR[self.to_move as usize], Piece::ANY)]))
                                 & check_mask;
                if self.pinned_pieces & pawn != 0 {
                    pmoves &= constants::RAYS[self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize][pawn.trailing_zeros() as usize];
                }
                while pmoves != 0 {
                    if (self.to_move == Color::WHITE && (pmoves & !(pmoves - 1)).is_at_north_border()) ||
                    (self.to_move == Color::BLACK && (pmoves & !(pmoves - 1)).is_at_south_border()) {
                        moves.push(Move{
                            from: pawn,
                            to: pmoves & !(pmoves - 1),
                            piece: Piece::PAWN,
                            promote: Some(Piece::KNIGHT),
                        });
                        moves.push(Move{
                            from: pawn,
                            to: pmoves & !(pmoves - 1),
                            piece: Piece::PAWN,
                            promote: Some(Piece::BISHOP),
                        });
                        moves.push(Move{
                            from: pawn,
                            to: pmoves & !(pmoves - 1),
                            piece: Piece::PAWN,
                            promote: Some(Piece::ROOK),
                        });
                        moves.push(Move{
                            from: pawn,
                            to: pmoves & !(pmoves - 1),
                            piece: Piece::PAWN,
                            promote: Some(Piece::QUEEN),
                        });
                    } else {
                        moves.push(Move{
                            from: pawn,
                            to: pmoves & !(pmoves - 1),
                            piece: Piece::PAWN,
                            promote: None
                        });
                    }
                    pmoves &= pmoves - 1;
                }
                pawns &= pawns - 1;
            }
    }
    pub fn get_moves(&mut self) -> Vec<Move> {
        let mut moves = Vec::new();
        if self.attacked_squares == 0 {
            self.generate_attack_table();
        }
        //Generate King moves first (except castling)
        let mut king_moves = self.king_moves(self.board[(self.to_move, Piece::KING)]);
        king_moves &= king_moves ^ self.board[(self.to_move, Piece::ANY)];
        king_moves &= king_moves ^ self.attacked_squares;
        while king_moves != 0 {
            moves.push(Move{
                from: self.board[(self.to_move, Piece::KING)],
                to: king_moves & !(king_moves - 1),
                piece: Piece::KING,
                promote: None
            });
            king_moves &= king_moves - 1;
        }
        //optimize double or better check
        if self.king_attackers.count_ones() > 1 {
            return moves
        }
        //If we are in check, we may only take the checker, or block it
        //If the checking piece is a knight, we can only take (or move the king)
        let check_mask = if self.board[(OTHER_COLOR[self.to_move as usize], Piece::KNIGHT)]
                            & self.king_attackers != 0 {
            self.king_attackers
        //For any other piece we may also try to block
        } else if self.king_attackers != 0 {
            constants::CONNECTING_RAYS[self.king_attackers.trailing_zeros() as usize][self.board[(self.to_move, Piece::KING)].trailing_zeros() as usize]
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
    pub fn do_move(&self, m: Move) -> Position {
        if m.to & self.board[(OTHER_COLOR[self.to_move as usize], Piece::KING)] != 0 {
            println!("{}",self.board);
            println!("Illegal Move: {}: {} -> {}", m.piece as u8, m.from, m.to);
            println!("Last Move: {}: {} -> {}", self.last_move.unwrap().piece as u8, self.last_move.unwrap().from, self.last_move.unwrap().to);
            println!("{:064b}", self.attacked_squares);
        }
        let mut pos = self.clone();
        pos.last_move = Some(m);
        if m.piece == Piece::PAWN ||
            m.to & pos.board[(OTHER_COLOR[self.to_move as usize], Piece::ANY)] != 0 {
            pos.rule_50_count = 0;
        } else {
            pos.rule_50_count += 1;
        }
        pos.to_move = OTHER_COLOR[self.to_move as usize];
        pos.board.do_move(m);
        pos.attacked_squares = 0;
        if m.piece == Piece::KING {
            pos.castling_legal[self.to_move as usize] = [false,false];
        } else if m.piece == Piece::ROOK{
            if m.from == 1 || m.from == 1 << 56 {
                pos.castling_legal[self.to_move as usize][1] = false;
            } else if m.from == 1 << 63 || m.from == 1 << 7 {
                pos.castling_legal[self.to_move as usize][0] = false;
            }
        }
        pos.pinned_pieces = 0;
        pos.king_attackers = 0;
        pos
    }
}
