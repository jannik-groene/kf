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

fn evaluate_pawns(pos: &mut chess::Position, game_state: GameState) -> i32 {
        let mut tot = 0.;
        for pawn in pos.board[(pos.color(), chess::Piece::PAWN)].iter() {
            let mut value = 1.;
            //Evaluate passers as more as valuable
            let file = chess::Board::get_file(pawn);
            if chess::Board::file(file) & pos.board[(pos.color().other(), chess::Piece::PAWN)] == 0 {
                value *= 1.2;
                //In the late game passers can be critical
                if game_state == GameState::LATE {
                    value*=1.1;
                }
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
            } as i32;
            if game_state != GameState::EARLY {
                let rank = chess::Board::get_rank(pawn) as i32;
                if (rank-base_rank).abs() == 6 {
                    value *= 1.2;
                } else if (rank-base_rank).abs() == 5 {
                    value *= 1.1;
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
            if game_state == GameState::EARLY {
                value += (7. - (chess::Board::get_rank(pawn) as i32 as f64 * 2. - 7.).abs())*(7. - (chess::Board::get_file(pawn) as i32 as f64 * 2. - 7.).abs())/64.;
            }
            tot += value;
        }
        (tot*100.) as i32
}

fn evaluate_king_position(pos: &mut chess::Position, game_state: GameState) -> i32 {
    let mut res: i32 = 0;
    //In the late game stages we want an active king.
    if game_state == GameState::LATE {
        res-=pos.hard_pins().count_ones() as i32 * 20;
        let neighbours = chess::Board::get_neighbours(pos.get_board()[(pos.color(), chess::Piece::KING)]);
        let free_squares = neighbours ^ (neighbours & pos.get_board()[(pos.color(), chess::Piece::ANY)]);
        res+= free_squares.count_ones() as i32 * 5;
    //In early and mid stages we prefer a well-protected king
    } else {
        res-=pos.hard_pins().count_ones() as i32 * 20;
        let neighbours = chess::Board::get_neighbours(pos.get_board()[(pos.color(), chess::Piece::KING)]);
        let free_squares = neighbours ^ (neighbours & pos.get_board()[(pos.color(), chess::Piece::ANY)]);
        res-= free_squares.count_ones() as i32 * 5;
    }
    res
}

fn evaluate_queens(pos: &mut chess::Position, game_state: GameState) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::QUEEN) * 900;
    //Do not run away with our Queen too fast
    for queen in pos.board[(pos.color(), chess::Piece::QUEEN)].iter() {
        if game_state == GameState::EARLY {
            let base_rank = match pos.color() {
                chess::Color::WHITE => 0,
                chess::Color::BLACK => 7,
            };
            res -= (chess::Board::get_rank(queen) as i32 - base_rank).abs() * 3;
        }
    }
    res
}
//TODO: stub..
fn evaluate_rooks(pos: &mut chess::Position, game_state: GameState) -> i32 {
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
fn evaluate_bishops(pos: &mut chess::Position, game_state: GameState) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::BISHOP) * 300;
    //Do not advance too fast
    for bishop in pos.board[(pos.color(), chess::Piece::BISHOP)].iter() {
        if game_state == GameState::EARLY {
            let best_rank = match pos.color() {
                chess::Color::WHITE => 3,
                chess::Color::BLACK => 4,
            };
            res -= (chess::Board::get_rank(bishop) as i32 - best_rank).abs()*3;
        }
    }
    res
}
fn evaluate_knights(pos: &mut chess::Position, game_state: GameState) -> i32 {
    let mut res: i32 = pos.piece_count(pos.color(), chess::Piece::KNIGHT) * 300;
    for knight in pos.board[(pos.color(), chess::Piece::KNIGHT)].iter() {
        //we want to centralize knights
        if game_state != GameState::LATE {
            res += (7-(chess::Board::get_rank(knight) as i32 * 2-7).abs())*2;
            res += (7-(chess::Board::get_file(knight) as i32 * 2-7).abs())*2;
        }
    }
    res
}
fn evaluate_mobility(moves_us: &Vec<chess::Move>, moves_them: &Vec<chess::Move>, game_state: GameState) -> i32 {
    const KNIGHT_MOVE_VALUES: [f32;3] = [3.,2.,1.];
    const BISHOP_MOVE_VALUES: [f32;3] = [2.,3.,1.];
    const QUEEN_MOVE_VALUES: [f32;3] = [1.,2.,1.];
    //Early our King will probably want to stay put
    const KING_MOVE_VALUES: [f32;3] = [0.2,0.5,1.];
    //Early rooks are hard to develop and it is not an urgent priority
    const ROOK_MOVE_VALUES: [f32;3] = [0.4,1.,1.];
    //We value early pawn movement low as to not discurage moves like e4e5 (halving the pawns
    //potential moves)
    const PAWN_MOVE_VALUES: [f32;3] = [0.5,1.,1.];
    let move_value_us = moves_us.iter().map(|m| match m.piece {
                                                chess::Piece::PAWN => PAWN_MOVE_VALUES[game_state as usize],
                                                chess::Piece::KNIGHT => KNIGHT_MOVE_VALUES[game_state as usize],
                                                chess::Piece::BISHOP => BISHOP_MOVE_VALUES[game_state as usize],
                                                chess::Piece::ROOK => ROOK_MOVE_VALUES[game_state as usize],
                                                chess::Piece::QUEEN => QUEEN_MOVE_VALUES[game_state as usize],
                                                chess::Piece::KING => KING_MOVE_VALUES[game_state as usize],
                                                _ => panic!("Invalid Position"),
                                             }).fold(0., |s, mv| s+mv);
    let move_value_them = moves_them.iter().map(|m| match m.piece {
                                                    chess::Piece::PAWN => PAWN_MOVE_VALUES[game_state as usize],
                                                    chess::Piece::KNIGHT => KNIGHT_MOVE_VALUES[game_state as usize],
                                                    chess::Piece::BISHOP => BISHOP_MOVE_VALUES[game_state as usize],
                                                    chess::Piece::ROOK => ROOK_MOVE_VALUES[game_state as usize],
                                                    chess::Piece::QUEEN => QUEEN_MOVE_VALUES[game_state as usize],
                                                    chess::Piece::KING => KING_MOVE_VALUES[game_state as usize],
                                                    _ => panic!("Invalid Position"),
                                                 }).fold(0., |s, mv| s+mv);
    return ((move_value_us/(move_value_them+move_value_them)-0.5) * 30.) as i32
}

pub fn evaluate(pos: &mut chess::Position) -> i32 {
    let game_state = if pos.total_piece_count() > 24 {GameState::EARLY} else if pos.total_piece_count() > 15 {GameState::MID} else {GameState::LATE};
    let moves_us = pos.get_moves();
    let mut res = evaluate_pawns(pos, game_state);
    res += evaluate_queens(pos, game_state);
    res += evaluate_rooks(pos, game_state);
    res += evaluate_knights(pos, game_state);
    res += evaluate_bishops(pos, game_state);
    res += evaluate_king_position(pos, game_state);
    pos.switch_color();
    let moves_them = pos.get_moves();
    res -= evaluate_pawns(pos, game_state);
    res -= evaluate_queens(pos, game_state);
    res -= evaluate_rooks(pos, game_state);
    res -= evaluate_knights(pos, game_state);
    res -= evaluate_bishops(pos, game_state);
    res -= evaluate_king_position(pos, game_state);
    pos.switch_color();
    res + evaluate_mobility(&moves_us, &moves_them, game_state)
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
