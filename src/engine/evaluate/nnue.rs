use super::chess;
use super::chess::SquareMethods;

struct NNUEState {
    white_state: [i16; 256],
    black_state: [i16; 256],
    // We use 64 King Position * 64 Piece Positions * 11 Piece Types
    // The order in memory is essentially
    // [oppKing on a1 ourKing a1, ... , oppKing on h8 ourKing a1, oppKing a1 ourKing a2, ..., oppKing h8 ourKing a2, ..., oppQueen on a1 ourKing a1..., -- oppBishop --, -- oppKnight --,
    //  -- oppRook --, --oppPawn--, --ourQueen--, ........, --ourPawn--];
    w1: [[i16; 256]; 45056],
    w2: [[i8; 512]; 32],
    w3: [[i8; 32]; 32],
    w4: [i8; 32],
}

impl NNUEState {
    fn initialize_white_state(&mut self, pos: &chess::Position, king_pos: Option<chess::Square>) {
        let kp = match king_pos {
            Some(p) => p,
            None => pos.get_board()[(pos.color(), chess::Piece::KING)],
        }.trailing_zeros() as usize;
        self.white_state = [0; 256];
        //We go through each relevant piece
        let bkp = pos.get_board()[(chess::Color::BLACK, chess::Piece::KING)].trailing_zeros() as usize;
        for (x,y) in self.w1[bkp+64*kp].iter().zip(self.white_state.iter_mut()) {
            *y += x;
        }
        for bq in pos.get_board()[(chess::Color::BLACK, chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[bq.trailing_zeros() as usize +64*kp + 4096].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for bb in pos.get_board()[(chess::Color::BLACK, chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[bb.trailing_zeros() as usize +64*kp + 8192].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for bn in pos.get_board()[(chess::Color::BLACK, chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[bn.trailing_zeros() as usize +64*kp + 12288].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for br in pos.get_board()[(chess::Color::BLACK, chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[br.trailing_zeros() as usize +64*kp + 16384].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for bp in pos.get_board()[(chess::Color::BLACK, chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[bp.trailing_zeros() as usize + 64*kp + 20480].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for wq in pos.get_board()[(chess::Color::WHITE, chess::Piece::QUEEN)].iter() {
            for (x,y) in self.w1[wq.trailing_zeros() as usize +64*kp + 24576].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for wb in pos.get_board()[(chess::Color::WHITE, chess::Piece::BISHOP)].iter() {
            for (x,y) in self.w1[wb.trailing_zeros() as usize +64*kp + 28672].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for wn in pos.get_board()[(chess::Color::WHITE, chess::Piece::KNIGHT)].iter() {
            for (x,y) in self.w1[wn.trailing_zeros() as usize +64*kp + 32768].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for wr in pos.get_board()[(chess::Color::WHITE, chess::Piece::ROOK)].iter() {
            for (x,y) in self.w1[wr.trailing_zeros() as usize +64*kp + 36864].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
        for wp in pos.get_board()[(chess::Color::WHITE, chess::Piece::PAWN)].iter() {
            for (x,y) in self.w1[wp.trailing_zeros() as usize + 64*kp + 40960].iter().zip(self.white_state.iter_mut()) {
                *y += x;
            }
        }
    }
    //Input is the position _BEFORE_ the move is made.
    pub fn update_white_state(&mut self, pos: &chess::Position, m: chess::Move) {
        //if the King has not moved the update is simple
        if m.piece != chess::Piece::KING || pos.color() != chess::Color::WHITE {
            let kp = pos.get_board()[(pos.color(), chess::Piece::KING)].trailing_zeros() as usize;
            let f = m.from.trailing_zeros() as usize;
            let t = m.to.trailing_zeros() as usize;

            let mut offset = kp*64 + 4096*(m.piece as usize)*pos.get_board()[(pos.color(), chess::Piece::KING)].trailing_zeros() as usize;

            if pos.color() == chess::Color::WHITE {
                offset += 64*64+5;
            }
            //Update loop
            //Consider faster crate for simd?
            for ((x,y),z) in self.w1[offset + f].iter().zip(self.w1[offset + t].iter()).zip(self.white_state.iter_mut()) {
                *z += y - x;
            }
        }
        //if the king has moved we need to update all weights in the position
        else {
            //Overwrite the old state
            self.initialize_white_state(pos, Some(m.to));
        }
    }
}
