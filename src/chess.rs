use std::fmt;
use std::iter::Iterator;

pub use crate::bitboard::{BitBoard, File, Rank, Square};
pub use crate::board::Board;

mod constants;

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
fn get_zobrist_number(p: Piece, c: Color, sq: Square) -> u64 {
    match c {
        Color::White => match p {
            Piece::King => constants::ZOBRIST_WHITE_KING_NUMBERS[sq],
            Piece::Queen => constants::ZOBRIST_WHITE_QUEEN_NUMBERS[sq],
            Piece::Bishop => constants::ZOBRIST_WHITE_BISHOP_NUMBERS[sq],
            Piece::Knight => constants::ZOBRIST_WHITE_KNIGHT_NUMBERS[sq],
            Piece::Rook => constants::ZOBRIST_WHITE_ROOK_NUMBERS[sq],
            Piece::Pawn => constants::ZOBRIST_WHITE_PAWN_NUMBERS[sq],
            Piece::Any => panic!("Invalid Piece"),
        },
        Color::Black => match p {
            Piece::King => constants::ZOBRIST_BLACK_KING_NUMBERS[sq],
            Piece::Queen => constants::ZOBRIST_BLACK_QUEEN_NUMBERS[sq],
            Piece::Bishop => constants::ZOBRIST_BLACK_BISHOP_NUMBERS[sq],
            Piece::Knight => constants::ZOBRIST_BLACK_KNIGHT_NUMBERS[sq],
            Piece::Rook => constants::ZOBRIST_BLACK_ROOK_NUMBERS[sq],
            Piece::Pawn => constants::ZOBRIST_BLACK_PAWN_NUMBERS[sq],
            Piece::Any => panic!("Invalid Piece"),
        },
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
                    return MoveType::PromotionCapture((
                        promote.unwrap(),
                        board.piece_at(to).unwrap(),
                    ));
                } else {
                    return MoveType::Promotion(promote.unwrap());
                }
            }
        }
        _ => {}
    }
    if board.occupation().is_set(to) {
        MoveType::Capture(board.piece_at(to).unwrap())
    } else {
        MoveType::Normal
    }
}

fn get_zobrist(board: &Board) -> u64 {
    let mut zobrist: u64 = 0;
    for p in board.get_bb(Color::White, Piece::King) {
        zobrist ^= constants::ZOBRIST_WHITE_KING_NUMBERS[p];
    }
    for p in board.get_bb(Color::White, Piece::Queen) {
        zobrist ^= constants::ZOBRIST_WHITE_QUEEN_NUMBERS[p];
    }
    for p in board.get_bb(Color::White, Piece::Bishop) {
        zobrist ^= constants::ZOBRIST_WHITE_BISHOP_NUMBERS[p];
    }
    for p in board.get_bb(Color::White, Piece::Knight) {
        zobrist ^= constants::ZOBRIST_WHITE_KNIGHT_NUMBERS[p];
    }
    for p in board.get_bb(Color::White, Piece::Rook) {
        zobrist ^= constants::ZOBRIST_WHITE_ROOK_NUMBERS[p];
    }
    for p in board.get_bb(Color::White, Piece::Pawn) {
        zobrist ^= constants::ZOBRIST_WHITE_PAWN_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::King) {
        zobrist ^= constants::ZOBRIST_BLACK_KING_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::Queen) {
        zobrist ^= constants::ZOBRIST_BLACK_QUEEN_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::Bishop) {
        zobrist ^= constants::ZOBRIST_BLACK_BISHOP_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::Knight) {
        zobrist ^= constants::ZOBRIST_BLACK_KNIGHT_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::Rook) {
        zobrist ^= constants::ZOBRIST_BLACK_ROOK_NUMBERS[p];
    }
    for p in board.get_bb(Color::Black, Piece::Pawn) {
        zobrist ^= constants::ZOBRIST_BLACK_PAWN_NUMBERS[p];
    }
    zobrist
}

#[inline]
pub fn get_neighbours(s: Square) -> BitBoard {
    BitBoard::new(constants::NEIGHBOURS[s])
}

#[inline]
pub fn get_next_neighbours(s: Square) -> BitBoard {
    BitBoard::new(constants::NEXT_NEIGHBOURS[s])
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum MoveType {
    Normal,
    Capture(Piece),
    Promotion(Piece),
    PromotionCapture((Piece, Piece)),
    Enpassant,
    Castle,
    Null,
}

#[derive(PartialEq, Clone, Copy, Debug)]
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
        from += 8 * (chars.next().unwrap() as u8 - b'1');
        let mut to = chars.next().unwrap() as u8 - b'a';
        to += 8 * (chars.next().unwrap() as u8 - b'1');
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
            typ: determine_move_type(&pos.board, from.into(), to.into(), piece, prom),
        }
    }
    pub fn compress(&self) -> CompressedMove {
        let mut piece_and_type = self.piece as u16;
        piece_and_type |= match self.typ {
            MoveType::Normal => 0,
            MoveType::Capture(p) => ((p as u16) << 3) | (1 << 9),
            MoveType::Promotion(p) => ((p as u16) << 3) | (2 << 9),
            MoveType::PromotionCapture((p, q)) => ((p as u16) << 3) | ((q as u16) << 6) | (3 << 9),
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

#[derive(Copy, Clone, PartialEq, Default)]
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
            3 => MoveType::PromotionCapture((
                u16_to_piece((self.piece_and_type >> 3) & 0b111),
                u16_to_piece((self.piece_and_type >> 6) & 0b111),
            )),
            4 => MoveType::Castle,
            5 => MoveType::Enpassant,
            _ => return None,
        };
        Some(Move {
            from: self.from.into(),
            to: self.to.into(),
            piece,
            typ,
        })
    }
}

pub type MoveList = arrayvec::ArrayVec<Move, 256>;

fn display_promotion(m: &Move) -> String {
    match m.typ {
        MoveType::Promotion(Piece::Queen) => "q".to_string(),
        MoveType::Promotion(Piece::Rook) => "r".to_string(),
        MoveType::Promotion(Piece::Bishop) => "b".to_string(),
        MoveType::Promotion(Piece::Knight) => "n".to_string(),
        MoveType::PromotionCapture((Piece::Queen, _)) => "q".to_string(),
        MoveType::PromotionCapture((Piece::Rook, _)) => "r".to_string(),
        MoveType::PromotionCapture((Piece::Bishop, _)) => "b".to_string(),
        MoveType::PromotionCapture((Piece::Knight, _)) => "n".to_string(),
        _ => "".to_string(),
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", self.from, self.to, display_promotion(self))
    }
}

#[inline]
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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Piece {
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
    Any,
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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    #[inline]
    pub fn other(self) -> Color {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }
}

#[derive(Copy, Clone)]
struct PlyInfo {
    castling_rights: [[bool; 2]; 2],
    rule_50_count: u8,
    attacked_squares: BitBoard,
    king_attackers: BitBoard,
    pinned_pieces: BitBoard,
    ep_square: Option<Square>,
}

#[derive(Clone)]
pub struct Position {
    pub board: Board,
    ply_info: PlyInfo,
    ply_info_history: Vec<PlyInfo>,
    move_history: Vec<Move>,
    to_move: Color,
    zobrist: u64,      //Zobrist-Hash
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
        let mut zobrist = get_zobrist(&board);
        for i in 0..4 {
            zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[i];
        }
        Position {
            board,
            move_history: Vec::with_capacity(20),
            to_move: Color::White,
            ply_info: PlyInfo {
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                ep_square: None,
            },
            ply_info_history: Vec::new(),
            zobrist,
            history: Vec::new(),
        }
    }
    pub fn from_fen(fen: String) -> Option<Position> {
        //First set up the pieces
        let mut fen_parts = fen.split_whitespace();
        let b = from_fen(fen_parts.next().unwrap())?;
        let mut pos = Position {
            board: b,
            move_history: Vec::with_capacity(20),
            to_move: Color::White,
            ply_info: PlyInfo {
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                ep_square: None,
            },
            ply_info_history: Vec::new(),
            zobrist: 0,
            history: Vec::new(),
        };
        pos.zobrist = get_zobrist(&b);
        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {
                    pos.to_move = Color::White;
                } else if p == "b" {
                    pos.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
                    pos.to_move = Color::Black;
                } else {
                    return None;
                }
            }
            None => return None,
        }
        //Set castling rights
        let mut castling_legal = [[false, false], [false, false]];
        if let Some(p) = fen_parts.next() {
            if p.contains('K') {
                castling_legal[0][0] = true;
                pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
            }
            if p.contains('Q') {
                castling_legal[0][1] = true;
                pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
            }
            if p.contains('k') {
                castling_legal[1][0] = true;
                pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
            }
            if p.contains('q') {
                castling_legal[1][1] = true;
                pos.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
            }
        }
        pos.ply_info.castling_rights = castling_legal;
        //Set en passant if necessary
        if let Some(p) = fen_parts.next() {
            match p {
                "-" => {}
                _ => {
                    let sq = Square::from_string(p);
                    pos.ply_info.ep_square = Some(sq);
                    pos.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[sq.file() as usize];
                    pos.move_history.push(Move {
                        piece: Piece::Pawn,
                        from: sq.advance(pos.to_move),
                        to: sq.advance(pos.to_move.other()),
                        typ: MoveType::Normal,
                    });
                }
            }
        }
        if let Some(p) = fen_parts.next() {
            match p.parse::<u8>() {
                Ok(n) => pos.ply_info.rule_50_count = n,
                Err(_) => return None,
            }
        }
        Some(pos)
    }
    #[inline]
    pub fn get_board(&self) -> &Board {
        &self.board
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
        BitBoard::new(
            constants::ROOK_MMASK[occ.pext(moves) + constants::ROOK_MMASK_OFFSETS[sq] as usize],
        )
    }
    #[inline]
    fn rook_moves(&self, sq: Square) -> BitBoard {
        self.rook_moves_for_occupation(sq, self.board.occupation())
    }
    // Compute the bishop moves from a given square using the lookup table and PEXT/PDEP boards
    // Computes the possible moves from the least significant bit in the Square
    #[inline]
    fn bishop_moves_for_occupation(&self, sq: Square, occ: BitBoard) -> BitBoard {
        let moves = constants::BISHOP_MOVES[sq];
        BitBoard::new(
            constants::BISHOP_MMASK[occ.pext(moves) + constants::BISHOP_MMASK_OFFSETS[sq] as usize],
        )
    }
    #[inline]
    fn bishop_moves(&self, sq: Square) -> BitBoard {
        self.bishop_moves_for_occupation(sq, self.board.occupation())
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
        let single = self.board.get_bb(c, Piece::Pawn).shifted_forward(c)
            & !relative_eighth_rank
            & !self.board.occupation();
        let double = (single & relative_third_rank).shifted_forward(c) & !self.board.occupation();
        (single, double)
    }
    #[inline]
    fn pawn_promotions(&self, c: Color) -> BitBoard {
        let relative_seventh_rank = BitBoard::from_rank(Rank::Seventh.relative(c));
        (relative_seventh_rank & self.board.get_bb(c, Piece::Pawn)).shifted_forward(c)
            & !self.board.occupation()
    }
    #[inline]
    pub fn pawn_attacks(&self, sq: Square, c: Color) -> BitBoard {
        BitBoard::new(constants::PAWN_ATTACKS[c as usize][sq as usize])
    }
    #[inline]
    pub fn generate_attack_table(&mut self) {
        let opp = self.to_move.other();
        let king_pos = self.board.get_bb(self.to_move, Piece::King);
        let king_square = king_pos.least_square();
        let occupation = self.board.occupation() ^ king_pos;
        //make sure we cause no collisions..
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        //calculate squares attacked by each pawn
        let pawns = self.board.get_bb(opp, Piece::Pawn);
        for pawn in pawns {
            let attacked = self.pawn_attacks(pawn, opp);
            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(pawn);
            }
            self.ply_info.attacked_squares |= attacked;
        }
        //calculate squares attacked by knights
        let knights = self.board.get_bb(opp, Piece::Knight);
        for knight in knights {
            let attacked = self.knight_moves(knight);
            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(knight);
            }
            self.ply_info.attacked_squares |= attacked;
        }
        //calculate squares attacked by king
        self.ply_info.attacked_squares |=
            self.king_moves(self.board.get_bb(opp, Piece::King).least_square());
        //calculate squares attacked by rooks
        let rook_moves_from_king = self.rook_moves(king_square);
        let rooks = self.board.get_bb(opp, Piece::Rook);
        for rook in rooks {
            let attacked = self.rook_moves_for_occupation(rook, occupation);
            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(rook);
            }
            self.ply_info.pinned_pieces |= attacked
                & rook_moves_from_king
                & self.board.occupation()
                & BitBoard::new(constants::RAYS[king_square][rook]);
            self.ply_info.attacked_squares |= attacked;
        }
        //calculate squares attacked by bishops
        let bish_moves_from_king = self.bishop_moves(king_square);
        let bishops = self.board.get_bb(opp, Piece::Bishop);
        for bishop in bishops {
            let attacked = self.bishop_moves_for_occupation(bishop, occupation);
            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(bishop);
            }
            self.ply_info.pinned_pieces |= attacked
                & bish_moves_from_king
                & self.board.occupation()
                & BitBoard::new(constants::RAYS[king_square][bishop]);
            self.ply_info.attacked_squares |= attacked;
        }
        let queens = self.board.get_bb(opp, Piece::Queen);
        //calculate squares attacked by queens
        for queen in queens {
            let attacked_r = self.rook_moves_for_occupation(queen, occupation);
            let attacked_b = self.bishop_moves_for_occupation(queen, occupation);
            if !((attacked_r | attacked_b) & king_pos).is_empty() {
                self.ply_info.king_attackers |= queen.into();
            }
            self.ply_info.pinned_pieces |= attacked_r
                & rook_moves_from_king
                & self.board.occupation()
                & BitBoard::new(constants::RAYS[king_square][queen]);
            self.ply_info.pinned_pieces |= attacked_b
                & bish_moves_from_king
                & self.board.occupation()
                & BitBoard::new(constants::RAYS[king_square][queen]);
            self.ply_info.attacked_squares |= attacked_b | attacked_r;
        }
    }
    //Calculate possible moves of a piece on a given square, using the provide move gen closure
    //
    #[inline]
    fn get_piece_moves(
        &self,
        move_getter: impl Fn(Square) -> BitBoard,
        moves: &mut MoveList,
        p: Piece,
    ) {
        for pos in self.board.get_bb(self.to_move, p) {
            let mut pmoves = move_getter(pos);
            pmoves &= pmoves ^ self.board.get_color_bb(self.to_move);
            if !(self.ply_info.pinned_pieces & pos.into()).is_empty() {
                pmoves &= BitBoard::new(
                    constants::RAYS[pos]
                        [self.board.get_bb(self.to_move, Piece::King).least_square()],
                );
            }
            for m in pmoves {
                moves.push(Move {
                    from: pos,
                    to: m,
                    piece: p,
                    typ: if !self.board.occupation().is_set(m) {
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
        if let Some(esq) = self.ply_info.ep_square {
            let cap_sq = esq.advance(self.to_move.other());
            if check_mask.is_set(esq) || check_mask.is_set(cap_sq) {
                let cands = self.pawn_attacks(esq, self.to_move.other())
                    & self.board.get_bb(self.to_move, Piece::Pawn);
                for cand in cands {
                    let kpos = self.board.get_bb(self.to_move, Piece::King);
                    let pin_ray = BitBoard::new(constants::RAYS[cand][kpos.least_square()]);
                    //Check if we expose the king by taking en passant
                    //See if our pawn is pinned
                    if (self.ply_info.pinned_pieces.is_set(cand) && !pin_ray.is_set(esq))
                        || (self.ply_info.pinned_pieces.is_set(cap_sq)
                            && !BitBoard::new(constants::RAYS[cap_sq][kpos.least_square()])
                                .is_set(esq))
                    {
                        continue;
                    //Check for a double pin by a rook or queen.
                    } else if pin_ray.is_set(cap_sq) {
                        let mut occupation = self.board.occupation();
                        occupation.unset(cap_sq);
                        occupation.unset(cand);
                        let k_ray = pin_ray
                            & self.rook_moves_for_occupation(kpos.least_square(), occupation);
                        if !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Rook))
                            .is_empty()
                            || !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Queen))
                                .is_empty()
                        {
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
        if self.ply_info.king_attackers.count() != 0 {
            return;
        }
        match self.to_move {
            Color::White => {
                const WHITE_KING_CASTLE_MASK: BitBoard = BitBoard::new(0b01100000);
                const WHITE_QUEEN_CASTLE_CHECK_MASK: BitBoard = BitBoard::new(0b00001100);
                const WHITE_QUEEN_CASTLE_MATERIAL_MASK: BitBoard = BitBoard::new(0b00001110);
                //Check for kingside castling.
                if self.ply_info.castling_rights[0][0]
                    && ((self.ply_info.attacked_squares | self.board.occupation())
                        & WHITE_KING_CASTLE_MASK)
                        .is_empty()
                {
                    moves.push(Move {
                        from: Square::E1,
                        to: Square::G1,
                        piece: Piece::King,
                        typ: MoveType::Castle,
                    });
                }
                if self.ply_info.castling_rights[0][1]
                    && (self.ply_info.attacked_squares & WHITE_QUEEN_CASTLE_CHECK_MASK).is_empty()
                    && (self.board.occupation() & WHITE_QUEEN_CASTLE_MATERIAL_MASK).is_empty()
                {
                    moves.push(Move {
                        from: Square::E1,
                        to: Square::C1,
                        piece: Piece::King,
                        typ: MoveType::Castle,
                    });
                }
            }
            Color::Black => {
                const BLACK_KING_CASTLE_MASK: BitBoard = BitBoard::new(0b01100000 << 56);
                const BLACK_QUEEN_CASTLE_CHECK_MASK: BitBoard = BitBoard::new(0b00001100 << 56);
                const BLACK_QUEEN_CASTLE_MATERIAL_MASK: BitBoard = BitBoard::new(0b00001110 << 56);
                //Check for kingside castling.
                if self.ply_info.castling_rights[1][0]
                    && ((self.ply_info.attacked_squares | self.board.occupation())
                        & BLACK_KING_CASTLE_MASK)
                        .is_empty()
                {
                    moves.push(Move {
                        from: Square::E8,
                        to: Square::G8,
                        piece: Piece::King,
                        typ: MoveType::Castle,
                    });
                }
                if self.ply_info.castling_rights[1][1]
                    && (self.ply_info.attacked_squares & BLACK_QUEEN_CASTLE_CHECK_MASK).is_empty()
                    && (self.board.occupation() & BLACK_QUEEN_CASTLE_MATERIAL_MASK).is_empty()
                {
                    moves.push(Move {
                        from: Square::E8,
                        to: Square::C8,
                        piece: Piece::King,
                        typ: MoveType::Castle,
                    });
                }
            }
        }
    }
    #[inline]
    fn get_pawn_moves(&self, moves: &mut MoveList, check_mask: BitBoard) {
        const PROMOTION_PIECES: [Piece; 4] =
            [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];
        let pawns = self.board.get_bb(self.to_move, Piece::Pawn);
        let ksq = self.board.get_bb(self.to_move, Piece::King).least_square();
        let (advances, double_advances) = self.pawn_moves(self.to_move);
        let direction = match self.to_move {
            Color::White => 1,
            Color::Black => -1,
        };
        for to in advances & check_mask {
            let from = to.shifted_by(-direction * 8);
            if self.ply_info.pinned_pieces.is_set(from)
                && !BitBoard::new(constants::RAYS[ksq][from]).is_set(to)
            {
                continue;
            }
            moves.push(Move {
                piece: Piece::Pawn,
                from,
                to,
                typ: MoveType::Normal,
            });
        }
        for to in double_advances & check_mask {
            let from = to.shifted_by(-direction * 16);
            if self.ply_info.pinned_pieces.is_set(from)
                && !BitBoard::new(constants::RAYS[ksq][from]).is_set(to)
            {
                continue;
            }
            moves.push(Move {
                piece: Piece::Pawn,
                from,
                to,
                typ: MoveType::Normal,
            });
        }
        for to in self.pawn_promotions(self.to_move) & check_mask {
            let from = to.shifted_by(-direction * 8);
            if self.ply_info.pinned_pieces.is_set(from) {
                continue;
            }
            for p in PROMOTION_PIECES {
                moves.push(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::Promotion(p),
                });
            }
        }
        for pawn in pawns {
            let mut attacks = (self.pawn_attacks(pawn, self.to_move)
                & self.board.get_color_bb(self.to_move.other()))
                & check_mask;
            if self.ply_info.pinned_pieces.is_set(pawn) {
                attacks &= BitBoard::new(constants::RAYS[ksq][pawn]);
            }
            for m in attacks {
                let cap = self.board.piece_at(m).unwrap();
                if m.rank().relative(self.to_move) == Rank::Eighth {
                    for p in PROMOTION_PIECES {
                        moves.push(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::PromotionCapture((p, cap)),
                        });
                    }
                } else {
                    moves.push(Move {
                        from: pawn,
                        to: m,
                        piece: Piece::Pawn,
                        typ: MoveType::Capture(cap),
                    });
                }
            }
        }
    }
    #[allow(dead_code)]
    pub fn get_opponent_moves(&mut self) -> MoveList {
        self.to_move = self.to_move.other();
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        let moves = self.get_moves();
        self.to_move = self.to_move.other();
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        moves
    }
    pub fn get_moves(&mut self) -> MoveList {
        //We expect about 35 moves in the average position
        let mut moves = MoveList::new();
        if self.ply_info.rule_50_count == 100 || self.board.occupation().count() == 2 {
            return moves;
        }
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        //Generate King moves first (except castling)
        let mut king_moves =
            self.king_moves(self.board.get_bb(self.to_move, Piece::King).least_square());
        king_moves &= !self.board.get_color_bb(self.to_move);
        king_moves &= !self.ply_info.attacked_squares;
        for km in king_moves {
            moves.push(Move {
                from: self.board.get_bb(self.to_move, Piece::King).least_square(),
                to: km,
                piece: Piece::King,
                typ: if !self.board.occupation().is_set(km) {
                    MoveType::Normal
                } else {
                    MoveType::Capture(self.board.piece_at(km).unwrap())
                },
            });
        }
        //optimize double or better check
        if self.ply_info.king_attackers.count() > 1 {
            return moves;
        }
        //If we are in check, we may only take the checker, or block it
        //If the checking piece is a knight, we can only take (or move the king)
        let check_mask = if !(self.board.get_bb(self.to_move.other(), Piece::Knight)
            & self.ply_info.king_attackers)
            .is_empty()
        {
            self.ply_info.king_attackers
        //For any other piece we may also try to block
        } else if !self.ply_info.king_attackers.is_empty() {
            BitBoard::new(
                constants::CONNECTING_RAYS[self.ply_info.king_attackers.least_square()]
                    [self.board.get_bb(self.to_move, Piece::King).least_square()],
            ) ^ self.board.get_bb(self.to_move, Piece::King)
        } else {
            BitBoard::FULL
        };
        //generate queen moves
        self.get_piece_moves(
            |sq: Square| -> BitBoard { (self.rook_moves(sq) | self.bishop_moves(sq)) & check_mask },
            &mut moves,
            Piece::Queen,
        );
        //generate rook moves
        self.get_piece_moves(
            |sq: Square| -> BitBoard { self.rook_moves(sq) & check_mask },
            &mut moves,
            Piece::Rook,
        );
        //generate bishop moves
        self.get_piece_moves(
            |sq: Square| -> BitBoard { self.bishop_moves(sq) & check_mask },
            &mut moves,
            Piece::Bishop,
        );
        //generate knight moves
        self.get_piece_moves(
            |sq: Square| -> BitBoard { self.knight_moves(sq) & check_mask },
            &mut moves,
            Piece::Knight,
        );
        //generate pawn moves without en passant
        self.get_pawn_moves(&mut moves, check_mask);
        //Handle castling and en passant.
        self.handle_castling(&mut moves);
        self.handle_en_passant(&mut moves, check_mask);
        moves
    }
    //tells us to score this as + or - one for white/black
    #[inline]
    pub fn color(&self) -> Color {
        self.to_move
    }
    pub fn from_move(&self, m: Move) -> Position {
        let mut pos = self.clone();
        pos.do_move(m);
        pos
    }
    #[inline]
    fn update_zobrist(&mut self, m: Move) {
        self.zobrist ^= match m.typ {
            MoveType::Castle => match m.to {
                Square::C1 => {
                    get_zobrist_number(Piece::King, Color::White, Square::E1)
                        ^ get_zobrist_number(Piece::King, Color::White, Square::C1)
                        ^ get_zobrist_number(Piece::Rook, Color::White, Square::A1)
                        ^ get_zobrist_number(Piece::Rook, Color::White, Square::D1)
                }
                Square::G1 => {
                    get_zobrist_number(Piece::King, Color::White, Square::E1)
                        ^ get_zobrist_number(Piece::King, Color::White, Square::G1)
                        ^ get_zobrist_number(Piece::Rook, Color::White, Square::H1)
                        ^ get_zobrist_number(Piece::Rook, Color::White, Square::F1)
                }
                Square::C8 => {
                    get_zobrist_number(Piece::King, Color::Black, Square::E8)
                        ^ get_zobrist_number(Piece::King, Color::Black, Square::C8)
                        ^ get_zobrist_number(Piece::Rook, Color::Black, Square::A8)
                        ^ get_zobrist_number(Piece::Rook, Color::Black, Square::D8)
                }
                Square::G8 => {
                    get_zobrist_number(Piece::King, Color::Black, Square::E8)
                        ^ get_zobrist_number(Piece::King, Color::Black, Square::G8)
                        ^ get_zobrist_number(Piece::Rook, Color::Black, Square::H8)
                        ^ get_zobrist_number(Piece::Rook, Color::Black, Square::F8)
                }
                _ => 0,
            },
            MoveType::Enpassant => {
                get_zobrist_number(Piece::Pawn, self.to_move, m.from)
                    ^ get_zobrist_number(Piece::Pawn, self.to_move, m.to)
                    ^ get_zobrist_number(
                        Piece::Pawn,
                        self.to_move.other(),
                        self.ply_info.ep_square.unwrap(),
                    )
            }
            MoveType::Promotion(p) => {
                get_zobrist_number(Piece::Pawn, self.to_move, m.from)
                    ^ get_zobrist_number(p, self.to_move, m.to)
            }
            MoveType::PromotionCapture((p, c)) => {
                get_zobrist_number(Piece::Pawn, self.to_move, m.from)
                    ^ get_zobrist_number(p, self.to_move, m.to)
                    ^ get_zobrist_number(c, self.to_move.other(), m.to)
            }
            MoveType::Capture(c) => {
                get_zobrist_number(m.piece, self.to_move, m.from)
                    ^ get_zobrist_number(m.piece, self.to_move, m.to)
                    ^ get_zobrist_number(c, self.to_move.other(), m.to)
            }
            MoveType::Normal => {
                get_zobrist_number(m.piece, self.to_move, m.from)
                    ^ get_zobrist_number(m.piece, self.to_move, m.to)
            }

            _ => 0,
        }
    }
    pub fn do_move(&mut self, m: Move) {
        //Commit zobrist hash to history stack
        self.history.push(self.zobrist);
        //Store info over the current ply
        self.ply_info_history.push(self.ply_info);
        self.board.do_move(m);
        self.update_zobrist(m);
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[esq.file() as usize];
        }
        //Check if a new en passant flag is to be set
        if m.piece == Piece::Pawn
            && m.from.relative(self.to_move).rank() == Rank::Second
            && m.to.relative(self.to_move).rank() == Rank::Fourth
        {
            self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[m.to.file() as usize];
            self.ply_info.ep_square = Some(ep_square(m.to.file()).relative(self.to_move));
        } else {
            self.ply_info.ep_square = None;
        }
        self.move_history.push(m);
        if m.piece == Piece::King {
            if self.ply_info.castling_rights[self.to_move as usize][0] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2 * self.to_move as usize];
            }
            if self.ply_info.castling_rights[self.to_move as usize][1] {
                self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2 * self.to_move as usize + 1];
            }
            self.ply_info.castling_rights[self.to_move as usize] = [false, false];
        }
        if self.ply_info.castling_rights[0][1] && (m.to == Square::A1 || m.from == Square::A1) {
            self.ply_info.castling_rights[0][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[1];
        } else if self.ply_info.castling_rights[0][0]
            && (m.to == Square::H1 || m.from == Square::H1)
        {
            self.ply_info.castling_rights[0][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[0];
        }
        if self.ply_info.castling_rights[1][0] && (m.to == Square::H8 || m.from == Square::H8) {
            self.ply_info.castling_rights[1][0] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[2];
        } else if self.ply_info.castling_rights[1][1]
            && (m.to == Square::A8 || m.from == Square::A8)
        {
            self.ply_info.castling_rights[1][1] = false;
            self.zobrist ^= constants::ZOBRIST_CASTLING_NUMBERS[3];
        }
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        self.to_move = self.to_move.other();
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        if m.piece == Piece::Pawn || matches!(m.typ, MoveType::Capture(_)) {
            self.ply_info.rule_50_count = 0;
        } else {
            self.ply_info.rule_50_count += 1;
        }
    }
    #[allow(dead_code)]
    pub fn get_castling_rights(&self) -> [[bool; 2]; 2] {
        self.ply_info.castling_rights
    }
    #[inline]
    pub fn undo_move(&mut self) {
        //remove move from history stack
        self.zobrist = self.history.pop().unwrap();
        self.ply_info = self.ply_info_history.pop().unwrap();
        let m = self
            .move_history
            .pop()
            .unwrap_or_else(|| panic!("No move to undo!"));
        self.board.undo_move(m);
        self.to_move = self.to_move.other();
    }
    pub fn do_null_move(&mut self) {
        self.move_history.push(Move {
            typ: MoveType::Null,
            piece: Piece::Any,
            to: Square::A1,
            from: Square::A1,
        });
        self.history.push(self.zobrist);
        self.zobrist ^= constants::ZOBRIST_BLACK_NUMBER;
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.zobrist ^= constants::ZOBRIST_ENPASSANT_NUMBERS[esq.file() as usize];
        }
        self.to_move = self.to_move.other();
        self.ply_info_history.push(self.ply_info);
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        self.ply_info.rule_50_count = 0;
        self.ply_info.ep_square = None;
    }
    #[inline]
    pub fn undo_null_move(&mut self) {
        self.move_history.pop();
        self.zobrist = self.history.pop().unwrap();
        self.to_move = self.to_move.other();
        self.ply_info = self.ply_info_history.pop().unwrap();
    }
    #[inline]
    pub fn in_check(&mut self) -> bool {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        !self.ply_info.king_attackers.is_empty()
    }
    #[inline]
    pub fn piece_count(&self, c: Color, p: Piece) -> i32 {
        self.board.get_bb(c, p).count() as i32
    }
    #[allow(dead_code)]
    pub fn total_piece_count(&self) -> i32 {
        self.board.occupation().count() as i32
    }
    #[inline]
    fn material_count(&self, c: Color) -> i32 {
        self.piece_count(c, Piece::Pawn)
            + 3 * (self.piece_count(c, Piece::Bishop) + self.piece_count(c, Piece::Knight))
            + 5 * self.piece_count(c, Piece::Rook)
            + 9 * self.piece_count(c, Piece::Queen)
    }
    #[inline]
    pub fn material_balance(&self) -> i32 {
        self.material_count(self.to_move) - self.material_count(self.to_move.other())
    }
    #[allow(dead_code)]
    pub fn is_attacked(&self, sq: Square) -> bool {
        self.ply_info.attacked_squares.is_set(sq)
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
    #[inline]
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
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        self.ply_info.pinned_pieces
    }
    //Switch color for analysis
    //Does NOT update Zobrist hash
    pub fn switch_color(&mut self) {
        self.to_move = self.to_move.other();
        self.ply_info.attacked_squares = BitBoard::EMPTY;
    }
    //TODO: Should this _really_ be here? But where else to put it?
    //Helper for SEE
    #[inline]
    fn least_valuable_attacker(
        &self,
        mut attackers: BitBoard,
        c: Color,
    ) -> Option<(Square, Piece)> {
        attackers &= self.board.get_color_bb(c);
        attackers
            .into_iter()
            .map(|a| self.board.piece_at(a).map(|p| (a, p)))
            .filter(|x| x.is_some())
            .min_by_key(|x| x.unwrap().1.value())
            .flatten()
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
        let mut occupation = self.board.occupation();
        //We initialize pawn and knight attacks, since they do not depend on the occupation.
        let mut attackers = (self.pawn_attacks(target, Color::White)
            & self.board.get_bb(Color::Black, Piece::Pawn))
            | (self.pawn_attacks(target, Color::Black)
                & self.board.get_bb(Color::White, Piece::Pawn));
        attackers |= self.knight_moves(target)
            & (self.board.get_bb(Color::White, Piece::Knight)
                | self.board.get_bb(Color::Black, Piece::Knight));
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
            if std::cmp::max(-last_gain, taker.value() - last_gain) < 0 {
                break;
            }
            occupation ^= from.into();
            attackers ^= from.into();
            color = color.other();
            attackers |= self.bishop_moves_for_occupation(target, occupation)
                & (self.board.get_bb(Color::Black, Piece::Bishop)
                    | self.board.get_bb(Color::White, Piece::Bishop)
                    | self.board.get_bb(Color::Black, Piece::Queen)
                    | self.board.get_bb(Color::White, Piece::Queen))
                & occupation;
            attackers |= self.rook_moves_for_occupation(target, occupation)
                & (self.board.get_bb(Color::Black, Piece::Rook)
                    | self.board.get_bb(Color::White, Piece::Rook)
                    | self.board.get_bb(Color::Black, Piece::Queen)
                    | self.board.get_bb(Color::White, Piece::Queen))
                & occupation; //Do not accidentally include already used pieces again
            let next = match self.least_valuable_attacker(attackers, color) {
                Some(a) => a,
                None => break,
            };
            taker = next.1;
            from = next.0;
        }
        gain.reverse();
        for d in 1..gain.len() - 1 {
            let g = -std::cmp::max(-gain[d + 1], gain[d]);
            gain[d + 1] = g;
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
        self.ply_info.rule_50_count
    }
    pub fn gives_check(&self, m: &Move) -> bool {
        let kpos = self
            .board
            .get_bb(self.color().other(), Piece::King)
            .least_square();
        let final_piece = match m.typ {
            MoveType::Promotion(p) | MoveType::PromotionCapture((p, _)) => p,
            _ => m.piece,
        };
        match final_piece {
            Piece::Knight => self.knight_moves(kpos).is_set(m.to),
            Piece::Bishop => self.bishop_moves(kpos).is_set(m.to),
            Piece::Rook => self.rook_moves(kpos).is_set(m.to),
            Piece::Queen => (self.rook_moves(kpos) | self.bishop_moves(kpos)).is_set(m.to),
            Piece::Pawn => {
                let attacked_sqs =
                    BitBoard::new(constants::PAWN_ATTACKS[self.to_move as usize][m.to]);
                attacked_sqs.is_set(kpos)
            }
            _ => false,
        }
    }
}

//Tests
#[test]
fn simple_sse() {
    let pos = Position::from_fen(String::from(
        "1k1r4/1pp4p/p7/4p3/8/P5P1/1PP4P/2K1R3 w - - 0 0",
    ))
    .unwrap();
    let mov = Move {
        from: Square::E1,
        to: Square::E5,
        typ: MoveType::Capture(Piece::Pawn),
        piece: Piece::Rook,
    };
    assert!(pos.see(mov) == 100);
    let pos2 = Position::from_fen(String::from(
        "1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 0",
    ))
    .unwrap();
    let mov2 = Move {
        from: Square::D3,
        to: Square::E5,
        typ: MoveType::Capture(Piece::Pawn),
        piece: Piece::Knight,
    };
    assert!(pos2.see(mov2) == -200);
}

#[test]
fn compress_and_decompress_move() {
    let pieces = vec![
        Piece::Pawn,
        Piece::King,
        Piece::Queen,
        Piece::Bishop,
        Piece::Knight,
        Piece::Rook,
    ];
    for p in pieces.iter() {
        let m = Move {
            from: Square::A1,
            to: Square::B1,
            piece: *p,
            typ: MoveType::Normal,
        };
        let m2 = m.compress().decompress();
        assert!(m == m2.unwrap());
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {
                from: Square::G7,
                to: Square::H8,
                piece: Piece::Pawn,
                typ: MoveType::PromotionCapture((*p, *q)),
            };
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
    for p in pieces.iter() {
        for q in pieces.iter() {
            let m = Move {
                from: Square::G7,
                to: Square::H8,
                piece: *p,
                typ: MoveType::Capture(*q),
            };
            let m2 = m.compress().decompress();
            assert!(m == m2.unwrap());
        }
    }
}
