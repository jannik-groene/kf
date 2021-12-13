use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use arrayvec::ArrayVec;
use lazy_static::lazy_static;
use crate::chess::{Square, SquareMethods, SquareIndexMethods, Move, MoveType, Position, Piece, Color};

lazy_static! {
    static ref NNUE_DATA: Arc<RwLock<NNUEData>> = Arc::new(RwLock::new(NNUEData {
        w1: Box::new([[[[0; 256]; 64]; 11]; 64]),
        w2: Box::new([[[0; 256]; 2]; 32]),
        w3: [[0; 32]; 32],
        w4: [0; 32],
        b1: [0; 256],
        b2: [0; 32],
        b3: [0; 32],
        b4: 0,
    }));
}

struct NNUEData {
    // We use 64 King Position * 64 Piece Positions * 11 Piece Types
    // It is indexed as w1[king position][piece type][piece position]
    w1: Box<[[[[i16; 256]; 64]; 11]; 64]>,
    w2: Box<[[[i8; 256]; 2]; 32]>,
    w3: [[i8; 32]; 32],
    w4: [i8; 32],
    b1: [i16; 256],
    b2: [i8; 32],
    b3: [i8; 32],
    b4: i8,
}

#[derive(Clone)]
pub struct NNUEState {
    //state of the first hidden layer
    state: [[i16; 256]; 2],
    data: &'static NNUE_DATA,
}

#[inline(always)]
fn calculate_piece_index(p: Piece, c: Color, us: Color) -> usize {
    let index_base = [5,4,2,1,3,0];
    let mut idx = 2 * index_base[p as usize] + if c == us {0} else {1};
    if p == Piece::KING {
        idx -= 1;
    }
    idx
    //if c == us {
    //    p as usize + 5
    //} else {
    //    p as usize
    //}
}

impl NNUEState {
    const WEIGHT_SCALE: i32 = 64;
    const INPUT_SCALE: i32 = 127;
    pub fn new(pos: &Position) -> NNUEState {
        let mut nnue = NNUEState {
            state: [[0; 256]; 2],
            data: &NNUE_DATA,
        };
        nnue.initialize_color_state(pos, Color::WHITE, None);
        nnue.initialize_color_state(pos, Color::BLACK, None);
        nnue
    }
    pub fn load_weights(weight_path: &Path) {
        let file = File::open(weight_path).unwrap();
        let mut buf = BufReader::new(file);
        let mut entry = [0u8; 4];
        let mut data = NNUE_DATA.write().unwrap();
        //read feature transformer weights
        for kpos in 0..64 {
            for ptype in 0..11 {
                for ppos in 0..64 {
                    for i in 0..256 {
                        buf.read(&mut entry).unwrap();
                        data.w1[kpos][ptype][ppos][i] = (f32::from_ne_bytes(entry) * Self::INPUT_SCALE as f32) as i16;
                    }
                }
            }
        }
        //read feature transformer biases
        for i in 0..256 {
            buf.read(&mut entry).unwrap();
            data.b1[i] = (f32::from_ne_bytes(entry) * Self::INPUT_SCALE as f32) as i16;
        }
        //read first linear layer and its biases
        for i in 0..32 {
            for j in 0..512 {
                buf.read(&mut entry).unwrap();
                data.w2[i][j/256][j%256] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
            }
        }
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            data.b2[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        //read second linear layer
        for i in 0..32 {
            for j in 0..32 {
                buf.read(&mut entry).unwrap();
                data.w3[i][j] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
            }
        }
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            data.b3[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        //read last layer
        for i in 0..32 {
            buf.read(&mut entry).unwrap();
            data.w4[i] = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
        }
        buf.read(&mut entry).unwrap();
        data.b4 = (f32::from_ne_bytes(entry) * Self::WEIGHT_SCALE as f32) as i8;
    }
    //Initialize the input layer state. A certain king position may be specified to be used instead
    //of the one on the board (used when updating the net state after a king move)
    fn initialize_color_state(&mut self, pos: &Position, c: Color, king_pos: Option<Square>) {
        //Used to flip the board into the white player perspective
        let data = self.data.read().unwrap();
        let flip = match c {Color::WHITE => 0, Color::BLACK => 56};
        let kp = match king_pos {
            Some(p) => p,
            None => pos.get_board()[(c, Piece::KING)],
        }.index() ^ flip;
        //Apply the bias
        self.state[c as usize].iter_mut().zip(data.b1.iter()).for_each(|(x,y)| *x = *y);
        //We go through each relevant piece
        //for (x,y) in self.w1[kp][0][kp].iter().zip(self.state[c as usize].iter_mut()) {
        //    *y += x;
        //}
        let okp = pos.get_board()[(c.other(), Piece::KING)].index() ^ flip;
        for (x,y) in data.w1[kp][10][okp].iter().zip(self.state[c as usize].iter_mut()) {
            *y += *x;
        }
        for oq in pos.get_board()[(c.other(), Piece::QUEEN)].iter() {
            for (x,y) in data.w1[kp][9][oq.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for ob in pos.get_board()[(c.other(), Piece::BISHOP)].iter() {
            for (x,y) in data.w1[kp][5][ob.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for on in pos.get_board()[(c.other(), Piece::KNIGHT)].iter() {
            for (x,y) in data.w1[kp][3][on.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for or in pos.get_board()[(c.other(), Piece::ROOK)].iter() {
            for (x,y) in data.w1[kp][7][or.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for op in pos.get_board()[(c.other(), Piece::PAWN)].iter() {
            for (x,y) in data.w1[kp][1][op.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for q in pos.get_board()[(c, Piece::QUEEN)].iter() {
            for (x,y) in data.w1[kp][8][q.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for b in pos.get_board()[(c, Piece::BISHOP)].iter() {
            for (x,y) in data.w1[kp][4][b.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for n in pos.get_board()[(c, Piece::KNIGHT)].iter() {
            for (x,y) in data.w1[kp][2][n.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for r in pos.get_board()[(c, Piece::ROOK)].iter() {
            for (x,y) in data.w1[kp][6][r.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
        for p in pos.get_board()[(c, Piece::PAWN)].iter() {
            for (x,y) in data.w1[kp][0][p.index() ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += *x;
            }
        }
    }
    #[inline]
    fn replace_feature(&mut self, old: (usize,usize,usize), new: (usize,usize,usize), undo: bool, c: Color, data: &RwLockReadGuard<NNUEData>) {
        if !undo {
            for ((old_weight, new_weight), state) in data.w1[old.0][old.1][old.2].iter()
                                                       .zip(data.w1[new.0][new.1][new.2].iter())
                                                       .zip(self.state[c as usize].iter_mut()) {
                *state += *new_weight - *old_weight;
            }
        } else {
            for ((old_weight, new_weight), state) in data.w1[old.0][old.1][old.2].iter()
                                                       .zip(data.w1[new.0][new.1][new.2].iter())
                                                       .zip(self.state[c as usize].iter_mut()) {
                *state += *old_weight - *new_weight;
            }
        }
    }
    #[inline]
    fn remove_feature(&mut self, feature: (usize,usize,usize), c: Color, undo: bool, data: &RwLockReadGuard<NNUEData>) {
        if !undo {
            for (weight,state) in data.w1[feature.0][feature.1][feature.2].iter()
                                    .zip(self.state[c as usize].iter_mut()) {
                *state -= *weight;
            }
        } else {
            for (weight,state) in data.w1[feature.0][feature.1][feature.2].iter()
                                    .zip(self.state[c as usize].iter_mut()) {
                *state += *weight;
            }
        }
    }
    //Input is the position _BEFORE_ the move is made.
    pub fn update_color_state(&mut self, pos: &Position, m: Move, c: Color, undo: bool) {
        let data = self.data.read().unwrap();
        //We flip the bord to out perspective by ^-ing with flip
        let flip = match c {Color::WHITE => 0, Color::BLACK => 56};
        //if the King has not moved the update is simple
        if m.piece != Piece::KING
                || pos.get_board()[(c.other(), Piece::KING)] == m.from.square() {

            let kp = pos.get_board()[(c, Piece::KING)].index() ^ flip;
            let f = m.from.index() ^ flip;
            let t = m.to.index() ^ flip;

            let piece_color = if pos.get_board()[(c, Piece::ANY)] & m.from.square() != 0 {c} else {c.other()};

            let piece_index = calculate_piece_index(m.piece, piece_color, c);

            //Update loop
            //Consider faster crate for simd?
            match m.typ {
                MoveType::CAPTURE(p) => {
                    self.replace_feature((kp,piece_index,f), (kp,piece_index,t), undo, c, &data);
                    let capture_index = calculate_piece_index(p, piece_color.other(), c);
                    self.remove_feature((kp,capture_index,t), c, undo, &data);
                },
                MoveType::PROMOTION(p) => {
                    let promotion_index = calculate_piece_index(p, piece_color, c);
                    self.replace_feature((kp,piece_index,f), (kp,promotion_index,t), undo, c, &data);
                },
                MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => {
                    let promotion_index = calculate_piece_index(p_prom, piece_color, c);
                    let capture_index = calculate_piece_index(p_cap, piece_color.other(), c);
                    //Move and promote piece
                    self.replace_feature((kp,piece_index,f), (kp,promotion_index,t), undo, c, &data);
                    //Remove opponents piece
                    self.remove_feature((kp,capture_index,t), c, undo, &data);
                },
                MoveType::ENPASSANT => {
                    //First just move the pawn
                    self.replace_feature((kp,piece_index,f), (kp,piece_index,t), undo, c, &data);
                    //square to capture on; We are always in the white perspective
                    let cap_square = if piece_color == c {t - 8} else {t + 8};
                    let capture_index = calculate_piece_index(Piece::PAWN, piece_color.other(), c);
                    //Capture the pawn
                    self.remove_feature((kp,capture_index,cap_square), c, undo, &data);
                },
                MoveType::CASTLE => {
                    //Move the king
                    self.replace_feature((kp,piece_index,f), (kp,piece_index,t), undo, c, &data);
                    //Move the rook
                    let ri = calculate_piece_index(Piece::ROOK, c.other(), c);
                    let (rf,rt) = if t == 58 {
                        (56,59)
                    } else {
                        (63,61)
                    };
                    self.replace_feature((kp,ri,rf), (kp,ri,rt), undo, c, &data);
                }
                _ => {
                    self.replace_feature((kp,piece_index,f), (kp,piece_index,t), undo, c, &data);
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
                MoveType::CAPTURE(p) => {
                    let ci = calculate_piece_index(p, c.other(), c);
                    self.remove_feature((t,ci,t), c, undo, &data);
                },
                MoveType::CASTLE => {
                    let ri = calculate_piece_index(Piece::ROOK, c, c);
                    let (rf,rt) = if t == 2 {
                        (0,3)
                    } else {
                        (7,5)
                    };
                    self.replace_feature((t,ri,rf), (t,ri,rt), undo, c, &data);
                },
                _ => {},
            }
        }
    }
    //Apply a move to the first hidden layer (linear layer) state
    pub fn do_move(&mut self, m: Move, pos: &Position) {
        self.update_color_state(pos, m, Color::WHITE, false);
        self.update_color_state(pos, m, Color::BLACK, false);
    }
    pub fn undo_move(&mut self, m: Move, pos: &Position) {
        self.update_color_state(pos, m, Color::WHITE, true);
        self.update_color_state(pos, m, Color::BLACK, true);
    }
    //Initialize the first hidden layer with the given position
    pub fn initialize_state(&mut self, pos: &Position) {
        self.initialize_color_state(pos, Color::WHITE, None);
        self.initialize_color_state(pos, Color::BLACK, None);
    }
    //Evaluate the current state of the input transformers
    pub fn evaluate_position(&self, pos: &Position) -> i32 {
        let data = self.data.read().unwrap();

        let us = pos.color() as usize;
        let them = pos.color().other() as usize;

        let state_us_clipped: ArrayVec<_,256> = self.state[us].iter().map(|x| (*x).clamp(0,127)).collect();
        let state_them_clipped: ArrayVec<_,256> = self.state[them].iter().map(|x| (*x).clamp(0,127)).collect();

        let mut h2_acc = [0i32; 32];

        for (hn, (w,b)) in h2_acc.iter_mut().zip(data.w2.iter().zip(data.b2.iter())) {
            *hn = w[0].iter().zip(state_us_clipped.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += w[1].iter().zip(state_them_clipped.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += *b as i32 * Self::INPUT_SCALE;//TODO: scaling?
        }

        let h2: ArrayVec<_,256> = h2_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        let mut h3_acc = [0i32; 32];
        for (hn, (w,b)) in h3_acc.iter_mut().zip(data.w3.iter().zip(data.b3.iter())) {
            *hn = w.iter().zip(h2.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32);
            *hn += *b as i32 * Self::INPUT_SCALE;
        }

        let h3: ArrayVec<_,32> = h3_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        -(data.w4.iter().zip(h3.iter()).fold(0, |sum, (x,y)| sum + *x as i32 * *y as i32) + data.b4 as i32 * Self::INPUT_SCALE) * 600 / 361 / 128
    }
}

#[cfg(test)]
mod tests {
    use rand::{seq::SliceRandom, thread_rng};
    use super::NNUEState;
    use crate::chess::{Position, Move, MoveType, Color, Piece};

    fn do_and_undo(pos: &mut Position, nnue: &mut NNUEState, count: usize) {
        if count == 0 {return;}
        //let eval = nnue.evaluate_position(&pos);
        let moves = pos.get_moves();
        let m = match moves.choose(&mut thread_rng()) {
            Some(m) => m,
            None => return,
        };
        nnue.do_move(*m, &pos);
        pos.do_move(*m);
        do_and_undo(pos, nnue, count-1);
        pos.undo_move();
        nnue.undo_move(*m, &pos);
        //let eval2 = nnue.evaluate_position(&pos);
        //if eval != eval2 {println!("evals {}, {}, {}, {:?}, {:?}", eval, eval2, *m, m.piece, m.typ)};
        //assert!(eval2 == eval);
    }

    #[test]
    fn load_model() {
        let now = std::time::Instant::now();
        let path = std::path::Path::new("/home/jannik/Code/kf/model.nnue");
        NNUEState::load_weights(&path);
        println!("loaded!, {}", now.elapsed().as_millis());
        let pos = Position::new();
        let mut nnue = NNUEState::new(&pos);
        let now_init = std::time::Instant::now();
        println!("initialized!, {}", now_init.elapsed().as_nanos());
        println!("{}", nnue.evaluate_position(&pos));
        let pos = Position::from_fen(String::from("8/4k3/p3b1p1/3n2R1/1P5P/5K2/3B4/8 b - - 6 65")).unwrap();
        let now_init = std::time::Instant::now();
        nnue.initialize_state(&pos);
        println!("initialized!, {}", now_init.elapsed().as_nanos());
        println!("{}", nnue.evaluate_position(&pos));
        let mut pos = Position::new();
        nnue.initialize_state(&pos);
        let now_loop = std::time::Instant::now();
        for _i in 0..10000 {
            do_and_undo(&mut pos, &mut nnue, 20);
        }
        println!("Took {} ms", now_loop.elapsed().as_millis());
        panic!()
    }
    #[test]
    fn test_evals() {
        let mut pos = Position::new();
        let path = std::path::Path::new("/home/jannik/Code/kf/model.nnue");
        NNUEState::load_weights(&path);
        let mut nnue = NNUEState::new(&pos);
        let eval = nnue.evaluate_position(&pos);
        pos.do_null_move();
        assert!(eval == nnue.evaluate_position(&pos));
        let test_fens = [
            String::from("3k4/8/8/8/P6P/8/8/3K4 w - - 0 1"), //winning
            String::from("3k4/8/8/8/P6P/8/8/3K4 b - - 0 1"), //losing
            String::from("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPPKPPP/RNBQ1BNR b kq - 0 1"), //bongcloud
            String::from("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPPKPPP/RNBQ1BNR w kq - 0 1"), //bongcloud
            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp3/8/NP4PN/P1PPPP1P/R1BQKB1R w KQkq - 0 1"), //losing
            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp3/8/NP4PN/P1PPPP1P/R1BQKB1R b KQkq - 0 1"), //winning
            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp1K1/8/NP4PN/P1PPPP1P/R1BQ1B1R b kq - 0 1"), //winning
        ];
        for fen in test_fens {
            pos = Position::from_fen(fen).unwrap();
            nnue.initialize_state(&pos);
            println!("eval {}", nnue.evaluate_position(&pos));
        }
        panic!()
    }
    #[test]
    fn simple_move() {
        let path = std::path::Path::new("/home/jannik/Code/kf/model.nnue");
        NNUEState::load_weights(&path);
        let mut pos = Position::new();
        let mut nnue = NNUEState::new(&pos);
        let mut nnue2 = NNUEState::new(&pos);
        let m1 = Move{from:12, to: 20, piece: Piece::PAWN, typ: MoveType::MOVE};
        let m2 = Move{from:52, to: 44, piece: Piece::PAWN, typ: MoveType::MOVE};
        let m3 = Move{from:4, to: 12, piece: Piece::KING, typ: MoveType::MOVE};
        nnue.initialize_state(&pos);
        nnue.do_move(m1, &pos);
        pos.do_move(m1);
        nnue.do_move(m2, &pos);
        pos.do_move(m2);
        nnue.do_move(m3, &pos);
        pos.do_move(m3);
        pos.undo_move();
        nnue.undo_move(m3, &pos);
        nnue2.initialize_state(&pos);
        println!("{:?}\n{:?}", nnue.state[0], nnue2.state[0]);
        println!("{:?}\n{:?}", nnue.state[1], nnue2.state[1]);
        assert!(nnue.state[0] == nnue2.state[0]);
        assert!(nnue.state[1] == nnue2.state[1]);
    }
    #[test]
    fn piece_indices() {
        assert!(super::calculate_piece_index(Piece::KING, Color::WHITE, Color::BLACK) == 0);
        assert!(super::calculate_piece_index(Piece::QUEEN, Color::WHITE, Color::BLACK) == 1);
        assert!(super::calculate_piece_index(Piece::BISHOP, Color::WHITE, Color::BLACK) == 2);
        assert!(super::calculate_piece_index(Piece::KNIGHT, Color::WHITE, Color::BLACK) == 3);
        assert!(super::calculate_piece_index(Piece::ROOK, Color::WHITE, Color::BLACK) == 4);
        assert!(super::calculate_piece_index(Piece::PAWN, Color::WHITE, Color::BLACK) == 5);
        assert!(super::calculate_piece_index(Piece::QUEEN, Color::WHITE, Color::WHITE) == 6);
    }
}
