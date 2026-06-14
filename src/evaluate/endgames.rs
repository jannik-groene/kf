#![allow(clippy::all)]
#![allow(warnings)]
use crate::{
    chess::{BitBoard, Color, Piece, Position, Rank, Square},
    constants,
};

pub enum State {
    Won(i32),
    Lost(i32),
    Drawn,
    Unknown,
}

fn distance(x: Square, y: Square) -> i32 {
    (x.file() as i32 - y.file() as i32)
        .abs()
        .max((x.rank() as i32 - y.rank() as i32).abs())
}

pub fn kkx(pos: &Position) -> State {
    if !pos.get_board().get_piece_bb(Piece::Queen).is_empty()
        || !pos.get_board().get_piece_bb(Piece::Rook).is_empty()
    {
        kkr_or_kkq(pos)
    } else if !pos.get_board().get_piece_bb(Piece::Pawn).is_empty() {
        kkp(pos)
    } else {
        State::Drawn
    }
}

// We pretend that queens are just the same as rooks for mating. Search should catch the difference
// (stalemate traps etc).
pub fn kkr_or_kkq(pos: &Position) -> State {
    let wcolor = if pos.get_board().get_color_bb(Color::White).count() > 1 {
        Color::White
    } else {
        Color::Black
    };

    let piece = if pos.get_board().get_piece_bb(Piece::Rook).is_empty() {
        Piece::Queen
    } else {
        Piece::Rook
    };

    let w_kpos = pos.get_board().get_bb(wcolor, Piece::King).least_square();
    let l_kpos = pos
        .get_board()
        .get_bb(wcolor.other(), Piece::King)
        .least_square();
    let p_pos = pos.get_board().get_piece_bb(piece).least_square();

    // Area of the square that the losing king is confined to
    let lk_sq_width = if p_pos.file() as i32 > l_kpos.file() as i32 {
        p_pos.file() as i32
    } else {
        7 - p_pos.file() as i32
    };
    let lk_sq_height = if p_pos.rank() as i32 > l_kpos.rank() as i32 {
        p_pos.rank() as i32
    } else {
        7 - p_pos.rank() as i32
    };
    let area_score = 49 - lk_sq_width * lk_sq_height;

    // Manhattan distance of the king and rook/queen
    let man_dist = distance(w_kpos, p_pos);

    let k_dist = distance(w_kpos, l_kpos);

    // We add the piece value so we are always promoting when possible
    if pos.color() == wcolor {
        State::Won(constants::piece_value(piece) + area_score * 10 - man_dist - k_dist)
    } else {
        State::Lost(constants::piece_value(piece) + area_score * 10 - man_dist - k_dist)
    }
}

//king and pawn vs king
pub fn kkp(pos: &Position) -> State {
    let wcolor = if pos.get_board().get_color_bb(Color::White).count() > 1 {
        Color::White
    } else {
        Color::Black
    };
    let to_move = pos.color();
    let decisive_out = if to_move == wcolor {
        State::Won
    } else {
        State::Lost
    };

    let w_kpos = pos.get_board().get_bb(wcolor, Piece::King).least_square();
    let l_kpos = pos
        .get_board()
        .get_bb(wcolor.other(), Piece::King)
        .least_square();
    let p_pos = pos.get_board().get_piece_bb(Piece::Pawn).least_square();

    // Triviality check: we can just queen by running up.
    let p_rank = p_pos.rank().relative(wcolor) as i32;
    let lk_rank = l_kpos.rank().relative(wcolor) as i32;
    let prom_dist = if p_rank == 1 { 5 } else { 7 - p_rank };
    let lk_dist = distance(
        (BitBoard::from_file(p_pos.file()) & BitBoard::from_rank(Rank::Eighth.relative(wcolor)))
            .least_square(),
        l_kpos,
    );

    if ((to_move == wcolor && lk_dist > prom_dist) || (lk_dist - 1 > prom_dist))
        && (w_kpos.file() != p_pos.file()
            || (w_kpos.rank().relative(wcolor) as u8) < p_pos.rank().relative(wcolor) as u8)
    {
        return decisive_out(p_rank * 50);
    }

    // Basic blocking by the black king
    if l_kpos == p_pos.advance(wcolor) && (lk_rank != 7 || to_move == wcolor) {
        return State::Drawn;
    }

    if p_rank < 6 && l_kpos == p_pos.advance(wcolor).advance(wcolor) {
        return State::Drawn;
    }

    // Win conditions for white
    let wk_rank = w_kpos.rank().relative(wcolor) as i32;
    let wk_in_front = w_kpos.file() == p_pos.file() && wk_rank > p_rank;
    let opposition = !(constants::rook_moves(w_kpos, BitBoard::EMPTY)
        & pos.get_board().get_color_bb(wcolor.other()))
    .is_empty()
        && distance(l_kpos, w_kpos) == 2
        && to_move != wcolor;
    let sixth = w_kpos.rank().relative(wcolor) == Rank::Sixth;

    if distance(w_kpos, p_pos) < distance(l_kpos, p_pos) + (to_move == wcolor) as i32
        && ((opposition && wk_in_front) || (opposition && sixth) || (sixth && wk_in_front))
    {
        return decisive_out(p_rank * 50);
    }

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
        decisive_out(0)
    }
}
