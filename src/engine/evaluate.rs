mod nnue;
use super::chess;
use super::chess::SquareMethods;
use rand::{Rng, thread_rng};

#[derive(PartialEq,Copy,Clone,Debug)]
enum GameState {
    EARLY,
    MID,
    LATE,
}

fn evaluate_pawns(pos: &mut chess::Position, phase: i32) -> i32 {
        let mut tot = 0.;
        let phase_factor = (OPENING_PHASE-phase) as f64 / OPENING_PHASE as f64;
        for pawn in pos.board[(pos.color(), chess::Piece::PAWN)].iter() {
            let mut value = 1.;
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
            //See if we are close to promoting
            let base_rank = match pos.color() {
                chess::Color::WHITE => 0,
                chess::Color::BLACK => 7,
            };
            let rank = chess::Board::get_rank(pawn) as i32;
            if (rank-base_rank).abs() == 6 {
                value *= 1.+(0.2*phase_factor);
            } else if (rank-base_rank).abs() == 5 {
                value *= 1.+(0.1*phase_factor);
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
            tot += value;
        }
        (tot*100.) as i32
}

fn evaluate_king_position(pos: &mut chess::Position, phase: i32) -> i32 {
    //In the late game stages we want an active king.
    let mut late_eval = -(pos.hard_pins().count_ones() as i32) * 40;
    let neighbours = chess::Board::get_neighbours(pos.get_board()[(pos.color(), chess::Piece::KING)]);
    let free_squares = neighbours ^ (neighbours & pos.get_board()[(pos.color(), chess::Piece::ANY)]);
    late_eval += free_squares.count_ones() as i32 * 5;
    //In early and mid stages we prefer a well-protected king
    let mut early_eval = -(pos.hard_pins().count_ones() as i32) * 20;
    let neighbours = chess::Board::get_neighbours(pos.get_board()[(pos.color(), chess::Piece::KING)]);
    let free_squares = neighbours ^ (neighbours & pos.get_board()[(pos.color(), chess::Piece::ANY)]);
    early_eval -= free_squares.count_ones() as i32 * 5;
    (phase * early_eval + (OPENING_PHASE-phase)*late_eval)/OPENING_PHASE
}

fn evaluate_queens(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::QUEEN) * 900;
    //Do not run away with our Queen too fast
    for queen in pos.board[(pos.color(), chess::Piece::QUEEN)].iter() {
            let base_rank = match pos.color() {
                chess::Color::WHITE => 0,
                chess::Color::BLACK => 7,
            };
            res -= ((chess::Board::get_rank(queen) as i32 - base_rank).abs() * 3 * phase)/OPENING_PHASE;
    }
    res
}
//TODO: stub..
fn evaluate_rooks(pos: &mut chess::Position, _phase: i32) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::ROOK) * 500;
    for rook in pos.board[(pos.color(), chess::Piece::ROOK)].iter() {
        let file = chess::Board::get_file(rook);
        //Rooks are good on semi-open and open files
        if chess::Board::file(file) & pos.board[(pos.color(), chess::Piece::PAWN)] == 0 {
            res += 5;
            if chess::Board::file(file) & pos.board[(pos.color().other(), chess::Piece::PAWN)] == 0 {
                res += 10;
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
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::BISHOP) * 300;
    //Do not advance too fast
    for bishop in pos.board[(pos.color(), chess::Piece::BISHOP)].iter() {
            let best_rank = match pos.color() {
                chess::Color::WHITE => 3,
                chess::Color::BLACK => 4,
            };
            res -= (chess::Board::get_rank(bishop) as i32 - best_rank).abs()*3*phase/OPENING_PHASE;
    }
    res
}
fn evaluate_knights(pos: &mut chess::Position, phase: i32) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::KNIGHT) * 300;
    for knight in pos.board[(pos.color(), chess::Piece::KNIGHT)].iter() {
        //we want to centralize knights
        res += (7-(chess::Board::get_rank(knight) as i32 * 2-7).abs())*2*phase/OPENING_PHASE;
        res += (7-(chess::Board::get_file(knight) as i32 * 2-7).abs())*2*phase/OPENING_PHASE;
    }
    res
}
fn evaluate_mobility(moves_us: &Vec<chess::Move>, moves_them: &Vec<chess::Move>, phase: i32) -> i32 {
    let phase_factor = phase as f32 / OPENING_PHASE as f32;
    const KNIGHT_MOVE_VALUES: [f32;2] = [3.,1.];
    const BISHOP_MOVE_VALUES: [f32;2] = [2.,1.];
    const QUEEN_MOVE_VALUES: [f32;2] = [1.,1.];
    //Early our King will probably want to stay put
    const KING_MOVE_VALUES: [f32;2] = [0.2,1.];
    //Early rooks are hard to develop and it is not an urgent priority
    const ROOK_MOVE_VALUES: [f32;2] = [0.4,1.];
    //We value early pawn movement low as to not discurage moves like e4e5 (halving the pawns
    //potential moves)
    const PAWN_MOVE_VALUES: [f32;2] = [0.5,1.];
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
    return ((move_value_us/(move_value_them+move_value_them)-0.5) * 30.) as i32
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

pub fn evaluate(pos: &mut chess::Position) -> i32 {
    let phase = phase_factor(pos);
    let moves_us = pos.get_moves();
    let mut res = evaluate_pawns(pos, phase);
    res += evaluate_queens(pos, phase);
    res += evaluate_rooks(pos, phase);
    res += evaluate_knights(pos, phase);
    res += evaluate_bishops(pos, phase);
    res += evaluate_king_position(pos, phase);
    pos.switch_color();
    let moves_them = pos.get_moves();
    res -= evaluate_pawns(pos, phase);
    res -= evaluate_queens(pos, phase);
    res -= evaluate_rooks(pos, phase);
    res -= evaluate_knights(pos, phase);
    res -= evaluate_bishops(pos, phase);
    res -= evaluate_king_position(pos, phase);
    pos.switch_color();
    res + evaluate_mobility(&moves_us, &moves_them, phase)
}

pub fn order_moves(movs: &mut Vec<chess::Move>, pos: &chess::Position, hash_move: Option<chess::Move>) {
    movs.sort_unstable_by_key(|m| match m.typ { chess::MoveType::CAPTURE(_) => -pos.see(*m), _ => 500 });
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
    assert!(eval == 0);
}
