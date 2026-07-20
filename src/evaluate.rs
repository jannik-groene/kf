pub mod eval;
pub mod nnue;

use crate::chess::{Color, Piece, Position};
pub use eval::Bound;

#[allow(dead_code)]
pub fn has_pawns(pos: &Position) -> bool {
    !(pos.board.get_bb(Color::White, Piece::Pawn) | pos.board.get_bb(Color::Black, Piece::Pawn))
        .is_empty()
}

#[inline]
pub fn has_minor_pieces(pos: &Position) -> bool {
    !(pos.board.get_piece_bb(Piece::Bishop) | pos.board.get_piece_bb(Piece::Knight)).is_empty()
}

#[inline]
pub fn has_major_pieces(pos: &Position) -> bool {
    !(pos.board.get_piece_bb(Piece::Rook) | pos.board.get_piece_bb(Piece::Queen)).is_empty()
}

#[inline]
pub fn is_material_draw(pos: &Position) -> bool {
    pos.board.occupation().count() == 2
        || (pos.board.occupation().count() == 3 && has_minor_pieces(pos))
}
