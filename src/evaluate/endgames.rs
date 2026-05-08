use crate::chess::{BitBoard, Position, Square, Color, Piece};

pub enum State {
    Won,
    Lost,
    Drawn,
    Unknown,
}

pub fn kkx(pos: &Position) -> State {
    let decisive_out = if pos.get_board().get_color_bb(pos.color()).count() > 1 {
                            State::Won
                        } else {
                            State::Lost
                        };
    if !pos.get_board().get_piece_bb(Piece::Pawn).is_empty() {
        kkp(pos)
    } else if !(pos.get_board().get_piece_bb(Piece::Rook)
                | pos.get_board().get_piece_bb(Piece::Queen)).is_empty() {
        decisive_out
    } else {
        State::Drawn
    }
}

//king and pawn vs king
pub fn kkp(pos: &Position) -> State {
    State::Unknown
}

//king with two minor pieces vs king
pub fn kkmm(pos: &Position) -> State {
    let decisive_out = if pos.get_board().get_color_bb(pos.color()).count() > 1 {
                            State::Won
                        } else {
                            State::Lost
                        };
    if pos.get_board().get_piece_bb(Piece::Bishop).is_empty() {
        State::Drawn
    } else {
        decisive_out
    }
}

