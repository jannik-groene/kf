pub mod nnue;
mod piecetables;
use super::chess;
use super::chess::{SquareMethods, SquareIndexMethods};
use rand::{Rng, thread_rng};

pub fn has_pawns(pos: &chess::Position) -> bool {
    pos.board[(chess::Color::WHITE, chess::Piece::PAWN)]
        | pos.board[(chess::Color::BLACK, chess::Piece::PAWN)] != 0
}

pub fn has_minor_pieces(pos: &chess::Position) -> bool {
    pos.board[(chess::Color::WHITE, chess::Piece::BISHOP)]
        | pos.board[(chess::Color::BLACK, chess::Piece::BISHOP)]
        | pos.board[(chess::Color::WHITE, chess::Piece::KNIGHT)]
        | pos.board[(chess::Color::BLACK, chess::Piece::KNIGHT)] != 0
}

pub fn has_major_pieces(pos: &chess::Position) -> bool {
    pos.board[(chess::Color::WHITE, chess::Piece::ROOK)]
        | pos.board[(chess::Color::BLACK, chess::Piece::ROOK)]
        | pos.board[(chess::Color::WHITE, chess::Piece::QUEEN)]
        | pos.board[(chess::Color::BLACK, chess::Piece::QUEEN)] != 0
}

pub fn is_material_draw(pos: &chess::Position) -> bool {
    if pos.board.occupation.count_ones() == 2 {
        true
    } else if pos.board.occupation.count_ones() == 3 && has_minor_pieces(pos) {
        true
    } else {
        false
    }
}

fn piece_table_value(p: chess::Piece, c: chess::Color, s: impl SquareMethods, phase: i32) -> i32 {
    let index = match c {
        chess::Color::WHITE => s.index(),
        chess::Color::BLACK => s.index() ^ 56,
    };
    match p {
        chess::Piece::PAWN => piecetables::PAWN_VALUES[index],
        chess::Piece::KNIGHT => piecetables::KNIGHT_VALUES[index],
        chess::Piece::BISHOP => piecetables::BISHOP_VALUES[index],
        chess::Piece::ROOK => piecetables::ROOK_VALUES[index],
        chess::Piece::QUEEN => piecetables::QUEEN_VALUES[index],
        chess::Piece::KING => (piecetables::KING_EARLY_VALUES[index] * phase + piecetables::KING_LATE_VALUES[index] * (OPENING_PHASE-phase)) / OPENING_PHASE,
        _ => 0
    }
}

fn evaluate_pawns(pos: &mut chess::Position, phase: i32) -> i32 {
        let mut tot = 0.;
        let phase_factor = (OPENING_PHASE-phase) as f64 / OPENING_PHASE as f64;
        for pawn in pos.board[(pos.color(), chess::Piece::PAWN)].iter() {
            let mut value = 1. + piece_table_value(chess::Piece::PAWN, pos.color(), pawn, phase) as f64 / 100.;
            //Evaluate passers as more as valuable
            let file = chess::Board::get_file(pawn);
            if chess::Board::file(file) & pos.board[(pos.color().other(), chess::Piece::PAWN)] == 0 {
                value *= 1.2;
                //In the late game passers can be critical
                value*=1.+(0.5*phase_factor);
                //Protected passers are desirable
                if chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::ROOK)] != 0 ||
                    chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::QUEEN)] != 0 {
                    value *= 1.2;
                }
            }
            //Penalize isolated pawns
            if !pawn.is_at_east_border() &&
                chess::Board::file(chess::Board::get_file(pawn.go_e()))
                    & pos.board[(pos.color(), chess::Piece::PAWN)] == 0 {
                    value *= 0.8;
            }
            if !pawn.is_at_west_border() &&
                chess::Board::file(chess::Board::get_file(pawn.go_w()))
                    & pos.board[(pos.color(), chess::Piece::PAWN)] == 0 {
                    value *= 0.8;
            }
            //Penalize double pawns (or worse)
            let pawns_on_file = (chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::PAWN)]).count_ones();
            value *= 1. - 0.1*(pawns_on_file as f64 - 1.);
            //We want to take the center
            value += (7. - (chess::Board::get_rank(pawn) as i32 as f64 * 2. - 7.).abs())*(7. - (chess::Board::get_file(pawn) as i32 as f64 * 2. - 7.).abs())/64. * (1.-phase_factor);
            tot += value-1.;
        }
        (tot*100.) as i32
}

fn evaluate_king_position(pos: &mut chess::Position, phase: i32) -> i32 {
    let king_pos = pos.get_board()[(pos.color(), chess::Piece::KING)];
    //In the late game stages we want an active king. Maybe want to keep it somewhat central?
    piece_table_value(chess::Piece::KING, pos.color(), king_pos, phase)
}

fn evaluate_queens(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //Do not run away with our Queen too fast
    for queen in pos.board[(pos.color(), chess::Piece::QUEEN)].iter() {
        res += piece_table_value(chess::Piece::QUEEN, pos.color(), queen, phase);
    }
    res
}

fn evaluate_rooks(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    for rook in pos.board[(pos.color(), chess::Piece::ROOK)].iter() {
        res += piece_table_value(chess::Piece::ROOK, pos.color(), rook, phase);
        let file = chess::Board::get_file(rook);
        //Rooks are good on semi-open and open files
        if chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::PAWN)] == 0 {
            res += 10;
            if chess::Board::file(file) & pos.board[(pos.color().other(), chess::Piece::PAWN)] == 0 {
                res += 30;
            }
        }
        //Doubled Rooks may be good
        //We give half the bonus and double count
        if (chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::ROOK)]).count_ones() > 1 {
            res += 5;
        }
    }
    res
}

fn evaluate_bishops(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //Do not advance too fast
    for bishop in pos.board[(pos.color(), chess::Piece::BISHOP)].iter() {
            res += piece_table_value(chess::Piece::BISHOP, pos.color(), bishop, phase);
    }
    res
}

fn evaluate_knights(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    for knight in pos.board[(pos.color(), chess::Piece::KNIGHT)].iter() {
        res += piece_table_value(chess::Piece::KNIGHT, pos.color(), knight, phase);
    }
    res
}
fn evaluate_mobility(moves_us: &Vec<chess::Move>, moves_them: &Vec<chess::Move>, phase: i32) -> i32 {
    let phase_factor = phase as f32 / OPENING_PHASE as f32;
    const KNIGHT_MOVE_VALUES: [f32;2] = [2.,0.8];
    const BISHOP_MOVE_VALUES: [f32;2] = [2.,0.8];
    const QUEEN_MOVE_VALUES: [f32;2] = [1.,1.];
    //Early our King will probably want to stay put
    const KING_MOVE_VALUES: [f32;2] = [0.2,1.2];
    //Early rooks are hard to develop and it is not an urgent priority
    const ROOK_MOVE_VALUES: [f32;2] = [0.4,1.2];
    //We value early pawn movement low as to not discurage moves like e4e5 (halving the pawns
    //potential moves)
    const PAWN_MOVE_VALUES: [f32;2] = [0.5,1.2];

    let move_value_us = moves_us.iter().map(|m| match m.piece {
                                                chess::Piece::PAWN => PAWN_MOVE_VALUES[0]*phase_factor + PAWN_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::KNIGHT => KNIGHT_MOVE_VALUES[0]*phase_factor + KNIGHT_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::BISHOP => BISHOP_MOVE_VALUES[0]*phase_factor + BISHOP_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::ROOK => ROOK_MOVE_VALUES[0]*phase_factor + ROOK_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::QUEEN => QUEEN_MOVE_VALUES[0]*phase_factor + QUEEN_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::KING => KING_MOVE_VALUES[0]*phase_factor + KING_MOVE_VALUES[1]*(1.-phase_factor),
                                                _ => panic!("Invalid Position"),
                                             }).fold(0., |s, mv| s+mv);

    let move_value_them = moves_them.iter().map(|m| match m.piece {
                                                chess::Piece::PAWN => PAWN_MOVE_VALUES[0]*phase_factor + PAWN_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::KNIGHT => KNIGHT_MOVE_VALUES[0]*phase_factor + KNIGHT_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::BISHOP => BISHOP_MOVE_VALUES[0]*phase_factor + BISHOP_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::ROOK => ROOK_MOVE_VALUES[0]*phase_factor + ROOK_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::QUEEN => QUEEN_MOVE_VALUES[0]*phase_factor + QUEEN_MOVE_VALUES[1]*(1.-phase_factor),
                                                chess::Piece::KING => KING_MOVE_VALUES[0]*phase_factor + KING_MOVE_VALUES[1]*(1.-phase_factor),
                                                _ => panic!("Invalid Position"),
                                             }).fold(0., |s, mv| s+mv);

    ((move_value_us/(move_value_us+move_value_them)-0.5) * 70. * phase_factor) as i32
}

//use piece values as first approximation to phase
const PAWN_PHASE_WEIGHT: i32 = 1;
const BISHOP_PHASE_WEIGHT: i32 = 3;
const KNIGHT_PHASE_WEIGHT: i32 = 3;
const ROOK_PHASE_WEIGHT: i32 = 5;
const QUEEN_PHASE_WEIGHT: i32 = 9;
//In the starting position, this yields a phase factor of 78
const OPENING_PHASE: i32 = 16*PAWN_PHASE_WEIGHT + 4*BISHOP_PHASE_WEIGHT + 4*KNIGHT_PHASE_WEIGHT + 4*ROOK_PHASE_WEIGHT + 2*QUEEN_PHASE_WEIGHT;

fn phase_factor(pos: &chess::Position) -> i32 {
    let mut phase = 0;
    phase += PAWN_PHASE_WEIGHT*pos.piece_count(chess::Color::WHITE, chess::Piece::PAWN);
    phase += PAWN_PHASE_WEIGHT*pos.piece_count(chess::Color::BLACK, chess::Piece::PAWN);
    phase += BISHOP_PHASE_WEIGHT*pos.piece_count(chess::Color::WHITE, chess::Piece::KNIGHT);
    phase += BISHOP_PHASE_WEIGHT*pos.piece_count(chess::Color::BLACK, chess::Piece::KNIGHT);
    phase += KNIGHT_PHASE_WEIGHT*pos.piece_count(chess::Color::WHITE, chess::Piece::BISHOP);
    phase += KNIGHT_PHASE_WEIGHT*pos.piece_count(chess::Color::BLACK, chess::Piece::BISHOP);
    phase += ROOK_PHASE_WEIGHT*pos.piece_count(chess::Color::WHITE, chess::Piece::ROOK);
    phase += ROOK_PHASE_WEIGHT*pos.piece_count(chess::Color::BLACK, chess::Piece::ROOK);
    phase += QUEEN_PHASE_WEIGHT*pos.piece_count(chess::Color::WHITE, chess::Piece::QUEEN);
    phase += QUEEN_PHASE_WEIGHT*pos.piece_count(chess::Color::BLACK, chess::Piece::QUEEN);
    phase
}

fn evaluate_king_safety(pos: &chess::Position, moves_them: &Vec<chess::Move>, phase: i32) -> i32 {
    //We count attacks near our king. If there are many we penalize the evaluation.
    const DISTANCE_ONE_MULTIPLIER: i32 = 2;
    //const DISTANCE_TWO_MULTIPLIER: i32 = 1;
    const SCALE: i32 = 50;
    let mut safety = 0;
    let king_pos = pos.board[(pos.color(), chess::Piece::KING)];
    let king_neighbours = chess::Board::get_neighbours(king_pos);
    for m in moves_them {
        if m.to.square() & king_neighbours != 0 {
            safety -= DISTANCE_ONE_MULTIPLIER * (m.piece.value() / 100);
        }
    }
    let mut res = (400 - safety * safety).clamp(-3000,0) / SCALE;

    //In the early game we want pawns to shield our king.
    match pos.color() {
        chess::Color::WHITE => {
            if king_pos.index() < 2 {
                res += ((0b111 << 8) &
                        pos.get_board()[(chess::Color::WHITE, chess::Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            } else if king_pos.index() < 8 && king_pos.index() > 4 {
                res += ((0b11100000 << 8) &
                        pos.get_board()[(chess::Color::WHITE, chess::Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            }
        },
        chess::Color::BLACK => {
            if king_pos.index() > 47 && king_pos.index() < 51 {
                res += ((0b111 << 48) &
                        pos.get_board()[(chess::Color::BLACK, chess::Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            } else if king_pos.index() > 61 {
                res += ((0b11100000 << 48) &
                        pos.get_board()[(chess::Color::BLACK, chess::Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            }
        }
    }

    res
}

pub fn evaluate(pos: &mut chess::Position) -> i32 {
    let phase = phase_factor(pos);
    let moves_us = pos.get_moves();
    let mut res = pos.material_balance()*100 + 20;
    pos.switch_color();
    let moves_them = pos.get_moves();
    res += evaluate_mobility(&moves_us, &moves_them, phase);
    pos.switch_color();
    if res.abs() > 900 {
        return res;
    }
    res += evaluate_pawns(pos, phase);
    res += evaluate_queens(pos, phase);
    res += evaluate_rooks(pos, phase);
    res += evaluate_knights(pos, phase);
    res += evaluate_bishops(pos, phase);
    res += evaluate_king_position(pos, phase);
    res += evaluate_king_safety(pos, &moves_them, phase);
    pos.switch_color();
    res -= evaluate_pawns(pos, phase);
    res -= evaluate_queens(pos, phase);
    res -= evaluate_rooks(pos, phase);
    res -= evaluate_knights(pos, phase);
    res -= evaluate_bishops(pos, phase);
    res -= evaluate_king_position(pos, phase);
    res -= evaluate_king_safety(pos, &moves_us, phase);
    pos.switch_color();

    //if we are heading into a pawnless endgame, we aim to have a material advantage of five or
    //higher
    //let remaining_pawns = (pos.board[(chess::Color::WHITE, chess::Piece::PAWN)]
    //                        | pos.board[(chess::Color::BLACK, chess::Piece::PAWN)]).count_ones();
    //dampen eval quickly for material difference
    //if remaining_pawns == 0 && pos.material_balance() < 6 {
    //    res /= (6-pos.material_balance())*(6-pos.material_balance());
    //}

    //adjust evaluation to be lower near 50 move rule, since possibility of improvement may be
    //dubious
    if pos.rule_50_count() > 80 {
        res / (pos.rule_50_count() - 80) as i32
    } else {
        res
    }
}

pub fn order_moves(movs: &mut Vec<chess::Move>, pos: &chess::Position,
                   hash_move: Option<chess::Move>, killers: &[Option<chess::Move>; 2]) {
    movs.sort_unstable_by_key(|m| match m.typ { chess::MoveType::CAPTURE(_) => -pos.see(*m), _ => 200 } - if killers[0].map_or(false, |k| k == *m) || killers[1].map_or(false, |k| k == *m) {0} else {1});
    if hash_move.is_some() {
        movs.sort_by_key(|m| if *m == hash_move.unwrap() {0} else {1});
    }
}

pub fn order_moves_with_random_bias(movs: &mut Vec<chess::Move>, pos: &chess::Position, hash_move: Option<chess::Move>) {
    movs.sort_unstable_by_key(|m| match m.typ { chess::MoveType::CAPTURE(_) => -pos.see(*m), _ => 0 } + thread_rng().gen_range(-60..=60));
    if hash_move.is_some() {
        movs.sort_by_key(|m| if *m == hash_move.unwrap() {0} else {1});
    }
}

#[test]
fn evaluate_start_pos() {
    let mut pos = chess::Position::new();
    let eval = evaluate(&mut pos);
    println!("Eval {}", eval);
    //everything should be equal up to the tempo
    assert!(eval == 20);
}
