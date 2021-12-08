pub mod nnue;
pub mod eval;
mod piecetables;
use super::chess;
use super::chess::{Position, Move, MoveType, Board, Piece, Color, SquareMethods, SquareIndexMethods};
use rand::{Rng, thread_rng};

use eval::{Eval, Bound, Value};

pub fn has_pawns(pos: &Position) -> bool {
    pos.board[(Color::WHITE, Piece::PAWN)]
        | pos.board[(Color::BLACK, Piece::PAWN)] != 0
}

pub fn has_minor_pieces(pos: &Position) -> bool {
    pos.board[(Color::WHITE, Piece::BISHOP)]
        | pos.board[(Color::BLACK, Piece::BISHOP)]
        | pos.board[(Color::WHITE, Piece::KNIGHT)]
        | pos.board[(Color::BLACK, Piece::KNIGHT)] != 0
}

pub fn has_major_pieces(pos: &Position) -> bool {
    pos.board[(Color::WHITE, Piece::ROOK)]
        | pos.board[(Color::BLACK, Piece::ROOK)]
        | pos.board[(Color::WHITE, Piece::QUEEN)]
        | pos.board[(Color::BLACK, Piece::QUEEN)] != 0
}

pub fn is_material_draw(pos: &Position) -> bool {
    if pos.board.occupation.count_ones() == 2 {
        true
    } else if pos.board.occupation.count_ones() == 3 && has_minor_pieces(pos) {
        true
    } else {
        false
    }
}

fn piece_table_value(p: Piece, c: Color, s: impl SquareMethods, phase: i32) -> i32 {
    let index = match c {
        Color::WHITE => s.index(),
        Color::BLACK => s.index() ^ 56,
    };
    match p {
        Piece::PAWN => piecetables::PAWN_VALUES[index],
        Piece::KNIGHT => piecetables::KNIGHT_VALUES[index],
        Piece::BISHOP => piecetables::BISHOP_VALUES[index],
        Piece::ROOK => piecetables::ROOK_VALUES[index],
        Piece::QUEEN => piecetables::QUEEN_VALUES[index],
        Piece::KING => (piecetables::KING_EARLY_VALUES[index] * phase + piecetables::KING_LATE_VALUES[index] * (OPENING_PHASE-phase)) / OPENING_PHASE,
        _ => 0
    }
}

fn pawn_attacks(pos: &Position) -> (u64,u64) {
    let mut pawn_attacks_white = 0;
    let mut pawn_attacks_black = 0;

    for pawn in pos.board[(Color::WHITE, Piece::PAWN)].iter() {
        if !pawn.is_at_west_border() {
            pawn_attacks_white |= pawn.go_nw();
        }
        if !pawn.is_at_east_border() {
            pawn_attacks_white |= pawn.go_ne()
        }
    }

    for pawn in pos.board[(Color::BLACK, Piece::PAWN)].iter() {
        if !pawn.is_at_west_border() {
            pawn_attacks_black |= pawn.go_sw();
        }
        if !pawn.is_at_east_border() {
            pawn_attacks_black |= pawn.go_se()
        }
    }

    match pos.color() {
        Color::WHITE => (pawn_attacks_white, pawn_attacks_black),
        Color::BLACK=> (pawn_attacks_black, pawn_attacks_white),
    }
}

fn evaluate_pawns(pos: &mut Position, phase: i32) -> i32 {
    let mut res_early = 0;
    let mut res_late = 0;
    let mut res = 0;

    let pawns_us = pos.board[(pos.color(), Piece::PAWN)];
    let pawns_them = pos.board[(pos.color().other(), Piece::PAWN)];

    let (guarded_us, guarded_them) = pawn_attacks(pos);

    for pawn in pawns_us.iter() {

        let file = Board::file(Board::get_file(pawn));
        let in_front = match pos.color() {
            Color::WHITE => file << ((pawn.index() / 8) * 8 + 8),
            Color::BLACK => file >> ((8 - (pawn.index() / 8)) * 8),
        };
        let advance = match pos.color() {
            Color::WHITE => pawn.go_n(),
            Color::BLACK => pawn.go_s(),
        };
        let behind = file ^ in_front;

        //0. Add the base value of the pawn
        res += piece_table_value(Piece::PAWN, pos.color(), pawn, phase);

        //1. Check if the pawn is doubled (or worse).
        let pawns_us_on_file = file & pawns_us;
        res_early -= (pawns_us_on_file.count_ones() as i32 - 1) * 10;
        res_late  -= (pawns_us_on_file.count_ones() as i32 - 1) * 30;

        //2. Check if the pawn is isolated
        if pawn.is_at_east_border() {
            if pawns_us & Board::file(chess::File::B) == 0 {
                res -= 20;
            }
        } else if pawn.is_at_west_border() {
            if pawns_us & Board::file(chess::File::G) == 0 {
                res -= 20;
            }
        } else {
            if (pawns_us & Board::file(Board::get_file(pawn.go_e())))
                | (pawns_us & Board::file(Board::get_file(pawn.go_w()))) == 0 {
                res -= 30;
            }
        }

        //3. Check for backward pawn
        if in_front & guarded_them != 0 && (behind | advance) & guarded_us == 0 {
            res -= 20;
        }

        //4. Check if we have a passer
        if in_front & (pawns_them | guarded_them | pawns_us) == 0 {
            res_early += match pos.color() {
                Color::WHITE => 50 / (7 - pawn.index() as i32 / 8),
                Color::BLACK => 50 / (pawn.index() as i32 / 8)
            };
            res_late += match pos.color() {
                Color::WHITE => 100 / (7 - pawn.index() as i32 / 8),
                Color::BLACK => 100 / (pawn.index() as i32 / 8)
            };
        }

        //5. Check for candidate passers
        let supporters = (if !pawn.is_at_west_border() { behind.go_w() } else {0}
                            | if !pawn.is_at_east_border() { behind.go_e() } else {0})
                         & pawns_us;

        let sentries   = (if !pawn.is_at_west_border() { in_front.go_w() } else {0}
                            | if !pawn.is_at_east_border() { in_front.go_e() } else {0})
                         & pawns_them;

        if in_front & (pawns_them | pawns_us) == 0 && supporters.count_ones() > sentries.count_ones() {
            res_early += 15; res_late += 25;
        }

        //TODO: hidden passers?
    }

    res_late * (OPENING_PHASE - phase) / OPENING_PHASE + res_early * phase / OPENING_PHASE + res
}

fn evaluate_king_position(pos: &mut Position, phase: i32) -> i32 {
    let king_pos = pos.get_board()[(pos.color(), Piece::KING)];
    //In the late game stages we want an active king. Maybe want to keep it somewhat central?
    piece_table_value(Piece::KING, pos.color(), king_pos, phase)
}

fn evaluate_queens(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //Do not run away with our Queen too fast
    for queen in pos.board[(pos.color(), Piece::QUEEN)].iter() {
        res += piece_table_value(Piece::QUEEN, pos.color(), queen, phase);
    }
    res
}

fn evaluate_rooks(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    for rook in pos.board[(pos.color(), Piece::ROOK)].iter() {
        res += piece_table_value(Piece::ROOK, pos.color(), rook, phase);
        let file = Board::get_file(rook);
        //Rooks are good on semi-open and open files
        if Board::file(file) & pos.board[(pos.color(), Piece::PAWN)] == 0 {
            res += 10;
            if Board::file(file) & pos.board[(pos.color().other(), Piece::PAWN)] == 0 {
                res += 30;
            }
        }
        //Doubled Rooks may be good
        //We give half the bonus and double count
        if (Board::file(file) & pos.board[(pos.color(), Piece::ROOK)]).count_ones() > 1 {
            res += 5;
        }
    }
    res
}

fn evaluate_bishops(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //Do not advance too fast
    for bishop in pos.board[(pos.color(), Piece::BISHOP)].iter() {
            res += piece_table_value(Piece::BISHOP, pos.color(), bishop, phase);
    }
    //give a bonus for the bishop pair
    //TODO: check that bishops are actually of opposite color.
    if pos.piece_count(pos.color(), Piece::BISHOP) >= 2 {
        res += 50;
    }
    res
}

fn evaluate_knights(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //we like knights in positions with many pawns
    let pawns = pos.piece_count(Color::WHITE, Piece::PAWN) + pos.piece_count(Color::BLACK, Piece::PAWN);

    for knight in pos.board[(pos.color(), Piece::KNIGHT)].iter() {
        res += piece_table_value(Piece::KNIGHT, pos.color(), knight, phase);
        //bonus for closed positions
        res += 25 * pawns / 16;
    }
    res
}
fn evaluate_mobility(moves_us: &Vec<Move>, moves_them: &Vec<Move>, phase: i32) -> i32 {
    let phase_factor = phase as f32 / OPENING_PHASE as f32;
    const KNIGHT_MOVE_VALUES: [f32;2] = [1.,1.5];
    const BISHOP_MOVE_VALUES: [f32;2] = [2.,1.];
    const QUEEN_MOVE_VALUES: [f32;2] = [1.,1.];
    //Early our King will probably want to stay put
    const KING_MOVE_VALUES: [f32;2] = [0.2,1.8];
    //Early rooks are hard to develop and it is not an urgent priority
    const ROOK_MOVE_VALUES: [f32;2] = [0.4,1.0];
    //We value early pawn movement low as to not discurage moves like e4e5 (halving the pawns
    //potential moves)
    const PAWN_MOVE_VALUES: [f32;2] = [0.5,1.2];

    let move_value_us = moves_us.iter().map(|m| match m.piece {
                                                Piece::PAWN => PAWN_MOVE_VALUES[0]*phase_factor + PAWN_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::KNIGHT => KNIGHT_MOVE_VALUES[0]*phase_factor + KNIGHT_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::BISHOP => BISHOP_MOVE_VALUES[0]*phase_factor + BISHOP_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::ROOK => ROOK_MOVE_VALUES[0]*phase_factor + ROOK_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::QUEEN => QUEEN_MOVE_VALUES[0]*phase_factor + QUEEN_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::KING => KING_MOVE_VALUES[0]*phase_factor + KING_MOVE_VALUES[1]*(1.-phase_factor),
                                                _ => panic!("Invalid Position"),
                                             }).fold(0., |s, mv| s+mv);

    let move_value_them = moves_them.iter().map(|m| match m.piece {
                                                Piece::PAWN => PAWN_MOVE_VALUES[0]*phase_factor + PAWN_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::KNIGHT => KNIGHT_MOVE_VALUES[0]*phase_factor + KNIGHT_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::BISHOP => BISHOP_MOVE_VALUES[0]*phase_factor + BISHOP_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::ROOK => ROOK_MOVE_VALUES[0]*phase_factor + ROOK_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::QUEEN => QUEEN_MOVE_VALUES[0]*phase_factor + QUEEN_MOVE_VALUES[1]*(1.-phase_factor),
                                                Piece::KING => KING_MOVE_VALUES[0]*phase_factor + KING_MOVE_VALUES[1]*(1.-phase_factor),
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

fn phase_factor(pos: &Position) -> i32 {
    let mut phase = 0;
    phase += PAWN_PHASE_WEIGHT*pos.piece_count(Color::WHITE, Piece::PAWN);
    phase += PAWN_PHASE_WEIGHT*pos.piece_count(Color::BLACK, Piece::PAWN);
    phase += BISHOP_PHASE_WEIGHT*pos.piece_count(Color::WHITE, Piece::KNIGHT);
    phase += BISHOP_PHASE_WEIGHT*pos.piece_count(Color::BLACK, Piece::KNIGHT);
    phase += KNIGHT_PHASE_WEIGHT*pos.piece_count(Color::WHITE, Piece::BISHOP);
    phase += KNIGHT_PHASE_WEIGHT*pos.piece_count(Color::BLACK, Piece::BISHOP);
    phase += ROOK_PHASE_WEIGHT*pos.piece_count(Color::WHITE, Piece::ROOK);
    phase += ROOK_PHASE_WEIGHT*pos.piece_count(Color::BLACK, Piece::ROOK);
    phase += QUEEN_PHASE_WEIGHT*pos.piece_count(Color::WHITE, Piece::QUEEN);
    phase += QUEEN_PHASE_WEIGHT*pos.piece_count(Color::BLACK, Piece::QUEEN);
    phase
}

fn evaluate_king_safety(pos: &Position, moves_them: &Vec<Move>, phase: i32) -> i32 {
    //We count attacks near our king. If there are many we penalize the evaluation.
    const DISTANCE_ONE_MULTIPLIER: i32 = 2;
    const DISTANCE_TWO_MULTIPLIER: i32 = 1;
    const SCALE: i32 = 32;
    let mut safety = 0;
    let king_pos = pos.board[(pos.color(), Piece::KING)];
    let king_neighbours = Board::get_neighbours(king_pos);
    let king_next_neighbours = Board::get_neighbours(king_pos);
    for m in moves_them {
        if m.to.square() & king_neighbours != 0 {
            safety -= DISTANCE_ONE_MULTIPLIER * (m.piece.value() / 100);
        } else if m.to.square() & king_next_neighbours != 0 {
            safety -= DISTANCE_TWO_MULTIPLIER * (m.piece.value() / 100);
        }
    }
    let mut res = (500 - safety * safety).clamp(-15000,0) / SCALE;

    //In the early game we want pawns to shield our king.
    match pos.color() {
        Color::WHITE => {
            if king_pos.index() < 2 {
                res += ((0b111 << 8) &
                        pos.get_board()[(Color::WHITE, Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            } else if king_pos.index() < 8 && king_pos.index() > 4 {
                res += ((0b11100000 << 8) &
                        pos.get_board()[(Color::WHITE, Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            }
        },
        Color::BLACK => {
            if king_pos.index() > 47 && king_pos.index() < 51 {
                res += ((0b111 << 48) &
                        pos.get_board()[(Color::BLACK, Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            } else if king_pos.index() > 61 {
                res += ((0b11100000 << 48) &
                        pos.get_board()[(Color::BLACK, Piece::PAWN)]).count_ones() as i32
                    * 5 * phase / OPENING_PHASE;
            }
        }
    }

    res
}

pub fn evaluate(pos: &mut Position) -> Eval {
    let phase = phase_factor(pos);
    let moves_us = pos.get_moves();
    let mut res = pos.material_balance()*100 + 20;
    pos.switch_color();
    let moves_them = pos.get_moves();
    res += evaluate_mobility(&moves_us, &moves_them, phase);
    pos.switch_color();
    if res.abs() > 900 {
        return Eval::new(Bound::EXACT, Value::CENTIS(res));
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
    let remaining_pawns = (pos.board[(Color::WHITE, Piece::PAWN)]
                            | pos.board[(Color::BLACK, Piece::PAWN)]).count_ones();
    //dampen eval quickly for low material difference
    if remaining_pawns < 2 && pos.material_balance().abs() < 5 && pos.board.occupation.count_ones() < 7 {
        res /= ((5-pos.material_balance())*(5-pos.material_balance())/(remaining_pawns as i32 + 1)).clamp(1,25);
    }

    //adjust evaluation to be lower near 50 move rule, since possibility of improvement may be
    //dubious
    if pos.rule_50_count() > 80 {
        res /= (pos.rule_50_count() - 80) as i32;
    }
    Eval::new(Bound::EXACT, Value::CENTIS(res))
}

pub fn order_moves(movs: &mut Vec<Move>, pos: &Position,
                   hash_move: Option<Move>, killers: &[Option<Move>; 2]) {
    movs.sort_unstable_by_key(|m| match m.typ { MoveType::CAPTURE(_) => -pos.see(*m), _ => 200 } - if killers[0].map_or(false, |k| k == *m) || killers[1].map_or(false, |k| k == *m) {0} else {1});
    if hash_move.is_some() {
        movs.sort_by_key(|m| if *m == hash_move.unwrap() {0} else {1});
    }
}

pub fn order_moves_with_random_bias(movs: &mut Vec<Move>, pos: &Position, hash_move: Option<Move>) {
    movs.sort_unstable_by_key(|m| match m.typ { MoveType::CAPTURE(_) => -pos.see(*m), _ => 0 } + thread_rng().gen_range(-60..=60));
    if hash_move.is_some() {
        movs.sort_by_key(|m| if *m == hash_move.unwrap() {0} else {1});
    }
}

#[test]
fn evaluate_start_pos() {
    let mut pos = Position::new();
    let eval = evaluate(&mut pos);
    println!("Eval {}", eval);
    //everything should be equal up to the tempo
    assert!(eval.value() == Value::CENTIS(20));
}

#[test]
fn bug_hunt() {
    let mut pos = Position::from_fen(String::from("r6r/ppp1nkbp/3B2p1/4p3/1n2P3/2N3PP/qPP2PB1/2KR3R b - - 1 16")).unwrap();
    println!("eval: {}", evaluate(&mut pos));
    let mut pos2 = Position::from_fen(String::from("r6r/pp2nkbp/2nR2p1/4p3/4P3/4Q1PP/1PPKNPB1/q6R b - - 2 17")).unwrap();
    println!("eval: {}", evaluate(&mut pos2));
    panic!()
}
