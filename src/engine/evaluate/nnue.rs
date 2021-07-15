use super::chess;
use super::chess::SquareMethods;

struct NNUEState {
    //state of the first hidden layer
    state: [[i16; 256]; 2],
    // We use 64 King Position * 64 Piece Positions * 11 Piece Types
    // It is indexed as w1[king position][piece type][piece position]
    w1: [[[[i16; 256]; 64]; 11]; 64],
    w2: [[[i8; 256]; 2]; 32],
    w3: [[i8; 32]; 32],
    w4: [i8; 32],
    b1: [i16; 256],
    b2: [[[i8; 256]; 2]; 32],
    b3: [[i8; 32]; 32],
    b4: [i8; 32],
}

#[inline(always)]
fn calculate_piece_index(kp: usize, p: chess::Piece, c: chess:: Color, pos: &chess::Position) -> usize {
    if c == pos.color() {
        p as usize + 5
    } else {
        p as usize
    }
}

impl NNUEState {
    const WEIGHT_SCALE: i32 = 64;
    const INPUT_SCALE: i32 = 127;
    //Initialize the input layer state. A certain king position may be specified to be used instead
    //of the one on the board (used when updating the net state after a king move)
    fn initialize_color_state(&mut self, pos: &chess::Position, c: chess::Color, king_pos: Option<chess::Square>) {
        //Used to flip the board into the white player perspective
        let flip = match c {chess::Color::WHITE => 0, chess::Color::BLACK => 56};
        let kp = match king_pos {
            Some(p) => p,
            None => pos.get_board()[(c, chess::Piece::KING)],
        }.trailing_zeros() as usize ^ flip;
        //Apply the bias
        self.state[c as usize] = self.b1;
        //We go through each relevant piece
        for (x,y) in self.w1[kp][0][kp].iter().zip(self.state[c as usize].iter_mut()) {
            *y += x;
        }
        let okp = pos.get_board()[(c.other(), chess::Piece::KING)].trailing_zeros() as usize ^ flip;
        for (x,y) in self.w1[kp][0][okp.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
            *y += x;
        }
        for oq in pos.get_board()[(c.other(), chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[kp][1][oq.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for ob in pos.get_board()[(c.other(), chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[kp][2][ob.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for on in pos.get_board()[(c.other(), chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[kp][3][on.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for or in pos.get_board()[(c.other(), chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[kp][4][or.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for op in pos.get_board()[(c.other(), chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[kp][5][op.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for q in pos.get_board()[(c, chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[kp][6][q.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for b in pos.get_board()[(c, chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[kp][7][b.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for n in pos.get_board()[(c, chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[kp][8][n.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for r in pos.get_board()[(c, chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[kp][9][r.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
        for p in pos.get_board()[(c, chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[kp][10][p.trailing_zeros() as usize ^ flip].iter().zip(self.state[c as usize].iter_mut()) {
                *y += x;
            }
        }
    }
    //Input is the position _BEFORE_ the move is made.
    pub fn update_color_state(&mut self, pos: &chess::Position, m: chess::Move, c: chess::Color) {
        //We flip the bord to out perspective by ^-ing with flip
        let flip = match c {chess::Color::WHITE => 0, chess::Color::BLACK => 56};
        //if the King has not moved the update is simple
        if m.piece != chess::Piece::KING || pos.color() != c {
            let kp = pos.get_board()[(c, chess::Piece::KING)].trailing_zeros() as usize ^ flip;
            let f = m.from.trailing_zeros() as usize ^ flip;
            let t = m.to.trailing_zeros() as usize ^ flip;

            let piece_index = calculate_piece_index(kp, m.piece, c, pos);

            //Update loop
            //Consider faster crate for simd?
            match m.typ {
                chess::MoveType::CAPTURE(p) => {
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += y - x;
                    }
                    let capture_index = calculate_piece_index(kp, p, c.other(), pos);
                    for (x,y) in self.w1[kp][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= x;
                    }
                },
                chess::MoveType::PROMOTION(p) => {
                    let promotion_index = calculate_piece_index(kp, p, c, pos);
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][promotion_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += y - x;
                    }
                },
                chess::MoveType::PROMOTIONCAPTURE((p_prom,p_cap)) => {
                    let promotion_index = calculate_piece_index(kp, p_prom, c, pos);
                    let capture_index = calculate_piece_index(kp, p_cap, c.other(), pos);
                    //Move and promote piece
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][promotion_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += y - x;
                    }
                    //Remove opponents piece
                    for (x,y) in self.w1[kp][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= x;
                    }
                },
                chess::MoveType::ENPASSANT => {
                    //First just move the pawn
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += y - x;
                    }
                    //Figure out the square to capture on
                    let cap_square = if c == chess::Color::WHITE {
                                         t - 8
                                     } else {
                                         t + 8
                                     };
                    //Capture the pawn
                    for (x,y) in self.w1[kp][6][cap_square].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= x;
                    }
                },
                _ => {
                    for ((x,y),z) in self.w1[kp][piece_index][f].iter().zip(self.w1[kp][piece_index][t].iter()).zip(self.state[c as usize].iter_mut()) {
                        *z += y - x;
                    }
                }
            }
        }
        //if the king has moved we need to update all weights in the position
        else {
            //Overwrite the old state
            self.initialize_color_state(pos, c, Some(m.to));
            //TODO: Handle castling and captures
            let t = m.to.trailing_zeros() as usize ^ flip;
            match m.typ {
                chess::MoveType::CAPTURE(p) => {
                    let capture_index = calculate_piece_index(t, p, c.other(), pos);
                    for (x,y) in self.w1[t][capture_index][t].iter().zip(self.state[c as usize].iter_mut()) {
                        *y -= x;
                    }
                },
                chess::MoveType::CASTLE => {
                    if t == 2 {
                        let rf = 0;
                        let rt = 3;
                        for ((x,y),z) in self.w1[t][9][rf].iter().zip(self.w1[t][9][rt].iter()).zip(self.state[c as usize].iter_mut()) {
                            *z += y - x;
                        }
                    } else if t == 6 {
                        let rf = 7;
                        let rt = 5;
                        for ((x,y),z) in self.w1[t][9][rf].iter().zip(self.w1[t][9][rt].iter()).zip(self.state[c as usize].iter_mut()) {
                            *z += y - x;
                        }
                    }
                },
                _ => {},
            }
        }
    }
    //Apply a move to the first hidden layer (linear layer) state
    pub fn update_state(&mut self, m: chess::Move, pos: &chess::Position) {
        self.update_color_state(pos, m, chess::Color::WHITE);
        self.update_color_state(pos, m, chess::Color::BLACK);
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
            *hn = w[0].iter().zip(b[0].iter()).zip(state_us_clipped.iter()).fold(0, |sum, tup| sum + *tup.0.0 as i32 * *tup.1 as i32 + *tup.0.1 as i32);
            *hn += w[1].iter().zip(b[1].iter()).zip(state_them_clipped.iter()).fold(0, |sum, tup| sum + *tup.0.0 as i32 * *tup.1 as i32 + *tup.0.1 as i32);
        }

        let h2: Vec<_> = h2_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        let mut h3_acc = vec![0i32; 0];
        for (hn, (w,b)) in h3_acc.iter_mut().zip(self.w3.iter().zip(self.b3.iter())) {
            *hn = w.iter().zip(b.iter()).zip(h2.iter()).fold(0, |sum, tup| sum + *tup.0.0 as i32 * *tup.1 as i32 + *tup.0.1 as i32)
        }

        let h3: Vec<_> = h3_acc.iter().map(|x| (*x / Self::WEIGHT_SCALE).clamp(0,127) as i8).collect();

        self.w4.iter().zip(self.b4.iter()).zip(h3.iter()).fold(0, |sum, tup| sum + *tup.0.0 as i32 * *tup.1 as i32 + *tup.0.1 as i32)
    }
}
