use crate::{
    chess::{
        get_neighbours, get_next_neighbours, BitBoard, Board, Color, File, Move, MoveList,
        MoveType, Piece, Position, Square,
    },
    eval::{Bound, Eval, Value},
    piecetables,
};

#[allow(dead_code)]
pub fn has_pawns(pos: &Position) -> bool {
    !(pos.board.get_bb(Color::White, Piece::Pawn) | pos.board.get_bb(Color::Black, Piece::Pawn))
        .is_empty()
}

pub fn has_minor_pieces(pos: &Position) -> bool {
    !(pos.board.get_bb(Color::White, Piece::Bishop)
        | pos.board.get_bb(Color::Black, Piece::Bishop)
        | pos.board.get_bb(Color::White, Piece::Knight)
        | pos.board.get_bb(Color::Black, Piece::Knight))
    .is_empty()
}

pub fn has_major_pieces(pos: &Position) -> bool {
    !(pos.board.get_bb(Color::White, Piece::Rook)
        | pos.board.get_bb(Color::Black, Piece::Rook)
        | pos.board.get_bb(Color::White, Piece::Queen)
        | pos.board.get_bb(Color::Black, Piece::Queen))
    .is_empty()
}

pub fn is_material_draw(pos: &Position) -> bool {
    pos.board.occupation().count() == 2
        || (pos.board.occupation().count() == 3 && has_minor_pieces(pos))
}

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
        _ => 0,
    }
}

fn pawn_attacks(pos: &Position) -> (BitBoard, BitBoard) {
    let mut pawn_attacks_white = BitBoard::EMPTY;
    let mut pawn_attacks_black = BitBoard::EMPTY;

    for pawn in pos.board.get_bb(Color::White, Piece::Pawn) {
        pawn_attacks_white |= pos.pawn_attacks(pawn, Color::White);
    }
    for pawn in pos.board.get_bb(Color::Black, Piece::Pawn) {
        pawn_attacks_black |= pos.pawn_attacks(pawn, Color::Black);
    }

    match pos.color() {
        Color::White => (pawn_attacks_white, pawn_attacks_black),
        Color::Black => (pawn_attacks_black, pawn_attacks_white),
    }
}

fn evaluate_pawns(pos: &mut Position, phase: i32) -> i32 {
    let mut res_early = 0;
    let mut res_late = 0;
    let mut res = 0;

    let pawns_us = pos.board.get_bb(pos.color(), Piece::Pawn);
    let pawns_them = pos.board.get_bb(pos.color().other(), Piece::Pawn);

    let (guarded_us, guarded_them) = pawn_attacks(pos);

    for pawn in pawns_us {
        let file = BitBoard::from_file(pawn.file());
        let in_front = file & BitBoard::forward_of(pawn, pos.color());
        let mut advance: BitBoard = pawn.into();
        advance = advance.shifted_forward(pos.color());
        let behind = file ^ in_front;

        let neighbours = if pawn.file() == File::H {
            pawns_us & file.shifted_by(-1)
        } else if pawn.file() == File::A {
            pawns_us & file.shifted_by(1)
        } else {
            pawns_us & (file.shifted_by(1) | file.shifted_by(-1))
        };

        //0. Add the base value of the pawn
        res += piece_table_value(Piece::Pawn, pos.color(), pawn, phase);

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
            res_early += match pos.color() {
                Color::White => 50 / (7 - pawn.rank() as i32),
                Color::Black => 50 / pawn.rank() as i32,
            };
            res_late += match pos.color() {
                Color::White => 150 / (7 - pawn.rank() as i32),
                Color::Black => 150 / pawn.rank() as i32,
            };
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

fn evaluate_king_position(pos: &mut Position, phase: i32) -> i32 {
    let king_pos = pos
        .get_board()
        .get_bb(pos.color(), Piece::King)
        .least_square();
    //In the late game stages we want an active king. Maybe want to keep it somewhat central?
    piece_table_value(Piece::King, pos.color(), king_pos, phase)
}

fn evaluate_queens(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //Do not run away with our Queen too fast
    for queen in pos.board.get_bb(pos.color(), Piece::Queen) {
        res += piece_table_value(Piece::Queen, pos.color(), queen, phase);
    }
    res
}

fn evaluate_rooks(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    for rook in pos.board.get_bb(pos.color(), Piece::Rook) {
        res += piece_table_value(Piece::Rook, pos.color(), rook, phase);
        let file = BitBoard::from_file(rook.file());
        //Rooks are good on semi-open and open files
        if (file & pos.board.get_bb(pos.color(), Piece::Pawn)).is_empty() {
            res += 10;
            if (file & pos.board.get_bb(pos.color().other(), Piece::Pawn)).is_empty() {
                res += 30;
            }
        }
        //Doubled Rooks may be good
        //We give half the bonus and double count
        if (file & pos.board.get_bb(pos.color(), Piece::Rook)).count() > 1 {
            res += 5;
        }
    }
    res
}

fn evaluate_bishops(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;

    let bishops = pos.board.get_bb(pos.color(), Piece::Bishop);
    let pawns_us = pos.board.get_bb(pos.color(), Piece::Pawn);

    for bishop in bishops {
        res += piece_table_value(Piece::Bishop, pos.color(), bishop, phase);

        //reduce the value of the bishop, if it is blocked in by pawns
        let mut blocked_score = if Board::WHITE_SQUARES.is_set(bishop) {
            (pawns_us & Board::WHITE_SQUARES).count().saturating_sub(3) * 10
        } else {
            (pawns_us & Board::BLACK_SQUARES).count().saturating_sub(3) * 10
        } as i32;
        //if the bishop is in front of the pawns, the penalty is smaller
        if (BitBoard::forward_of(bishop, pos.color()) & pawns_us).count() < 2 {
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

fn evaluate_knights(pos: &mut Position, phase: i32) -> i32 {
    let mut res: i32 = 0;
    //we like knights in positions with many pawns
    let pawns =
        pos.piece_count(Color::White, Piece::Pawn) + pos.piece_count(Color::Black, Piece::Pawn);

    for knight in pos.board.get_bb(pos.color(), Piece::Knight) {
        res += piece_table_value(Piece::Knight, pos.color(), knight, phase);
        //bonus for closed positions
        res += 25 * pawns / 16;
    }
    res
}
fn evaluate_mobility(moves_us: &MoveList, moves_them: &MoveList, phase: i32) -> i32 {
    let phase_factor = phase as f32 / OPENING_PHASE as f32;
    const KNIGHT_MOVE_VALUES: [f32; 2] = [1., 1.5];
    const BISHOP_MOVE_VALUES: [f32; 2] = [2., 1.];
    const QUEEN_MOVE_VALUES: [f32; 2] = [1., 1.];
    //Early our King will probably want to stay put
    const KING_MOVE_VALUES: [f32; 2] = [0.2, 1.8];
    //Early rooks are hard to develop and it is not an urgent priority
    const ROOK_MOVE_VALUES: [f32; 2] = [0.4, 1.0];
    //We value early pawn movement low as to not discurage moves like e4e5 (halving the pawns
    //potential moves)
    const PAWN_MOVE_VALUES: [f32; 2] = [0.5, 1.2];

    let move_value_us = moves_us
        .iter()
        .map(|m| match m.piece {
            Piece::Pawn => {
                PAWN_MOVE_VALUES[0] * phase_factor + PAWN_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Knight => {
                KNIGHT_MOVE_VALUES[0] * phase_factor + KNIGHT_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Bishop => {
                BISHOP_MOVE_VALUES[0] * phase_factor + BISHOP_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Rook => {
                ROOK_MOVE_VALUES[0] * phase_factor + ROOK_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Queen => {
                QUEEN_MOVE_VALUES[0] * phase_factor + QUEEN_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::King => {
                KING_MOVE_VALUES[0] * phase_factor + KING_MOVE_VALUES[1] * (1. - phase_factor)
            }
            _ => panic!("Invalid Position"),
        })
        .fold(0., |s, mv| s + mv);

    let move_value_them = moves_them
        .iter()
        .map(|m| match m.piece {
            Piece::Pawn => {
                PAWN_MOVE_VALUES[0] * phase_factor + PAWN_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Knight => {
                KNIGHT_MOVE_VALUES[0] * phase_factor + KNIGHT_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Bishop => {
                BISHOP_MOVE_VALUES[0] * phase_factor + BISHOP_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Rook => {
                ROOK_MOVE_VALUES[0] * phase_factor + ROOK_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::Queen => {
                QUEEN_MOVE_VALUES[0] * phase_factor + QUEEN_MOVE_VALUES[1] * (1. - phase_factor)
            }
            Piece::King => {
                KING_MOVE_VALUES[0] * phase_factor + KING_MOVE_VALUES[1] * (1. - phase_factor)
            }
            _ => panic!("Invalid Position"),
        })
        .fold(0., |s, mv| s + mv);

    ((move_value_us / (move_value_us + move_value_them) - 0.5) * 70. * phase_factor) as i32
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

fn phase_factor(pos: &Position) -> i32 {
    let mut phase = 0;
    phase += PAWN_PHASE_WEIGHT * pos.piece_count(Color::White, Piece::Pawn);
    phase += PAWN_PHASE_WEIGHT * pos.piece_count(Color::Black, Piece::Pawn);
    phase += BISHOP_PHASE_WEIGHT * pos.piece_count(Color::White, Piece::Knight);
    phase += BISHOP_PHASE_WEIGHT * pos.piece_count(Color::Black, Piece::Knight);
    phase += KNIGHT_PHASE_WEIGHT * pos.piece_count(Color::White, Piece::Bishop);
    phase += KNIGHT_PHASE_WEIGHT * pos.piece_count(Color::Black, Piece::Bishop);
    phase += ROOK_PHASE_WEIGHT * pos.piece_count(Color::White, Piece::Rook);
    phase += ROOK_PHASE_WEIGHT * pos.piece_count(Color::Black, Piece::Rook);
    phase += QUEEN_PHASE_WEIGHT * pos.piece_count(Color::White, Piece::Queen);
    phase += QUEEN_PHASE_WEIGHT * pos.piece_count(Color::Black, Piece::Queen);
    phase
}

fn attacker_weight(p: Piece) -> i32 {
    match p {
        Piece::Pawn => 1,
        Piece::Bishop | Piece::Knight => 2,
        Piece::Rook => 3,
        Piece::Queen => 5,
        _ => 0,
    }
}

fn evaluate_king_safety(pos: &Position, moves_them: &MoveList, phase: i32) -> i32 {
    //We count attacks near our king. If there are many we penalize the evaluation.
    const DISTANCE_ONE_MULTIPLIER: i32 = 2;
    const DISTANCE_TWO_MULTIPLIER: i32 = 1;
    const SCALE: i32 = 32;
    let mut safety = 0;
    let king_pos = pos.board.get_bb(pos.color(), Piece::King).least_square();
    let king_neighbours = get_neighbours(king_pos);
    let king_next_neighbours = get_next_neighbours(king_pos);
    let mut attackers = BitBoard::EMPTY;
    for m in moves_them {
        if king_neighbours.is_set(m.to) {
            safety -= DISTANCE_ONE_MULTIPLIER * attacker_weight(m.piece);
            attackers |= m.from.into();
        } else if king_next_neighbours.is_set(m.to) {
            safety -= DISTANCE_TWO_MULTIPLIER * attacker_weight(m.piece);
            attackers |= m.from.into();
        }
    }
    let (mut res_early, res_late) = if attackers.count() > 1 {
        (((-safety * safety) / SCALE).clamp(-500, 0), -safety)
    } else {
        (0, 0)
    };

    //In the early game we want pawns to shield our king.
    match pos.color() {
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

    res_early * phase / OPENING_PHASE + res_late * (OPENING_PHASE - phase) / OPENING_PHASE
}

pub fn evaluate(pos: &mut Position) -> Eval {
    let phase = phase_factor(pos);
    let moves_us = pos.get_moves();
    let mut res = pos.material_balance() * 100 + 20;
    pos.switch_color();
    let moves_them = pos.get_moves();
    res += evaluate_mobility(&moves_us, &moves_them, phase);
    pos.switch_color();
    if res.abs() > 900 {
        return Eval::new(Bound::Exact, Value::Centis(res));
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

pub fn order_moves(
    movs: &mut MoveList,
    pos: &Position,
    hash_move: Option<Move>,
    killers: &[Option<Move>; 2],
) {
    movs.sort_unstable_by_key(|m| match m.typ { MoveType::Capture(_) => -pos.see(*m), _ => 200 } - if killers[0].map_or(false, |k| k == *m) || killers[1].map_or(false, |k| k == *m) {0} else {1});
    if let Some(mov) = hash_move {
        movs.sort_by_key(|m| if *m == mov { 0 } else { 1 });
    }
}

#[test]
fn evaluate_start_pos() {
    let mut pos = Position::new();
    let eval = evaluate(&mut pos);
    println!("Eval {}", eval);
    //everything should be equal up to the tempo
    assert!(eval.value() == Value::Centis(20));
}

#[test]
fn bug_hunt() {
    let mut pos = Position::from_fen(String::from(
        "r6r/ppp1nkbp/3B2p1/4p3/1n2P3/2N3PP/qPP2PB1/2KR3R b - - 1 16",
    ))
    .unwrap();
    println!("eval: {}", evaluate(&mut pos));
    let mut pos2 = Position::from_fen(String::from(
        "r6r/pp2nkbp/2nR2p1/4p3/4P3/4Q1PP/1PPKNPB1/q6R b - - 2 17",
    ))
    .unwrap();
    println!("eval: {}", evaluate(&mut pos2));
}
