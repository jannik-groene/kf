mod bitboard;
mod board;
mod moves;

use std::iter::Iterator;

use crate::constants;
pub use bitboard::{BitBoard, File, Rank, Square};
pub use board::Board;
pub use moves::{Move, MoveList, MoveType};

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

fn get_zobrist(board: &Board) -> u64 {
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
        for sq in board.get_bb(Color::White, piece) {
            zobrist ^= constants::piece_zobrist(piece, Color::White, sq);
        }
        for sq in board.get_bb(Color::Black, piece) {
            zobrist ^= constants::piece_zobrist(piece, Color::Black, sq);
        }
    }
    zobrist
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
    to_move: Color,
    zobrist: u64,      //Zobrist-Hash
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

        for i in [Square::C1, Square::C8, Square::G1, Square::G8] {
            zobrist ^= constants::castle_zobrist(i);
        }

        Position {
            board,
            to_move: Color::White,
            ply_info: PlyInfo {
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                ep_square: None,
            },
            zobrist,
        }
    }

    pub fn from_fen(fen: String) -> Option<Position> {
        //First set up the pieces
        let mut fen_parts = fen.split_whitespace();

        let b = from_fen(fen_parts.next().unwrap())?;

        let mut pos = Position {
            board: b,
            to_move: Color::White,
            ply_info: PlyInfo {
                castling_rights: [[true, true], [true, true]],
                rule_50_count: 0,
                attacked_squares: BitBoard::EMPTY,
                king_attackers: BitBoard::EMPTY,
                pinned_pieces: BitBoard::EMPTY,
                ep_square: None,
            },
            zobrist: 0,
        };

        pos.zobrist = get_zobrist(&b);

        //Enter who is to move
        match fen_parts.next() {
            Some(p) => {
                if p == "w" {
                    pos.to_move = Color::White;
                } else if p == "b" {
                    pos.zobrist ^= constants::color_zobrist();
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
                pos.zobrist ^= constants::castle_zobrist(Square::G1);
            }
            if p.contains('Q') {
                castling_legal[0][1] = true;
                pos.zobrist ^= constants::castle_zobrist(Square::C1);
            }
            if p.contains('k') {
                castling_legal[1][0] = true;
                pos.zobrist ^= constants::castle_zobrist(Square::G8);
            }
            if p.contains('q') {
                castling_legal[1][1] = true;
                pos.zobrist ^= constants::castle_zobrist(Square::G8);
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
                    pos.zobrist ^= constants::enpassant_zobrist(sq.file());
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
    pub fn from_move(&self, m: Move) -> Position {
        let mut pos = self.clone();
        pos.do_move(m);
        pos
    }

    #[inline]
    pub fn get_board(&self) -> &Board {
        &self.board
    }

    #[inline]
    pub fn rule_50_count(&self) -> u8 {
        self.ply_info.rule_50_count
    }

    #[inline]
    pub fn color(&self) -> Color {
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
    fn get_piece_moves(
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

            for m in pmoves {
                //SAFETY: The ArrayVec capacity exceeds the maximal move number of 218
                unsafe {
                    moves.push_unchecked(Move {
                        from: pos,
                        to: m,
                        piece: p,
                        typ: if !self.board.occupation().is_set(m) {
                            MoveType::Normal
                        } else {
                            MoveType::Capture
                        },
                    });
                }
            }
        }
    }

    #[inline]
    fn handle_en_passant(&mut self, moves: &mut MoveList, check_mask: BitBoard) {
        if let Some(esq) = self.ply_info.ep_square {
            let cap_sq = esq.advance(self.to_move.other());

            if check_mask.is_set(esq) || check_mask.is_set(cap_sq) {
                let cands = constants::pawn_attacks(esq, self.to_move.other())
                    & self.board.get_bb(self.to_move, Piece::Pawn);

                for cand in cands {
                    let ksq = self.board.king_square(self.to_move);

                    let pin_ray = constants::ray(cand, ksq);

                    //Check if we expose the king by taking en passant
                    //See if our pawn is pinned
                    if (self.ply_info.pinned_pieces.is_set(cand) && !pin_ray.is_set(esq))
                        || (self.ply_info.pinned_pieces.is_set(cap_sq)
                            && !constants::ray(cap_sq, ksq).is_set(esq))
                    {
                        continue;
                    //Check for a double pin by a rook or queen.
                    } else if pin_ray.is_set(cap_sq) {
                        let mut occupation = self.board.occupation();

                        occupation.unset(cap_sq);
                        occupation.unset(cand);

                        let k_ray = pin_ray & constants::rook_moves(ksq, occupation);

                        if !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Rook))
                            .is_empty()
                            || !(k_ray & self.board.get_bb(self.to_move.other(), Piece::Queen))
                                .is_empty()
                        {
                            continue;
                        }
                    }

                    unsafe {
                        moves.push_unchecked(Move {
                            from: cand,
                            to: esq,
                            piece: Piece::Pawn,
                            typ: MoveType::Enpassant,
                        });
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
                moves.push_unchecked(Move {
                    from: Square::E1.relative(self.to_move),
                    to: Square::G1.relative(self.to_move),
                    piece: Piece::King,
                    typ: MoveType::Castle,
                });
            }
        }

        //Check for queenside castling.
        if self.ply_info.castling_rights[self.to_move as usize][1]
            && (self.ply_info.attacked_squares & QUEEN_CASTLE_CHECK_MASK[self.to_move as usize])
                .is_empty()
            && (self.board.occupation() & QUEEN_CASTLE_MATERIAL_MASK[self.to_move as usize])
                .is_empty()
        {
            moves.push(Move {
                from: Square::E1.relative(self.to_move),
                to: Square::C1.relative(self.to_move),
                piece: Piece::King,
                typ: MoveType::Castle,
            });
        }
    }

    #[inline]
    fn get_pawn_moves(&self, moves: &mut MoveList, check_mask: BitBoard) {
        let pawns = self.board.get_bb(self.to_move, Piece::Pawn);

        let ksq = self.board.get_bb(self.to_move, Piece::King).least_square();

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
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::Normal,
                });
            }
        }

        //Push double pawn advances
        for to in double_advances & check_mask {
            let from = to.shifted_by(-direction * 16);

            if self.ply_info.pinned_pieces.is_set(from) && !constants::ray(ksq, from).is_set(to) {
                continue;
            }

            unsafe {
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::Normal,
                });
            }
        }

        //Push pawn promotions
        for to in self.pawn_promotions(self.to_move) & check_mask {
            let from = to.shifted_by(-direction * 8);

            if self.ply_info.pinned_pieces.is_set(from) {
                continue;
            }

            unsafe {
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::PromotionN,
                });
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::PromotionB,
                });
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::PromotionR,
                });
                moves.push_unchecked(Move {
                    piece: Piece::Pawn,
                    from,
                    to,
                    typ: MoveType::PromotionQ,
                });
            }
        }

        //Push pawn captures
        for pawn in pawns {
            let mut attacks = (constants::pawn_attacks(pawn, self.to_move)
                & self.board.get_color_bb(self.to_move.other()))
                & check_mask;

            if self.ply_info.pinned_pieces.is_set(pawn) {
                attacks &= constants::ray(ksq, pawn);
            }

            for m in attacks {
                if m.rank().relative(self.to_move) == Rank::Eighth {
                    unsafe {
                        moves.push_unchecked(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::PromotionCaptureN,
                        });
                        moves.push_unchecked(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::PromotionCaptureB,
                        });
                        moves.push_unchecked(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::PromotionCaptureR,
                        });
                        moves.push_unchecked(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::PromotionCaptureQ,
                        });
                    }
                } else {
                    unsafe {
                        moves.push_unchecked(Move {
                            from: pawn,
                            to: m,
                            piece: Piece::Pawn,
                            typ: MoveType::Capture,
                        });
                    }
                }
            }
        }
    }

    pub fn get_moves(&mut self) -> MoveList {
        let mut moves = MoveList::new();

        if self.ply_info.rule_50_count == 100 || self.board.occupation().count() == 2 {
            return moves;
        }

        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }

        //Generate King moves first (except castling)
        let mut king_moves =
            constants::king_moves(self.board.get_bb(self.to_move, Piece::King).least_square());
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
                    MoveType::Capture
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
            constants::ray_between(
                self.ply_info.king_attackers.least_square(),
                self.board.king_square(self.to_move),
            ) | self.ply_info.king_attackers
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
            |sq: Square| -> BitBoard { constants::knight_moves(sq) & check_mask },
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

    #[inline]
    fn update_zobrist(&mut self, m: Move) {
        self.zobrist ^= m.zobrist(&self.board, self.to_move);
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.zobrist ^= constants::enpassant_zobrist(esq.file());
        }

        //Check if a new en passant flag is to be set
        if m.piece == Piece::Pawn
            && m.from.relative(self.to_move).rank() == Rank::Second
            && m.to.relative(self.to_move).rank() == Rank::Fourth
        {
            self.zobrist ^= constants::enpassant_zobrist(m.to.file());
            self.ply_info.ep_square = Some(m.to.file().ep_square().relative(self.to_move));
        } else {
            self.ply_info.ep_square = None;
        }
    }

    #[inline]
    fn update_castling_rights(&mut self, m: Move) {
        //If the king was moves we lose all castling rights
        if m.piece == Piece::King {
            if self.ply_info.castling_rights[self.to_move as usize][0] {
                self.zobrist ^= constants::castle_zobrist(Square::G1.relative(self.to_move));
            }

            if self.ply_info.castling_rights[self.to_move as usize][1] {
                self.zobrist ^= constants::castle_zobrist(Square::C1.relative(self.to_move));
            }

            self.ply_info.castling_rights[self.to_move as usize] = [false, false];
        }

        //Check if white's rooks are moved _or_ captured
        if self.ply_info.castling_rights[0][1] && (m.to == Square::A1 || m.from == Square::A1) {
            self.ply_info.castling_rights[0][1] = false;
            self.zobrist ^= constants::castle_zobrist(Square::C1);
        } else if self.ply_info.castling_rights[0][0]
            && (m.to == Square::H1 || m.from == Square::H1)
        {
            self.ply_info.castling_rights[0][0] = false;
            self.zobrist ^= constants::castle_zobrist(Square::G1);
        }

        //Check if black's rooks are moved _or_ captured
        if self.ply_info.castling_rights[1][0] && (m.to == Square::H8 || m.from == Square::H8) {
            self.ply_info.castling_rights[1][0] = false;
            self.zobrist ^= constants::castle_zobrist(Square::G8);
        } else if self.ply_info.castling_rights[1][1]
            && (m.to == Square::A8 || m.from == Square::A8)
        {
            self.ply_info.castling_rights[1][1] = false;
            self.zobrist ^= constants::castle_zobrist(Square::C8);
        }
    }

    pub fn do_move(&mut self, m: Move) {
        //Actually do the move on the board
        self.board.do_move(m);

        //Update the zobrist numbers (except castling numbers)
        self.update_zobrist(m);

        self.update_castling_rights(m);

        //Adjust other state information
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;

        self.to_move = self.to_move.other();
        self.zobrist ^= constants::color_zobrist();

        if m.piece == Piece::Pawn || matches!(m.typ, MoveType::Capture) {
            self.ply_info.rule_50_count = 0;
        } else {
            self.ply_info.rule_50_count += 1;
        }
    }

    pub fn do_null_move(&mut self) {
        //Unset the Zobrist en passant flag, if necessary
        if let Some(esq) = self.ply_info.ep_square {
            self.zobrist ^= constants::enpassant_zobrist(esq.file());
        }

        self.to_move = self.to_move.other();
        self.zobrist ^= constants::color_zobrist();

        //Reset state information
        self.ply_info.attacked_squares = BitBoard::EMPTY;
        self.ply_info.pinned_pieces = BitBoard::EMPTY;
        self.ply_info.king_attackers = BitBoard::EMPTY;
        self.ply_info.rule_50_count = 0;
        self.ply_info.ep_square = None;
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
        self.zobrist
    }

    #[allow(dead_code)]
    pub fn hard_pins(&mut self) -> BitBoard {
        if self.ply_info.attacked_squares.is_empty() {
            self.generate_attack_table();
        }
        self.ply_info.pinned_pieces
    }

    //X-ray attacks.
    //We ignore en passant here! Attackers are sorted by value!
    pub fn see(&self, m: Move) -> i32 {
        let target = m.to;
        let target_piece = match m.typ {
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

        attackers |= m.from.into();

        //We guess that most exchanges will feature less than ten pieces, which seems a safe
        //assumption
        let mut gain = Vec::with_capacity(10);
        let mut taker = m.piece;

        gain.push(constants::piece_value(target_piece));

        let mut from = m.from;
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
    pub fn gives_check(&self, m: &Move) -> bool {
        let kpos = self
            .board
            .get_bb(self.color().other(), Piece::King)
            .least_square();

        let final_piece = if let Some(p) = m.typ.promotion_piece() {p} else {m.piece}; 

        match final_piece {
            Piece::Knight => constants::knight_moves(kpos).is_set(m.to),
            Piece::Bishop => self.bishop_moves(kpos).is_set(m.to),
            Piece::Rook => self.rook_moves(kpos).is_set(m.to),
            Piece::Queen => (self.rook_moves(kpos) | self.bishop_moves(kpos)).is_set(m.to),
            Piece::Pawn => {
                let attacked_sqs = constants::pawn_attacks(m.to, self.to_move);
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
        typ: MoveType::Capture,
        piece: Piece::Rook,
    };
    assert_eq!(pos.see(mov), constants::piece_value(Piece::Pawn));
    let pos2 = Position::from_fen(String::from(
        "1k1r3q/1ppn3p/p4b2/4p3/8/P2N2P1/1PP1R1BP/2K1Q3 w - - 0 0",
    ))
    .unwrap();
    let mov2 = Move {
        from: Square::D3,
        to: Square::E5,
        typ: MoveType::Capture,
        piece: Piece::Knight,
    };
    assert_eq!(pos2.see(mov2), constants::piece_value(Piece::Pawn) - constants::piece_value(Piece::Knight));
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
        let m2 = Move::decompress(m.compress());
        assert!(m == m2.unwrap());
    }
    {
        let m = Move {
            from: Square::G7,
            to: Square::H8,
            piece: Piece::Pawn,
            typ: MoveType::PromotionCaptureB,
        };
        let m2 = Move::decompress(m.compress());
        assert!(m == m2.unwrap());
    }
    for p in pieces.iter() {
        let m = Move {
            from: Square::G7,
            to: Square::H8,
            piece: *p,
            typ: MoveType::Capture,
        };
        let m2 = Move::decompress(m.compress());
        assert!(m == m2.unwrap());
    }
}
