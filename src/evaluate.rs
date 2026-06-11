mod endgames;
mod eval;
mod piecetables;

use crate::{
    chess::{BitBoard, Board, Color, File, Piece, Position, Square},
    constants,
};
pub use eval::{Bound, Eval, Value};

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

#[inline]
fn piece_table_value(p: Piece, c: Color, s: Square, phase: i32) -> i32 {
    let index: usize = match c {
        Color::White => s.into(),
        Color::Black => s.flipped().into(),
    };
    match p {
        Piece::Pawn => piecetables::PAWN_VALUES[index],
        Piece::Knight => piecetables::KNIGHT_VALUES[index],
        Piece::Bishop => piecetables::BISHOP_VALUES[index],
        Piece::Rook => piecetables::ROOK_VALUES[index],
        Piece::Queen => piecetables::QUEEN_VALUES[index],
        Piece::King => {
            (piecetables::KING_EARLY_VALUES[index] * phase
                + piecetables::KING_LATE_VALUES[index] * (OPENING_PHASE - phase))
                / OPENING_PHASE
        }
    }
}

#[inline]
fn pawn_attacks(pos: &Position, color: Color) -> (BitBoard, BitBoard) {
    let mut pawn_attacks_white = BitBoard::EMPTY;
    let mut pawn_attacks_black = BitBoard::EMPTY;

    for pawn in pos.board.get_bb(Color::White, Piece::Pawn) {
        pawn_attacks_white |= constants::pawn_attacks(pawn, Color::White);
    }
    for pawn in pos.board.get_bb(Color::Black, Piece::Pawn) {
        pawn_attacks_black |= constants::pawn_attacks(pawn, Color::Black);
    }

    match color {
        Color::White => (pawn_attacks_white, pawn_attacks_black),
        Color::Black => (pawn_attacks_black, pawn_attacks_white),
    }
}

#[inline]
fn evaluate_pawns(pos: &Position, phase: i32, color: Color) -> i32 {
    let mut res_early = 0;
    let mut res_late = 0;
    let mut res = 0;

    let pawns_us = pos.board.get_bb(color, Piece::Pawn);
    let pawns_them = pos.board.get_bb(color.other(), Piece::Pawn);

    let (guarded_us, guarded_them) = pawn_attacks(pos, color);

    for pawn in pawns_us {
        let file = BitBoard::from_file(pawn.file());
        let in_front = file & BitBoard::forward_of(pawn, color);
        let mut advance: BitBoard = pawn.into();
        advance = advance.shifted_forward(color);
        let behind = file ^ in_front;

        let neighbours = if pawn.file() == File::H {
            pawns_us & file.shifted_by(-1)
        } else if pawn.file() == File::A {
            pawns_us & file.shifted_by(1)
        } else {
            pawns_us & (file.shifted_by(1) | file.shifted_by(-1))
        };

        //0. Add the base value of the pawn
        res += piece_table_value(Piece::Pawn, color, pawn, phase);

        //1. Check if the pawn is doubled (or worse).
        let pawns_us_on_file = file & pawns_us;
        res_early -= (pawns_us_on_file.count() as i32 - 1) * 10;
        res_late -= (pawns_us_on_file.count() as i32 - 1) * 20;

        //2. Check if the pawn is isolated
        if neighbours.is_empty() {
            if pawn.file() == File::A || pawn.file() == File::H {
                res -= 15;
            } else {
                res -= 25;
            }
        }
        //3. Check for backward pawn
        if !(in_front & guarded_them).is_empty() && ((behind | advance) & guarded_us).is_empty() {
            res -= 20;
        }

        //4. Check if we have a passer
        if (in_front & (pawns_them | guarded_them | pawns_us)).is_empty() {
            res_early += 50 / (7 - pawn.relative(color).rank() as i32);
            res_late += 150 / (7 - pawn.relative(color).rank() as i32);
        }

        //5. Check for candidate passers
        let supporters = (if pawn.file() != File::A {
            behind.shifted_by(-1)
        } else {
            BitBoard::EMPTY
        } | if pawn.file() != File::H {
            behind.shifted_by(1)
        } else {
            BitBoard::EMPTY
        }) & pawns_us;

        let sentries = (if pawn.file() != File::A {
            in_front.shifted_by(-1)
        } else {
            BitBoard::EMPTY
        } | if pawn.file() != File::H {
            in_front.shifted_by(1)
        } else {
            BitBoard::EMPTY
        }) & pawns_them;

        if (in_front & (pawns_them | pawns_us)).is_empty() && supporters.count() > sentries.count()
        {
            res_early += 15;
            res_late += 25;
        }

        //TODO: hidden passers?
    }

    res_late * (OPENING_PHASE - phase) / OPENING_PHASE + res_early * phase / OPENING_PHASE + res
}

#[inline]
fn evaluate_king_position(pos: &Position, phase: i32, color: Color) -> i32 {
    let king_pos = pos.get_board().king_square(color);
    //In the late game stages we want an active king. Maybe want to keep it somewhat central?
    piece_table_value(Piece::King, color, king_pos, phase)
}

#[inline]
fn evaluate_queens(pos: &Position, phase: i32, color: Color) -> i32 {
    let mut res: i32 = 0;
    //Do not run away with our Queen too fast
    for queen in pos.board.get_bb(color, Piece::Queen) {
        res += piece_table_value(Piece::Queen, color, queen, phase);
    }
    res
}

#[inline]
fn evaluate_rooks(pos: &Position, phase: i32, color: Color) -> i32 {
    let mut res: i32 = 0;
    for rook in pos.board.get_bb(color, Piece::Rook) {
        res += piece_table_value(Piece::Rook, color, rook, phase);
        let file = BitBoard::from_file(rook.file());
        //Rooks are good on semi-open and open files
        if (file & pos.board.get_bb(color, Piece::Pawn)).is_empty() {
            res += 10;
            if (file & pos.board.get_bb(color.other(), Piece::Pawn)).is_empty() {
                res += 30;
            }
        }
        //Doubled Rooks may be good
        //We give half the bonus and double count
        if (file & pos.board.get_bb(color, Piece::Rook)).count() > 1 {
            res += 5;
        }
    }
    res
}

#[inline]
fn evaluate_bishops(pos: &Position, phase: i32, color: Color) -> i32 {
    let mut res: i32 = 0;

    let bishops = pos.board.get_bb(color, Piece::Bishop);
    let pawns_us = pos.board.get_bb(color, Piece::Pawn);

    for bishop in bishops {
        res += piece_table_value(Piece::Bishop, color, bishop, phase);

        //reduce the value of the bishop, if it is blocked in by pawns
        let mut blocked_score = if Board::WHITE_SQUARES.is_set(bishop) {
            (pawns_us & Board::WHITE_SQUARES).count().saturating_sub(3) * 10
        } else {
            (pawns_us & Board::BLACK_SQUARES).count().saturating_sub(3) * 10
        } as i32;
        //if the bishop is in front of the pawns, the penalty is smaller
        if (BitBoard::forward_of(bishop, color) & pawns_us).count() < 2 {
            blocked_score /= 2;
        }
        res -= blocked_score;
    }

    //check for color weaknesses
    if (bishops & Board::WHITE_SQUARES).is_empty() {
        res -= 4_i32.saturating_sub((pawns_us & Board::WHITE_SQUARES).count() as i32) * 10 * phase
            / OPENING_PHASE;
    }
    if (bishops & Board::BLACK_SQUARES).is_empty() {
        res -= 4_i32.saturating_sub((pawns_us & Board::BLACK_SQUARES).count() as i32) * 10 * phase
            / OPENING_PHASE;
    }

    //give a bonus for the bishop pair
    if !(bishops & Board::WHITE_SQUARES).is_empty() && !(bishops & Board::BLACK_SQUARES).is_empty()
    {
        res += 50;
    }
    res
}

#[inline]
fn evaluate_knights(pos: &Position, phase: i32, color: Color) -> i32 {
    let mut res: i32 = 0;
    //we like knights in positions with many pawns
    let pawns =
        pos.piece_count(Color::White, Piece::Pawn) + pos.piece_count(Color::Black, Piece::Pawn);

    for knight in pos.board.get_bb(color, Piece::Knight) {
        res += piece_table_value(Piece::Knight, color, knight, phase);
        //bonus for closed positions
        res += 25 * pawns / 16;
    }
    res
}

//Evaluate the mobility in a position.
//This also evaluates the safety of the kings from attacks
#[inline]
fn evaluate_mobility(pos: &Position, phase: i32) -> i32 {
    let us = pos.color();
    let them = pos.color().other();

    const MOVE_VALUES: [[i32; 2]; 6] = [
        [0, 0],   //KING
        [10, 10], //QUEEN
        [20, 10], //BISHOP
        [10, 15], //KNIGHT
        [4, 0],   //ROOK
        [5, 2],   //PAWN
    ];
    let mut move_value_early = [0, 0];
    let mut move_value_late = [0, 0];

    const SAFETY_DISTANCE_ONE_MULTIPLIER: i32 = 2;
    const SAFETY_DISTANCE_TWO_MULTIPLIER: i32 = 1;
    const SAFETY_SCALE: i32 = 32;

    let mut safety_eval = [0, 0];
    let mut attackers = [0, 0];

    let king_neighbours = [
        constants::neighbours(pos.get_board().king_square(Color::White)),
        constants::neighbours(pos.get_board().king_square(Color::Black)),
    ];
    let king_nneighbours = [
        constants::next_neighbours(pos.get_board().king_square(Color::White)),
        constants::next_neighbours(pos.get_board().king_square(Color::Black)),
    ];
    let occupation = pos.get_board().occupation();
    let occ_by_color = [
        pos.get_board().get_color_bb(Color::White),
        pos.get_board().get_color_bb(Color::Black),
    ];

    let mut update_evals = |moves: BitBoard, piece, us: usize, them: usize| {
        move_value_early[us] += MOVE_VALUES[piece as usize][0] * moves.count() as i32;
        move_value_late[us] += MOVE_VALUES[piece as usize][1] * moves.count() as i32;

        let distance_one_count = (king_neighbours[them] & moves).count() as i32;
        let distance_two_count = (king_nneighbours[them] & moves).count() as i32;
        safety_eval[them] +=
            distance_one_count * SAFETY_DISTANCE_ONE_MULTIPLIER * attacker_weight(piece);
        safety_eval[them] +=
            distance_two_count * SAFETY_DISTANCE_TWO_MULTIPLIER * attacker_weight(piece);
        attackers[them] += distance_one_count + distance_two_count;
    };

    //Evaluate queen moves
    for queen in pos.get_board().get_bb(us, Piece::Queen) {
        let moves = (constants::rook_moves(queen, occupation)
            | constants::bishop_moves(queen, occupation))
            & !occ_by_color[us as usize];
        update_evals(moves, Piece::Queen, us as usize, them as usize);
    }
    for queen in pos.get_board().get_bb(them, Piece::Queen) {
        let moves = (constants::rook_moves(queen, occupation)
            | constants::bishop_moves(queen, occupation))
            & !occ_by_color[them as usize];
        update_evals(moves, Piece::Queen, them as usize, us as usize);
    }

    for bishop in pos.get_board().get_bb(us, Piece::Bishop) {
        let moves = constants::bishop_moves(bishop, occupation) & !occ_by_color[us as usize];
        update_evals(moves, Piece::Bishop, us as usize, them as usize);
    }
    for bishop in pos.get_board().get_bb(them, Piece::Bishop) {
        let moves = constants::bishop_moves(bishop, occupation) & !occ_by_color[them as usize];
        update_evals(moves, Piece::Bishop, them as usize, us as usize);
    }

    for knight in pos.get_board().get_bb(us, Piece::Knight) {
        let moves = constants::knight_moves(knight) & !occ_by_color[us as usize];
        update_evals(moves, Piece::Knight, us as usize, them as usize);
    }
    for knight in pos.get_board().get_bb(them, Piece::Knight) {
        let moves = constants::knight_moves(knight) & !occ_by_color[them as usize];
        update_evals(moves, Piece::Knight, them as usize, us as usize);
    }

    for rook in pos.get_board().get_bb(us, Piece::Rook) {
        let moves = constants::knight_moves(rook) & !occ_by_color[us as usize];
        update_evals(moves, Piece::Rook, us as usize, them as usize);
    }
    for rook in pos.get_board().get_bb(them, Piece::Rook) {
        let moves = constants::knight_moves(rook) & !occ_by_color[them as usize];
        update_evals(moves, Piece::Rook, them as usize, us as usize);
    }

    let (res_early_us, res_late_us) = if attackers[us as usize] > 1 {
        (
            ((-safety_eval[us as usize] * safety_eval[us as usize]) / SAFETY_SCALE).clamp(-500, 0),
            -safety_eval[us as usize],
        )
    } else {
        (0, 0)
    };
    let (res_early_them, res_late_them) = if attackers[them as usize] > 1 {
        (
            ((safety_eval[them as usize] * safety_eval[them as usize]) / SAFETY_SCALE)
                .clamp(0, 500),
            safety_eval[them as usize],
        )
    } else {
        (0, 0)
    };

    let (res_early, res_late) = (res_early_us + res_early_them, res_late_us + res_late_them);

    let total_early = if move_value_early[us as usize] + move_value_early[them as usize] != 0 {
        (move_value_early[us as usize] * 70)
            / (move_value_early[us as usize] + move_value_early[them as usize])
            - 35
    } else {
        0
    } + res_early;
    let total_late = if move_value_late[us as usize] + move_value_late[them as usize] != 0 {
        (move_value_late[us as usize] * 70)
            / (move_value_late[us as usize] + move_value_late[them as usize])
            - 35
    } else {
        0
    } + res_late;
    (total_early * phase + total_late * (OPENING_PHASE - phase)) / OPENING_PHASE
}

//use piece values as first approximation to phase
const PAWN_PHASE_WEIGHT: i32 = 1;
const BISHOP_PHASE_WEIGHT: i32 = 3;
const KNIGHT_PHASE_WEIGHT: i32 = 3;
const ROOK_PHASE_WEIGHT: i32 = 5;
const QUEEN_PHASE_WEIGHT: i32 = 9;
//In the starting position, this yields a phase factor of 78
const OPENING_PHASE: i32 = 16 * PAWN_PHASE_WEIGHT
    + 4 * BISHOP_PHASE_WEIGHT
    + 4 * KNIGHT_PHASE_WEIGHT
    + 4 * ROOK_PHASE_WEIGHT
    + 2 * QUEEN_PHASE_WEIGHT;

#[inline]
fn phase_factor(pos: &Position) -> i32 {
    let mut phase = 0;
    phase += PAWN_PHASE_WEIGHT * pos.get_board().get_piece_bb(Piece::Pawn).count() as i32;
    phase += BISHOP_PHASE_WEIGHT * pos.get_board().get_piece_bb(Piece::Bishop).count() as i32;
    phase += KNIGHT_PHASE_WEIGHT * pos.get_board().get_piece_bb(Piece::Knight).count() as i32;
    phase += ROOK_PHASE_WEIGHT * pos.get_board().get_piece_bb(Piece::Rook).count() as i32;
    phase += QUEEN_PHASE_WEIGHT * pos.get_board().get_piece_bb(Piece::Queen).count() as i32;
    phase
}

#[inline]
const fn attacker_weight(p: Piece) -> i32 {
    match p {
        Piece::Pawn => 1,
        Piece::Bishop | Piece::Knight => 2,
        Piece::Rook => 3,
        Piece::Queen => 5,
        _ => 0,
    }
}

#[inline]
fn evaluate_king_safety(pos: &Position, phase: i32, color: Color) -> i32 {
    let king_pos = pos.board.king_square(color);
    let mut res_early = 0;

    //In the early game we want pawns to shield our king.
    match color {
        Color::White => {
            if king_pos < Square::C1 {
                res_early += (BitBoard::new(0b111 << 8)
                    & pos.get_board().get_bb(Color::White, Piece::Pawn))
                .count() as i32
                    * 5;
            } else if king_pos <= Square::H1 && king_pos > Square::E1 {
                res_early += (BitBoard::new(0b11100000 << 8)
                    & pos.get_board().get_bb(Color::White, Piece::Pawn))
                .count() as i32
                    * 5;
            }
        }
        Color::Black => {
            if king_pos >= Square::A8 && king_pos < Square::C8 {
                res_early += (BitBoard::new(0b111 << 48)
                    & pos.get_board().get_bb(Color::Black, Piece::Pawn))
                .count() as i32
                    * 5;
            } else if king_pos > Square::E8 {
                res_early += (BitBoard::new(0b11100000 << 48)
                    & pos.get_board().get_bb(Color::Black, Piece::Pawn))
                .count() as i32
                    * 5;
            }
        }
    }

    res_early * phase / OPENING_PHASE
}

pub fn evaluate(pos: &Position) -> Eval {
    let phase = phase_factor(pos);
    let mut res = pos.material_balance() + 20;
    res += evaluate_mobility(pos, phase);
    if res.abs() > 900 {
        return Eval::new(Bound::Exact, Value::Centis(res));
    }

    res += evaluate_pawns(pos, phase, pos.color());
    res -= evaluate_pawns(pos, phase, pos.color().other());

    res += evaluate_queens(pos, phase, pos.color());
    res -= evaluate_queens(pos, phase, pos.color().other());

    res += evaluate_rooks(pos, phase, pos.color());
    res -= evaluate_rooks(pos, phase, pos.color().other());

    res += evaluate_knights(pos, phase, pos.color());
    res -= evaluate_knights(pos, phase, pos.color().other());

    res += evaluate_bishops(pos, phase, pos.color());
    res -= evaluate_bishops(pos, phase, pos.color().other());

    res += evaluate_king_position(pos, phase, pos.color());
    res -= evaluate_king_position(pos, phase, pos.color().other());

    res += evaluate_king_safety(pos, phase, pos.color());
    res -= evaluate_king_safety(pos, phase, pos.color().other());

    //if we are heading into a pawnless endgame, we aim to have a material advantage of five or
    //higher
    let remaining_pawns = (pos.board.get_bb(Color::White, Piece::Pawn)
        | pos.board.get_bb(Color::Black, Piece::Pawn))
    .count();
    //dampen eval quickly for low material difference
    if remaining_pawns < 2 && pos.material_balance().abs() < 5 && pos.board.occupation().count() < 7
    {
        res /= ((5 - pos.material_balance()) * (5 - pos.material_balance())
            / (remaining_pawns as i32 + 1))
            .clamp(1, 25);
    }

    //adjust evaluation to be lower near 50 move rule, since possibility of improvement may be
    //dubious
    if pos.rule_50_count() > 80 {
        res /= (pos.rule_50_count() - 80) as i32;
    }
    Eval::new(Bound::Exact, Value::Centis(res))
}

#[test]
fn evaluate_start_pos() {
    let pos = Position::new();
    let eval = evaluate(&pos);
    println!("Eval {}", eval);
    //everything should be equal up to the tempo
    assert_eq!(eval.value(), Value::Centis(20));
}
