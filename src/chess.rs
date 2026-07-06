mod bitboard;
mod board;
mod moves;

use std::iter::Iterator;

use crate::constants;
pub use bitboard::{BitBoard, File, Rank, Square};
pub use board::Board;
pub use moves::{Move, MoveList, MoveType};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Piece {
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
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

pub trait GenType {
    const QUIETS: bool;
    const CAPTS: bool;
}

pub struct All {}

impl GenType for All {
    const QUIETS: bool = true;
    const CAPTS: bool = true;
}

pub struct Quiets {}

impl GenType for Quiets {
    const QUIETS: bool = true;
    const CAPTS: bool = false;
}

pub struct Captures {}

impl GenType for Captures {
    const QUIETS: bool = false;
    const CAPTS: bool = true;
}

#[derive(Copy, Clone)]
struct PlyInfo {
    zobrist: u64,
    attacked_squares: BitBoard,
    king_attackers: BitBoard,
    pinned_pieces: BitBoard,
    last_move: Move,
    castling_rights: [[bool; 2]; 2],
    rule_50_count: u8,
    ep_square: Option<Square>,
    capture: Option<Piece>,
}

#[derive(Clone)]
pub struct Position {
    pub board: Board,
    ply_info: PlyInfo,
    history: Vec<PlyInfo>,
    to_move: Color,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub fn new() -> Position {
        let board = Board::new();

        let mut zobrist = board.zobrist();

        for i in [Square::C1, Square::C8, Square::G1, Square::G8] {
            zobrist ^= constants::castle_zobrist(i);
        }

        Position {
            board,
            to_move: Color::White,
            ply_info: PlyInfo {
                zobrist,
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                last_move: Move::ZERO,
                ep_square: None,
                capture: None,
            },
            history: Vec::with_capacity(256),
        }
    }

    pub fn from_fen(fen: String) -> Option<Position> {
        //First set up the pieces
        let mut fen_parts = fen.split_whitespace();

        let b = Board::from_fen(fen_parts.next().unwrap())?;

        let mut pos = Position {
            board: b,
            to_move: Color::White,
            ply_info: PlyInfo {
                zobrist: 0,
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                last_move: Move::ZERO,
                ep_square: None,
                capture: None,
            },
            history: Vec::with_capacity(256),
        };

        pos.ply_info.zobrist = b.zobrist();

        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {
                    pos.to_move = Color::White;
                } else if p == "b" {
                    pos.ply_info.zobrist ^= constants::color_zobrist();
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
                pos.ply_info.zobrist ^= constants::castle_zobrist(Square::G1);
            }
            if p.contains('Q') {
                castling_legal[0][1] = true;
                pos.ply_info.zobrist ^= constants::castle_zobrist(Square::C1);
            }
            if p.contains('k') {
                castling_legal[1][0] = true;
                pos.ply_info.zobrist ^= constants::castle_zobrist(Square::G8);
            }
            if p.contains('q') {
                castling_legal[1][1] = true;
                pos.ply_info.zobrist ^= constants::castle_zobrist(Square::G8);
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
                    pos.ply_info.zobrist ^= constants::enpassant_zobrist(sq.file());
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

    //Create a new position in whch the move was applied
    #[allow(dead_code)]
    pub fn from_move(pos: &Self, m: Move) -> Self {
        let mut pos = pos.clone();
        pos.do_move(m);
        pos
    }

    #[inline]
    pub const fn get_board(&self) -> &Board {
        &self.board
    }

    #[inline]
    pub const fn rule_50_count(&self) -> u8 {
        self.ply_info.rule_50_count
    }

    #[inline]
    pub const fn color(&self) -> Color {
        self.to_move
    }

    #[inline]
    fn rook_moves(&self, sq: Square) -> BitBoard {
        constants::rook_moves(sq, self.board.occupation())
    }

    #[inline]
    fn bishop_moves(&self, sq: Square) -> BitBoard {
        constants::bishop_moves(sq, self.board.occupation())
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
            let attacked = constants::pawn_attacks(pawn, opp);

            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(pawn);
            }

            self.ply_info.attacked_squares |= attacked;
        }

        //calculate squares attacked by knights
        let knights = self.board.get_bb(opp, Piece::Knight);

        for knight in knights {
            let attacked = constants::knight_moves(knight);

            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(knight);
            }

            self.ply_info.attacked_squares |= attacked;
        }

        //calculate squares attacked by king
        self.ply_info.attacked_squares |=
            constants::king_moves(self.board.get_bb(opp, Piece::King).least_square());

        //calculate squares attacked by rooks
        let rook_moves_from_king = self.rook_moves(king_square);
        let rooks = self.board.get_bb(opp, Piece::Rook);

        for rook in rooks {
            let attacked = constants::rook_moves(rook, occupation);

            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(rook);
            }

            self.ply_info.pinned_pieces |= attacked
                & rook_moves_from_king
                & self.board.occupation()
                & constants::ray(king_square, rook);

            self.ply_info.attacked_squares |= attacked;
        }

        //calculate squares attacked by bishops
        let bish_moves_from_king = self.bishop_moves(king_square);
        let bishops = self.board.get_bb(opp, Piece::Bishop);

        for bishop in bishops {
            let attacked = constants::bishop_moves(bishop, occupation);

            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers.set(bishop);
            }

            self.ply_info.pinned_pieces |= attacked
                & bish_moves_from_king
                & self.board.occupation()
                & constants::ray(king_square, bishop);

            self.ply_info.attacked_squares |= attacked;
        }

        let queens = self.board.get_bb(opp, Piece::Queen);

        //calculate squares attacked by queens
        for queen in queens {
            let attacked = constants::rook_moves(queen, occupation)
                | constants::bishop_moves(queen, occupation);

            if !(attacked & king_pos).is_empty() {
                self.ply_info.king_attackers |= queen.into();
            }

            self.ply_info.pinned_pieces |= attacked
                & (rook_moves_from_king | bish_moves_from_king)
                & self.board.occupation()
                & constants::ray(king_square, queen);

            self.ply_info.attacked_squares |= attacked;
        }
    }

    //Calculate possible moves of a piece on a given square, using the provide move gen closure
    #[inline]
    fn get_piece_moves<Gen: GenType>(
        &self,
        move_getter: impl Fn(Square) -> BitBoard,
        moves: &mut MoveList,
        p: Piece,
    ) {
        for pos in self.board.get_bb(self.to_move, p) {
            let mut pmoves = move_getter(pos);
            pmoves &= !self.board.get_color_bb(self.to_move);

            if !(self.ply_info.pinned_pieces & pos.into()).is_empty() {
                pmoves &= constants::ray(pos, self.board.king_square(self.to_move));
            }

            if Gen::CAPTS {
                for m in pmoves & self.board.get_color_bb(self.to_move.other()) {
                    //SAFETY: The ArrayVec capacity exceeds the maximal move number of 218
                    unsafe {
                        moves.push_unchecked(Move::new(pos, m, MoveType::Capture));
                    }
                }
            }
            if Gen::QUIETS {
                for m in pmoves & !self.board.get_color_bb(self.to_move.other()) {
                    //SAFETY: The ArrayVec capacity exceeds the maximal move number of 218
                    unsafe {
                        moves.push_unchecked(Move::new(pos, m, MoveType::Normal));
                    }
                }
            }
        }
    }

    #[inline]
    fn is_legal_enpassant(&self, cand: Square, cap_sq: Square, esq: Square) -> bool {
        let ksq = self.board.king_square(self.to_move);

        let pin_ray = constants::ray(cand, ksq);

        //Check if we expose the king by taking en passant
        //See if our pawn is pinned
        if (self.ply_info.pinned_pieces.is_set(cand) && !pin_ray.is_set(esq))
            || (self.ply_info.pinned_pieces.is_set(cap_sq)
                && !constants::ray(cap_sq, ksq).is_set(esq))
        {
            return false;
        //Check for a double pin by a rook or queen.
        } else if pin_ray.is_set(cap_sq) {
            let mut occupation = self.board.occupation();

            occupation.unset(cap_sq);
            occupation.unset(cand);

            let k_ray = pin_ray & constants::rook_moves(ksq, occupation);

            if !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Rook)).is_empty()
                || !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Queen)).is_empty()
            {
                return false;
            }
        }
        true
    }

    #[inline]
    fn handle_en_passant(&mut self, moves: &mut MoveList, check_mask: BitBoard) {
        if let Some(esq) = self.ply_info.ep_square {
            let cap_sq = esq.advance(self.to_move.other());

            if check_mask.is_set(esq) || check_mask.is_set(cap_sq) {
                let cands = constants::pawn_attacks(esq, self.to_move.other())
                    & self.board.get_bb(self.to_move, Piece::Pawn);

                for cand in cands {
                    if self.is_legal_enpassant(cand, cap_sq, esq) {
                        unsafe {
                            moves.push_unchecked(Move::new(cand, esq, MoveType::Enpassant));
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn handle_castling(&self, moves: &mut MoveList) {
        if self.ply_info.king_attackers.count() != 0 {
            return;
        }

        const KING_CASTLE_MASK: [BitBoard; 2] =
            [BitBoard::new(0b01100000), BitBoard::new(0b01100000 << 56)];
        const QUEEN_CASTLE_CHECK_MASK: [BitBoard; 2] =
            [BitBoard::new(0b00001100), BitBoard::new(0b00001100 << 56)];
        const QUEEN_CASTLE_MATERIAL_MASK: [BitBoard; 2] =
            [BitBoard::new(0b00001110), BitBoard::new(0b00001110 << 56)];

        //Check for kingside castling.
        if self.ply_info.castling_rights[self.to_move as usize][0]
            && ((self.ply_info.attacked_squares | self.board.occupation())
                & KING_CASTLE_MASK[self.to_move as usize])
                .is_empty()
        {
            unsafe {
                moves.push_unchecked(Move::new(
                    Square::E1.relative(self.to_move),
                    Square::G1.relative(self.to_move),
                    MoveType::Castle,
                ));
            }
        }

        //Check for queenside castling.
        if self.ply_info.castling_rights[self.to_move as usize][1]
            && (self.ply_info.attacked_squares & QUEEN_CASTLE_CHECK_MASK[self.to_move as usize])
                .is_empty()
            && (self.board.occupation() & QUEEN_CASTLE_MATERIAL_MASK[self.to_move as usize])
                .is_empty()
        {
            moves.push(Move::new(
                Square::E1.relative(self.to_move),
                Square::C1.relative(self.to_move),
                MoveType::Castle,
            ));
        }
    }

    #[inline]
    fn get_pawn_moves<Gen: GenType>(&self, moves: &mut MoveList, check_mask: BitBoard) {
        let pawns = self.board.get_bb(self.to_move, Piece::Pawn);

        let ksq = self.board.get_bb(self.to_move, Piece::King).least_square();

        if Gen::CAPTS {
            //Push pawn captures
            for pawn in pawns {
                let mut attacks = (constants::pawn_attacks(pawn, self.to_move)
                    & self.board.get_color_bb(self.to_move.other()))
                    & check_mask;

                if self.ply_info.pinned_pieces.is_set(pawn) {
                    attacks &= constants::ray(ksq, pawn);
                }

                for m in attacks & BitBoard::from_rank(Rank::Eighth.relative(self.to_move)) {
                    unsafe {
                        moves.push_unchecked(Move::new(pawn, m, MoveType::PromotionCaptureN));
                        moves.push_unchecked(Move::new(pawn, m, MoveType::PromotionCaptureB));
                        moves.push_unchecked(Move::new(pawn, m, MoveType::PromotionCaptureR));
                        moves.push_unchecked(Move::new(pawn, m, MoveType::PromotionCaptureQ));
                    }
                }
                for m in attacks & !BitBoard::from_rank(Rank::Eighth.relative(self.to_move)) {
                    unsafe {
                        moves.push_unchecked(Move::new(pawn, m, MoveType::Capture));
                    }
                }
            }
        }

        if !Gen::QUIETS {
            return;
        }

        let (advances, double_advances) = self.pawn_moves(self.to_move);

        let direction = match self.to_move {
            Color::White => 1,
            Color::Black => -1,
        };

        //Push single pawn advances
        for to in advances & check_mask {
            let from = to.shifted_by(-direction * 8);

            if self.ply_info.pinned_pieces.is_set(from) && !constants::ray(ksq, from).is_set(to) {
                continue;
            }

            unsafe {
                moves.push_unchecked(Move::new(from, to, MoveType::Normal));
            }
        }

        //Push double pawn advances
        for to in double_advances & check_mask {
            let from = to.shifted_by(-direction * 16);

            if self.ply_info.pinned_pieces.is_set(from) && !constants::ray(ksq, from).is_set(to) {
                continue;
            }

            unsafe {
                moves.push_unchecked(Move::new(from, to, MoveType::Normal));
            }
        }

        //Push pawn promotions
        for to in self.pawn_promotions(self.to_move) & check_mask {
            let from = to.shifted_by(-direction * 8);

            if self.ply_info.pinned_pieces.is_set(from) {
                continue;
            }

            unsafe {
                moves.push_unchecked(Move::new(from, to, MoveType::PromotionN));
                moves.push_unchecked(Move::new(from, to, MoveType::PromotionB));
                moves.push_unchecked(Move::new(from, to, MoveType::PromotionR));
                moves.push_unchecked(Move::new(from, to, MoveType::PromotionQ));
            }
        }
    }

    #[inline]
    fn check_mask(&self) -> BitBoard {
        //If we are in check, we may only take the checker, or block it
        //If the checking piece is a knight, we can only take (or move the king)
        if !(self.board.get_bb(self.to_move.other(), Piece::Knight) & self.ply_info.king_attackers)
            .is_empty()
        {
            self.ply_info.king_attackers
        //For any other piece we may also try to block
        } else if !self.ply_info.king_attackers.is_empty() {
            constants::ray_between(
                self.ply_info.king_attackers.least_square(),
                self.board.king_square(self.to_move),
            ) | self.ply_info.king_attackers
        } else {
            BitBoard::FULL
        }
    }

    pub fn get_moves<Gen: GenType>(&mut self, moves: &mut MoveList) {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }

        let ksq = self.board.get_bb(self.to_move, Piece::King).least_square();
        //Generate King moves first (except castling)
        let mut king_moves = constants::king_moves(ksq);
        king_moves &= !self.board.get_color_bb(self.to_move);
        king_moves &= !self.ply_info.attacked_squares;

        if Gen::CAPTS {
            for km in king_moves & self.board.get_color_bb(self.to_move.other()) {
                moves.push(Move::new(ksq, km, MoveType::Capture));
            }
        }
        if Gen::QUIETS {
            for km in king_moves & !self.board.get_color_bb(self.to_move.other()) {
                moves.push(Move::new(ksq, km, MoveType::Normal));
            }
        }

        //optimize double or better check
        if self.ply_info.king_attackers.count() > 1 {
            return;
        }

        let check_mask = self.check_mask();

        //generate queen moves
        self.get_piece_moves::<Gen>(
            |sq: Square| -> BitBoard { (self.rook_moves(sq) | self.bishop_moves(sq)) & check_mask },
            moves,
            Piece::Queen,
        );

        //generate rook moves
        self.get_piece_moves::<Gen>(
            |sq: Square| -> BitBoard { self.rook_moves(sq) & check_mask },
            moves,
            Piece::Rook,
        );

        //generate bishop moves
        self.get_piece_moves::<Gen>(
            |sq: Square| -> BitBoard { self.bishop_moves(sq) & check_mask },
            moves,
            Piece::Bishop,
        );

        //generate knight moves
        self.get_piece_moves::<Gen>(
            |sq: Square| -> BitBoard { constants::knight_moves(sq) & check_mask },
            moves,
            Piece::Knight,
        );

        //generate pawn moves without en passant
        self.get_pawn_moves::<Gen>(moves, check_mask);

        //Handle castling and en passant.
        if Gen::QUIETS {
            self.handle_castling(moves);
        }

        if Gen::CAPTS {
            self.handle_en_passant(moves, check_mask);
        }
    }

    // We assume the move is well-formed in principle, but not compatible with the current board
    // state. This will not check that e.g. a promotion is a move from seventh to eighth rank.
    pub fn is_legal(&mut self, m: Move) -> bool {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        if !self.board.get_color_bb(self.to_move).is_set(m.from()) {
            return false;
        };
        let mover = if let Some(p) = self.board.piece_at(m.from()) {
            p
        } else {
            return false;
        };
        if self.ply_info.king_attackers.count() > 1 && mover != Piece::King {
            return false;
        }
        let ksq = self.board.get_bb(self.to_move, Piece::King).least_square();
        let check_mask = self.check_mask();
        if mover != Piece::King {
            if !check_mask.is_set(m.to()) && m.typ() != MoveType::Enpassant {
                return false;
            }
            if self.ply_info.pinned_pieces.is_set(m.from())
                && !constants::ray(m.from(), ksq).is_set(m.to())
            {
                return false;
            }
        }
        match m.typ() {
            MoveType::Enpassant => {
                let esq = if let Some(sq) = self.ply_info.ep_square
                    && m.to() == sq
                {
                    sq
                } else {
                    return false;
                };
                let cap_sq = esq.advance(self.to_move.other());
                mover == Piece::Pawn
                    && (check_mask.is_set(esq) || check_mask.is_set(cap_sq))
                    && self.is_legal_enpassant(m.from(), cap_sq, esq)
            }
            MoveType::PromotionN
            | MoveType::PromotionB
            | MoveType::PromotionR
            | MoveType::PromotionQ => {
                mover == Piece::Pawn
                    && self.board.piece_at(m.to()).is_none()
                    && m.from().advance(self.to_move) == m.to()
            }
            MoveType::PromotionCaptureN
            | MoveType::PromotionCaptureB
            | MoveType::PromotionCaptureR
            | MoveType::PromotionCaptureQ => {
                mover == Piece::Pawn
                    && (constants::pawn_attacks(m.from(), self.to_move)
                        & self.board.get_color_bb(self.to_move.other()))
                    .is_set(m.to())
            }
            MoveType::Castle => {
                let piece_ray = if m.to().file() == File::C {
                    constants::ray_between(
                        Square::E1.relative(self.to_move),
                        Square::A1.relative(self.to_move),
                    )
                } else {
                    constants::ray_between(
                        Square::E1.relative(self.to_move),
                        Square::H1.relative(self.to_move),
                    )
                };
                let check_ray = if m.to().file() == File::C {
                    constants::ray_between(
                        Square::E1.relative(self.to_move),
                        Square::B1.relative(self.to_move),
                    )
                } else {
                    constants::ray_between(
                        Square::E1.relative(self.to_move),
                        Square::H1.relative(self.to_move),
                    )
                };
                if m.to().rank().relative(self.to_move) == Rank::Eighth {
                    return false;
                }
                //Having the correct pieces on each square is guaranteed by castling rights
                self.ply_info.king_attackers.is_empty()
                    && ((m.to().file() == File::G
                        && self.ply_info.castling_rights[self.to_move as usize][0])
                        || (m.to().file() == File::C
                            && self.ply_info.castling_rights[self.to_move as usize][1]))
                    && (piece_ray & self.board.occupation()).is_empty()
                    && (check_ray & self.ply_info.attacked_squares).is_empty()
            }
            MoveType::Normal | MoveType::Capture => {
                if (m.is_capture() && !self.board.get_color_bb(self.to_move.other()).is_set(m.to()))
                    || (!m.is_capture() && self.board.occupation().is_set(m.to()))
                {
                    return false;
                }
                match mover {
                    Piece::Pawn => {
                        if matches!(m.to().rank(), Rank::Eighth | Rank::First) {
                            return false;
                        }
                        if m.is_capture() {
                            return (constants::pawn_attacks(m.from(), self.to_move)
                                & self.board.get_color_bb(self.to_move.other()))
                            .is_set(m.to());
                        }
                        m.to() == m.from().advance(self.to_move)
                            || (m.from().rank().relative(self.to_move) == Rank::Second
                                && !self
                                    .board
                                    .occupation()
                                    .is_set(m.from().advance(self.to_move))
                                && m.to() == m.from().advance(self.to_move).advance(self.to_move))
                    }
                    Piece::King => {
                        constants::king_moves(m.from()).is_set(m.to())
                            && !self.ply_info.attacked_squares.is_set(m.to())
                    }
                    Piece::Knight => constants::knight_moves(m.from()).is_set(m.to()),
                    Piece::Rook => {
                        constants::rook_moves(m.from(), self.board.occupation()).is_set(m.to())
                    }
                    Piece::Bishop => {
                        constants::bishop_moves(m.from(), self.board.occupation()).is_set(m.to())
                    }
                    Piece::Queen => (constants::rook_moves(m.from(), self.board.occupation())
                        | constants::bishop_moves(m.from(), self.board.occupation()))
                    .is_set(m.to()),
                }
            }
        }
    }

    #[inline]
    fn update_zobrist(&mut self, m: Move, p: Piece) {
        self.ply_info.zobrist ^= m.zobrist(&self.board, self.to_move);
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.ply_info.zobrist ^= constants::enpassant_zobrist(esq.file());
        }

        //Check if a new en passant flag is to be set
        if p == Piece::Pawn
            && m.from().relative(self.to_move).rank() == Rank::Second
            && m.to().relative(self.to_move).rank() == Rank::Fourth
        {
            self.ply_info.zobrist ^= constants::enpassant_zobrist(m.to().file());
            self.ply_info.ep_square = Some(m.to().file().ep_square().relative(self.to_move));
        } else {
            self.ply_info.ep_square = None;
        }
    }

    #[inline]
    fn update_castling_rights(&mut self, m: Move, piece: Piece) {
        if self.ply_info.castling_rights == [[false; 2]; 2] {
            return;
        }

        //If the king was moves we lose all castling rights
        if piece == Piece::King {
            if self.ply_info.castling_rights[self.to_move as usize][0] {
                self.ply_info.zobrist ^=
                    constants::castle_zobrist(Square::G1.relative(self.to_move));
            }

            if self.ply_info.castling_rights[self.to_move as usize][1] {
                self.ply_info.zobrist ^=
                    constants::castle_zobrist(Square::C1.relative(self.to_move));
            }

            self.ply_info.castling_rights[self.to_move as usize] = [false, false];
        }

        //Check if white's rooks are moved _or_ captured
        if self.ply_info.castling_rights[0][1] && (m.to() == Square::A1 || m.from() == Square::A1) {
            self.ply_info.castling_rights[0][1] = false;
            self.ply_info.zobrist ^= constants::castle_zobrist(Square::C1);
        }
        if self.ply_info.castling_rights[0][0] && (m.to() == Square::H1 || m.from() == Square::H1) {
            self.ply_info.castling_rights[0][0] = false;
            self.ply_info.zobrist ^= constants::castle_zobrist(Square::G1);
        }

        //Check if black's rooks are moved _or_ captured
        if self.ply_info.castling_rights[1][0] && (m.to() == Square::H8 || m.from() == Square::H8) {
            self.ply_info.castling_rights[1][0] = false;
            self.ply_info.zobrist ^= constants::castle_zobrist(Square::G8);
        } else if self.ply_info.castling_rights[1][1]
            && (m.to() == Square::A8 || m.from() == Square::A8)
        {
            self.ply_info.castling_rights[1][1] = false;
            self.ply_info.zobrist ^= constants::castle_zobrist(Square::C8);
        }
    }

    pub fn do_move(&mut self, m: Move) {
        let piece = self.board.piece_at(m.from()).unwrap();

        self.history.push(self.ply_info);

        self.ply_info.capture = if m.typ() == MoveType::Enpassant {
            Some(Piece::Pawn)
        } else if m.is_capture() {
            self.board.piece_at(m.to())
        } else {
            None
        };

        //Update the zobrist numbers (except castling numbers)
        self.update_zobrist(m, piece);

        //Actually do the move on the board
        self.board.do_move(m);

        self.update_castling_rights(m, piece);

        //Adjust other state information
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        self.ply_info.last_move = m;

        self.to_move = self.to_move.other();
        self.ply_info.zobrist ^= constants::color_zobrist();

        if piece == Piece::Pawn || matches!(m.typ(), MoveType::Capture) {
            self.ply_info.rule_50_count = 0;
        } else {
            self.ply_info.rule_50_count += 1;
        }
    }

    pub fn undo_move(&mut self) {
        assert!(self.ply_info.last_move != Move::ZERO);
        assert_eq!(
            self.ply_info.last_move.is_capture(),
            self.ply_info.capture.is_some()
        );
        self.board
            .undo_move(self.ply_info.last_move, self.ply_info.capture);
        self.ply_info = self.history.pop().unwrap();
        self.to_move = self.to_move.other();
    }

    pub fn do_null_move(&mut self) {
        self.history.push(self.ply_info);
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.ply_info.zobrist ^= constants::enpassant_zobrist(esq.file());
        }

        self.to_move = self.to_move.other();
        self.ply_info.last_move = Move::ZERO;
        self.ply_info.zobrist ^= constants::color_zobrist();

        //Reset state information
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        self.ply_info.rule_50_count = 0;
        self.ply_info.ep_square = None;
    }

    pub fn undo_null_move(&mut self) {
        self.ply_info = self.history.pop().unwrap();
        self.to_move = self.to_move.other();
    }

    #[inline]
    pub fn in_check(&mut self) -> bool {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        !self.ply_info.king_attackers.is_empty()
    }

    pub fn will_repeat(&self, m: Move) -> bool {
        if m.typ() != MoveType::Normal || self.ply_info.ep_square.is_some() {
            return false;
        }
        let zobrist = self.ply_info.zobrist
            ^ m.zobrist(&self.board, self.to_move)
            ^ constants::color_zobrist();
        self.history.iter().any(|info| info.zobrist == zobrist)
    }

    #[allow(dead_code)]
    pub fn is_repetition(&self) -> bool {
        self.history
            .iter()
            .any(|info| info.zobrist == self.ply_info.zobrist)
    }

    // Check if we have a repetition in the last plys
    pub fn is_repetition_in_plys(&self, plys: usize) -> bool {
        self.history
            .iter()
            .rev()
            .take(plys)
            .any(|info| info.zobrist == self.ply_info.zobrist)
    }

    #[allow(dead_code)]
    pub fn is_threefold(&self) -> bool {
        self.history
            .iter()
            .filter(|info| info.zobrist == self.ply_info.zobrist)
            .count()
            > 1
    }

    #[inline]
    pub fn piece_count(&self, c: Color, p: Piece) -> i32 {
        self.board.get_bb(c, p).count() as i32
    }

    #[inline]
    fn material_count(&self, c: Color) -> i32 {
        constants::piece_value(Piece::Pawn) * self.piece_count(c, Piece::Pawn)
            + constants::piece_value(Piece::Bishop) * self.piece_count(c, Piece::Bishop)
            + constants::piece_value(Piece::Knight) * self.piece_count(c, Piece::Knight)
            + constants::piece_value(Piece::Rook) * self.piece_count(c, Piece::Rook)
            + constants::piece_value(Piece::Queen) * self.piece_count(c, Piece::Queen)
    }

    #[inline]
    pub fn material_balance(&self) -> i32 {
        self.material_count(self.to_move) - self.material_count(self.to_move.other())
    }

    #[inline]
    pub fn zobrist_hash(&self) -> u64 {
        self.ply_info.zobrist
    }

    #[allow(dead_code)]
    pub fn hard_pins(&mut self) -> BitBoard {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        self.ply_info.pinned_pieces
    }

    #[inline]
    pub fn last_move(&self) -> Move {
        self.ply_info.last_move
    }

    //X-ray attacks.
    //We ignore en passant here! Attackers are sorted by value!
    pub fn see(&self, m: Move) -> i32 {
        let target = m.to();
        let target_piece = match m.typ() {
            MoveType::Capture => self.board.piece_at(target).unwrap(),
            _ => return 0,
        };

        let mut color = self.to_move;
        let mut occupation = self.board.occupation();

        //We initialize pawn and knight attacks, since they do not depend on the occupation.
        let mut attackers = (constants::pawn_attacks(target, Color::White)
            & self.board.get_bb(Color::Black, Piece::Pawn))
            | (constants::pawn_attacks(target, Color::Black)
                & self.board.get_bb(Color::White, Piece::Pawn));

        attackers |= constants::knight_moves(target) & self.board.get_piece_bb(Piece::Knight);

        attackers |= m.from().into();

        //We guess that most exchanges will feature less than ten pieces, which seems a safe
        //assumption
        let mut gain = Vec::with_capacity(10);
        let mut taker = self.board.piece_at(m.from()).unwrap();

        gain.push(constants::piece_value(target_piece));

        let mut from = m.from();
        loop {
            let last_gain = *gain.last().unwrap_or(&0);

            gain.push(constants::piece_value(taker) - last_gain);

            if std::cmp::max(-last_gain, constants::piece_value(taker) - last_gain) < 0 {
                break;
            }

            occupation ^= from.into();
            attackers ^= from.into();

            color = color.other();

            attackers |= constants::bishop_moves(target, occupation)
                & (self.board.get_piece_bb(Piece::Bishop) | self.board.get_piece_bb(Piece::Queen))
                & occupation;

            attackers |= constants::rook_moves(target, occupation)
                & (self.board.get_piece_bb(Piece::Rook) | self.board.get_piece_bb(Piece::Queen))
                & occupation; //Do not accidentally include already used pieces again

            let next = match self.least_valuable_attacker(attackers, color) {
                Some(a) => a,
                None => break,
            };

            taker = next.1;
            from = next.0;
        }

        for d in (1..gain.len() - 1).rev() {
            gain[d - 1] = -std::cmp::max(-gain[d], gain[d - 1]);
        }

        gain[0]
    }

    //TODO: Should this _really_ be here? But where else to put it?
    //Helper for SEE
    #[inline]
    fn least_valuable_attacker(&self, attackers: BitBoard, c: Color) -> Option<(Square, Piece)> {
        let pieces = [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
        ];

        for p in pieces {
            let p_attackers = attackers & self.board.get_bb(c, p);
            if !p_attackers.is_empty() {
                return Some((p_attackers.least_square(), p));
            }
        }

        None
    }

    #[inline]
    #[allow(dead_code)]
    pub fn gives_check(&self, m: Move) -> bool {
        let kpos = self
            .board
            .get_bb(self.color().other(), Piece::King)
            .least_square();

        let final_piece = if let Some(p) = m.typ().promotion_piece() {
            p
        } else {
            self.board.piece_at(m.from()).unwrap()
        };

        match final_piece {
            Piece::Knight => constants::knight_moves(kpos).is_set(m.to()),
            Piece::Bishop => self.bishop_moves(kpos).is_set(m.to()),
            Piece::Rook => self.rook_moves(kpos).is_set(m.to()),
            Piece::Queen => (self.rook_moves(kpos) | self.bishop_moves(kpos)).is_set(m.to()),
            Piece::Pawn => {
                let attacked_sqs = constants::pawn_attacks(m.to(), self.to_move);
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
    let mov = Move::new(Square::E1, Square::E5, MoveType::Capture);
    assert_eq!(pos.see(mov), constants::piece_value(Piece::Pawn));
    let pos2 = Position::from_fen(String::from(
        "1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 0",
    ))
    .unwrap();
    let mov2 = Move::new(Square::D3, Square::E5, MoveType::Capture);
    assert_eq!(
        pos2.see(mov2),
        constants::piece_value(Piece::Pawn) - constants::piece_value(Piece::Knight)
    );
}

#[test]
fn compress_and_decompress_move() {
    let m = Move::new(Square::A1, Square::B1, MoveType::Normal);
    let m2 = Move::decompress(m.compress());
    assert!(m == m2);

    let m = Move::new(Square::G7, Square::H8, MoveType::PromotionCaptureB);
    let m2 = Move::decompress(m.compress());
    assert!(m == m2);

    let m = Move::new(Square::G7, Square::H8, MoveType::Capture);
    let m2 = Move::decompress(m.compress());
    assert!(m == m2);
}
