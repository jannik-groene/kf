include!(concat!(env!("OUT_DIR"), "/constants.rs"));

use crate::bitboard::{BitBoard, Square, File};
use crate::chess::{Color, Piece};

#[inline]
pub const fn piece_zobrist(p: Piece, c: Color, sq: Square) -> u64 {
    match (c,p) {
        (Color::White, Piece::King) => WHITE_KING_ZOBRIST[sq as usize],
        (Color::White, Piece::Queen) => WHITE_QUEEN_ZOBRIST[sq as usize],
        (Color::White, Piece::Bishop) => WHITE_BISHOP_ZOBRIST[sq as usize],
        (Color::White, Piece::Knight) => WHITE_KNIGHT_ZOBRIST[sq as usize],
        (Color::White, Piece::Rook) => WHITE_ROOK_ZOBRIST[sq as usize],
        (Color::White, Piece::Pawn) => WHITE_PAWN_ZOBRIST[sq as usize],
        (Color::Black, Piece::King) => BLACK_KING_ZOBRIST[sq as usize],
        (Color::Black, Piece::Queen) => BLACK_QUEEN_ZOBRIST[sq as usize],
        (Color::Black, Piece::Bishop) => BLACK_BISHOP_ZOBRIST[sq as usize],
        (Color::Black, Piece::Knight) => BLACK_KNIGHT_ZOBRIST[sq as usize],
        (Color::Black, Piece::Rook) => BLACK_ROOK_ZOBRIST[sq as usize],
        (Color::Black, Piece::Pawn) => BLACK_PAWN_ZOBRIST[sq as usize],
        _ => panic!("Invalid piece"),
    }
}

#[inline]
pub const fn castle_zobrist(ksq: Square) -> u64 {
    match ksq {
        Square::C1 => CASTLING_ZOBRIST[0],
        Square::G1 => CASTLING_ZOBRIST[1],
        Square::C8 => CASTLING_ZOBRIST[2],
        Square::G8 => CASTLING_ZOBRIST[3],
        _ => panic!("Invalid castling Square")
    }
}

#[inline]
pub const fn enpassant_zobrist(f: File) -> u64 {
    ENPASSANT_ZOBRIST[f as usize]
}

#[inline]
pub const fn color_zobrist() -> u64 {
    COLOR_ZOBRIST
}

#[inline]
pub fn rook_moves(from: Square, occupation: BitBoard) -> BitBoard {
    BitBoard::new(
        ROOK_ATTACKS[ROOK_ATTACK_OFFSETS[from as usize] + occupation.pext(ROOK_MASKS[from as usize])]
    )
}

#[inline]
pub fn bishop_moves(from: Square, occupation: BitBoard) -> BitBoard {
    BitBoard::new(
        BISHOP_ATTACKS[BISHOP_ATTACK_OFFSETS[from as usize] + occupation.pext(BISHOP_MASKS[from as usize])]
    )
}

#[inline]
pub const fn knight_moves(from: Square) -> BitBoard {
    BitBoard::new(
        KNIGHT_ATTACKS[from as usize]
    )
}

#[inline]
pub const fn king_moves(from: Square) -> BitBoard {
    BitBoard::new(
        NEIGHBOURS[from as usize]
    )
}

#[inline]
pub const fn pawn_attacks(from: Square, c: Color) -> BitBoard {
    BitBoard::new(
        PAWN_ATTACKS[c as usize][from as usize]
    )
}

#[inline]
pub fn ray(a: Square, b: Square) -> BitBoard {
    BitBoard::new(
        RAYS[a as usize][b as usize]
    )
}

#[inline]
pub fn ray_between(from: Square, to: Square) -> BitBoard {
    BitBoard::new(
        CONNECTING_RAYS[from as usize][to as usize]
    )
}

#[inline]
pub const fn neighbours(sq: Square) -> BitBoard {
    BitBoard::new(
        NEIGHBOURS[sq as usize]
    )
}

#[inline]
pub const fn next_neighbours(sq: Square) -> BitBoard {
    BitBoard::new(
        NEXT_NEIGHBOURS[sq as usize]
    )
}

