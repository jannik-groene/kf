use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use super::chess;
use super::chess::{SquareMethods, SquareIndexMethods};

#[derive(Clone)]
pub struct NNUEState {
    //state of the first hidden layer
    state: [[i16; 256]; 2],
    // We use 64 King Position * 64 Piece Positions * 11 Piece Types
    // It is indexed as w1[king position][piece type][piece position]
    w1: Vec<[[[i16; 256]; 64]; 11]>,
    w2: Vec<[[i8; 256]; 2]>,
    w3: Vec<[i8; 32]>,
    w4: Vec<i8>,
    b1: Vec<i16>,
    b2: Vec<i8>,
    b3: Vec<i8>,
    b4: i8,
}

#[inline(always)]
fn calculate_piece_index(p: chess::Piece, c: chess:: Color, us: chess::Color) -> usize {
    if c == us {
        p as usize + 5
    } else {
        p as usize
    }
}

impl NNUEState {
    const WEIGHT_SCALE: i32 = 64;
    const INPUT_SCALE: i32 = 127;
    pub fn from_weights(weight_path: &Path) -> NNUEState {
        let file = File::open(weight_path).unwrap();
        let mut buf = BufReader::new(file);
        let mut nnue = NNUEState {
            state: [[0; 256]; 2],
            w1: vec![[[[0; 256]; 64]; 11]; 64],
            w2: vec![[[0; 256]; 2]; 32],
            w3: vec![[0; 32]; 32],
            w4: vec![0; 32],
            b1: vec![0; 256],
            b2: vec![0; 32],
            b3: vec![0; 32],
            b4: 0,
        };
        let mut entry = [0u8; 4];
        //read feature transformer weights
        for kpos in 0..64 {
            for ptype in 0..11 {
                for ppos in 0..64 {
                    for i in 0..256 {
                        buf.read(&mut entry).unwrap();
                        nnue.w1[kpos][ptype][ppos][i] = (f32::from_ne_bytes(entry) * Self::INPUT_SCALE as f32) as i16;
                    }
                }
            }
        }
        //read feature transformer biases
        for i in 0..256 {
            buf.read(&mut entry).unwrap();
            nnue.b1[i] = (f32::from_ne_bytes(entry) * Self::INPUT_SCALE as f32) as i16;
        }
        //read first linear layer and its biases
        for i in 0..32 {
            for j in 0..512 {
                buf.read(&mut entry).unwrap();
                nnue.w2[i][j/256][j%256] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
            }
        }
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            nnue.b2[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        //read second linear layer
        for i in 0..32 {
            for j in 0..32 {
                buf.read(&mut entry).unwrap();
                nnue.w3[i][j] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
            }
        }
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            nnue.b3[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        //read last layer
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            nnue.w4[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        buf.read(&mut entry).unwrap();
        nnue.b4 = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        nnue
    }
    //Initialize the input layer state. A certain king position may be specified to be used instead
    //of the one on the board (used when updating the net state after a king move)
    fn initialize_color_state(&mut self, pos: &chess::Position, c: chess::Color, king_pos: Option<chess::Square>) {
        //Used to flip the board into the white player perspective
        let flip = match c {chess::Color::WHITE => 0, chess::Color::BLACK => 56};
        let kp = match king_pos {
            Some(p) => p,
            None => pos.get_board()[(c, chess::Piece::KING)],
        }.index() ^ flip;
        //Apply the bias
        self.state[c as usize].iter_mut().zip(self.b1.iter()).for_each(|(x,y)| *x = *y);
        //We go through each relevant piece
        //for (x,y) in self.w1[kp][0][kp].iter().zip(self.state[c as usize].iter_mut()) {
        //    *y += x;
        //}
        let okp = pos.get_board()[(c.other(), chess::Piece::KING)].index() ^ flip;
        for (x,y) in self.w1[kp][0][okp].iter().zip(self.state[c as usize].iter_mut()) {
            *y += *x;
        }
        for oq in pos.get_board()[(c.other(), chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[kp][1][oq.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for ob in pos.get_board()[(c.other(), chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[kp][2][ob.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for on in pos.get_board()[(c.other(), chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[kp][3][on.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for or in pos.get_board()[(c.other(), chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[kp][4][or.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for op in pos.get_board()[(c.other(), chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[kp][5][op.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for q in pos.get_board()[(c, chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[kp][6][q.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for b in pos.get_board()[(c, chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[kp][7][b.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for n in pos.get_board()[(c, chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[kp][8][n.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for r in pos.get_board()[(c, chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[kp][9][r.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for p in pos.get_board()[(c, chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[kp][10][p.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
    }
    //Input is the position _BEFORE_ the move is made.
    pub fn update_color_state(&mut self, pos: &chess::Position, m: chess::Move, c: chess::Color, undo: bool) {
        let sign = if undo {-1} else {1};
        //We flip the bord to out perspective by ^-ing with flip
        let flip = match c {chess::Color::WHITE => 0, chess::Color::BLACK => 56};
        //if the King has not moved the update is simple
        if m.piece != chess::Piece::KING
                || pos.get_board()[(c.other(), chess::Piece::KING)] == m.from.square() {

            let kp = pos.get_board()[(c, chess::Piece::KING)].index() ^ flip;
            let f = m.from.index() ^ flip;
            let t = m.to.index() ^ flip;

            let piece_color = if pos.get_board()[(c, chess::Piece::ANY)] & m.from.square() != 0 {c} else {c.other()};

            let piece_index = calculate_piece_index(m.piece, piece_color, c);

            //Update loop
            //Consider faster crate for simd?
            match m.typ {
                chess::MoveType::CAPTURE(p) => {
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                    let capture_index = calculate_piece_index(p, piece_color.other(), c);
                    for (x,y) in self.w1[kp][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= *x * sign;
                    }
                },
                chess::MoveType::PROMOTION(p) => {
                    let promotion_index = calculate_piece_index(p, piece_color, c);
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][promotion_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                },
                chess::MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => {
                    let promotion_index = calculate_piece_index(p_prom, piece_color, c);
                    let capture_index = calculate_piece_index(p_cap, piece_color.other(), c);
                    //Move and promote piece
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][promotion_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                    //Remove opponents piece
                    for (x,y) in self.w1[kp][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= *x * sign;
                    }
                },
                chess::MoveType::ENPASSANT => {
                    //First just move the pawn
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                    //square to capture on; We are always in the white perspective
                    let cap_square = if piece_color == c {t - 8} else {t + 8};
                    let capture_index = calculate_piece_index(chess::Piece::PAWN, piece_color.other(), c);
                    //Capture the pawn
                    for (x,y) in self.w1[kp][capture_index][cap_square].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= *x * sign;
                    }
                },
                chess::MoveType::CASTLE => {
                    //Move the king
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                    //Move the rook
                    let ri = calculate_piece_index(chess::Piece::ROOK, c.other(), c);
                    let (rf,rt) = if t == 58 {
                        (56,59)
                    } else if t == 62 {
                        (63,61)
                    } else {return;};
                    for ((x,y),z) in self.w1[kp][ri][rf].iter().zip(self.w1[kp][ri][rt].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                }
                _ => {
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += (*y - *x) * sign;
                    }
                }
            }
        }
        //if the king has moved we need to update all weights in the position
        else {
            //Overwrite the old state
            let ksq = if undo {Some(m.from.square())} else {Some(m.to.square())};
            self.initialize_color_state(pos, c, ksq);
            if undo {return;}
            //TODO: Handle castling and captures
            let t = m.to.index() ^ flip;
            match m.typ {
                chess::MoveType::CAPTURE(p) => {
                    let capture_index = calculate_piece_index(p, c.other(), c);
                    for (x,y) in self.w1[t][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= *x;
                    }
                },
                chess::MoveType::CASTLE => {
                    let ri = calculate_piece_index(chess::Piece::ROOK, c, c);
                    if t == 2 {
                        let rf = 0;
                        let rt = 3;
                        for ((x,y),z) in self.w1[t][ri][rf].iter().zip(self.w1[t][ri][rt].iter()).zip(self.state[c as usize].iter_mut()) {
                            *z += *y - *x;
                        }
                    } else if t == 6 {
                        let rf = 7;
                        let rt = 5;
                        for ((x,y),z) in self.w1[t][ri][rf].iter().zip(self.w1[t][ri][rt].iter()).zip(self.state[c as usize].iter_mut()) {
                            *z += *y - *x;
                        }
                    }
                },
                _ => {},
            }
        }
    }
    //Apply a move to the first hidden layer (linear layer) state
    pub fn do_move(&mut self, m: chess::Move, pos: &chess::Position) {
        self.update_color_state(pos, m, chess::Color::WHITE, false);
        self.update_color_state(pos, m, chess::Color::BLACK, false);
    }
    pub fn undo_move(&mut self, m: chess::Move, pos: &chess::Position) {
        self.update_color_state(pos, m, chess::Color::WHITE, true);
        self.update_color_state(pos, m, chess::Color::BLACK, true);
    }
    //Initialize the first hidden layer with the given position
    pub fn initialize_state(&mut self, pos: &chess::Position) {
        self.initialize_color_state(pos, chess::Color::WHITE, None);
        self.initialize_color_state(pos, chess::Color::BLACK, None);
    }
    //Evaluate the current state of the input transformers
    pub fn evaluate_position(&self, pos: &chess::Position) -> i32 {
        let us = pos.color() as usize;
        let them = pos.color().other() as usize;

        let state_us_clipped: Vec<_> = self.state[us].iter().map(|x| (*x).clamp(0,127)).collect();
        let state_them_clipped: Vec<_> = self.state[them].iter().map(|x| (*x).clamp(0,127)).collect();

        let mut h2_acc = vec![0i32; 32];

        for (hn, (w,b)) in h2_acc.iter_mut().zip(self.w2.iter().zip(self.b2.iter())) {
            *hn = w[0].iter().zip(state_us_clipped.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += w[1].iter().zip(state_them_clipped.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += *b as i32 * Self::INPUT_SCALE;//TODO: scaling?
        }

        let h2: Vec<_> = h2_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        let mut h3_acc = vec![0i32; 32];
        for (hn, (w,b)) in h3_acc.iter_mut().zip(self.w3.iter().zip(self.b3.iter())) {
            *hn = w.iter().zip(h2.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += *b as i32 * 127;
        }

        let h3: Vec<_> = h3_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        self.w4.iter().zip(h3.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32) + self.b4 as i32 * 127
    }
}

#[cfg(test)]
mod tests {
    use rand::{seq::SliceRandom, thread_rng};
    use super::NNUEState;
    use super::super::chess::Position;

    fn do_and_undo(pos: &mut Position, nnue: &mut NNUEState, count: usize) {
        if count == 0 {return;}
        let eval = nnue.evaluate_position(&pos);
        match pos.get_last_move() {
            Some(m) => { let mut nnue2 = nnue.clone();
                         nnue2.initialize_state(&pos);
                         println!("move {}, {:?}, {:?}, {}", m, m.piece, m.typ, count);
                         assert!(nnue2.evaluate_position(&pos) == eval);
            },
            None => {}
        };
        let moves = pos.get_moves();
        let castle = pos.get_castling_rights();
        let m = match moves.choose(&mut thread_rng()) {
            Some(m) => m,
            None => return,
        };
        nnue.do_move(*m, &pos);
        pos.do_move(*m);
        do_and_undo(pos, nnue, count-1);
        pos.undo_move(*m, castle, None);
        nnue.undo_move(*m, &pos);
        let eval2 = nnue.evaluate_position(&pos);
        if eval != eval2 {println!("evals {}, {}, {}, {:?}, {:?}", eval, eval2, *m, m.piece, m.typ)};
        assert!(eval2 == eval);
    }

    #[test]
    fn load_model() {
        let now = std::time::Instant::now();
        let path = std::path::Path::new("/home/jannik/Code/kf/model.nnue");
        let mut nnue = NNUEState::from_weights(&path);
        println!("loaded!, {}", now.elapsed().as_millis());
        let acc = nnue.w1[5][6][4].iter().fold(0, |sum, x| sum + x);
        println!("wsum {}", acc);
        let pos = Position::new();
        let now_init = std::time::Instant::now();
        nnue.initialize_state(&pos);
        println!("initialized!, {}", now_init.elapsed().as_nanos());
        println!("{}", nnue.evaluate_position(&pos));
        let pos = Position::from_fen(String::from("8/4k3/p3b1p1/3n2R1/1P5P/5K2/3B4/8 b - - 6 65")).unwrap();
        let now_init = std::time::Instant::now();
        nnue.initialize_state(&pos);
        println!("initialized!, {}", now_init.elapsed().as_nanos());
        println!("{}", nnue.evaluate_position(&pos));
        let mut pos = Position::new();
        nnue.initialize_state(&pos);
        for _i in 0..1000 {
            do_and_undo(&mut pos, &mut nnue, 20);
        }
    }
    #[test]
    fn simple_move() {
        let path = std::path::Path::new("/home/jannik/Code/kf/model.nnue");
        let mut nnue = NNUEState::from_weights(&path);
        let mut nnue2 = NNUEState::from_weights(&path);
        let mut pos = Position::new();
        let m1 = super::chess::Move{from:12, to: 20, piece: super::chess::Piece::PAWN, typ: super::chess::MoveType::MOVE};
        let m2 = super::chess::Move{from:52, to: 44, piece: super::chess::Piece::PAWN, typ: super::chess::MoveType::MOVE};
        let m3 = super::chess::Move{from:4, to: 12, piece: super::chess::Piece::KING, typ: super::chess::MoveType::MOVE};
        nnue.initialize_state(&pos);
        nnue.do_move(m1, &pos);
        pos.do_move(m1);
        nnue.do_move(m2, &pos);
        pos.do_move(m2);
        nnue.do_move(m3, &pos);
        pos.do_move(m3);
        pos.undo_move(m3, [[true, true],[true,true]], None);
        nnue.undo_move(m3, &pos);
        nnue2.initialize_state(&pos);
        println!("{:?}\n{:?}", nnue.state[0], nnue2.state[0]);
        println!("{:?}\n{:?}", nnue.state[1], nnue2.state[1]);
        println!("{:?}\n{:?}", nnue.w1[4][5][12 ^ 56], nnue.w1[4][5][20 ^ 56]);
        assert!(nnue.w1 == nnue2.w1);
        assert!(nnue.state[0] == nnue2.state[0]);
        assert!(nnue.state[1] == nnue2.state[1]);
    }
    #[test]
    fn piece_indices() {
        assert!(super::calculate_piece_index(super::chess::Piece::KING, super::chess::Color::WHITE, super::chess::Color::BLACK) == 0);
        assert!(super::calculate_piece_index(super::chess::Piece::QUEEN, super::chess::Color::WHITE, super::chess::Color::BLACK) == 1);
        assert!(super::calculate_piece_index(super::chess::Piece::BISHOP, super::chess::Color::WHITE, super::chess::Color::BLACK) == 2);
        assert!(super::calculate_piece_index(super::chess::Piece::KNIGHT, super::chess::Color::WHITE, super::chess::Color::BLACK) == 3);
        assert!(super::calculate_piece_index(super::chess::Piece::ROOK, super::chess::Color::WHITE, super::chess::Color::BLACK) == 4);
        assert!(super::calculate_piece_index(super::chess::Piece::PAWN, super::chess::Color::WHITE, super::chess::Color::BLACK) == 5);
        assert!(super::calculate_piece_index(super::chess::Piece::QUEEN, super::chess::Color::WHITE, super::chess::Color::WHITE) == 6);
    }
}
