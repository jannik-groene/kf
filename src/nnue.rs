mod halfkav2;

use crate::chess::{SquareIndex, SquareIndexMethods, Move, Position, Piece, Color};
use nnue::{
    make_model,
    layers::Accumulator,
    features::{EnumerateFeatures, MoveFeatures, Perspective},
};

make_model!{sf_half_ka_v2, {64*64*11} => 1024 => 16 => 32 => 1, 8}

#[derive(Clone)]
pub struct NNUEState {
    //state of the first hidden layer
    states: Vec<Accumulator<512,8>>,
}

impl NNUEState {
    pub fn new(pos: &Position) -> NNUEState {
        let mut acc = Accumulator::new();
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Perspective::WHITE), 0);
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Perspective::BLACK), 1);
        NNUEState { states: vec![acc]}
    }

    //Input is the up-to-date position, after the move is done (or undone).
    pub fn update_color_state(&self, acc: &Accumulator<512,8>, acc_new: &mut Accumulator<512,8>,
                                                             pos: &Position, m: Move, c: Color) {
        //if the King has not moved the update is simple
        if m.piece != Piece::KING
                || pos.get_board()[(c.other(), Piece::KING)] & (m.from.square() | m.to.square()) != 0 {

            let kp = SquareIndex::from_square(pos.get_board()[(c, Piece::KING)]);

            let our_piece = pos.get_board()[(c, Piece::ANY)] & m.to.square() != 0;

            //select updated features
            let (added, removed) = m.changed_features(c.into(), kp, our_piece);
            sf_half_ka_v2::update_accumulator(acc, acc_new, added, removed, c as usize)
        }
        //if the king has moved we need to update all weights in the position
        else {
            sf_half_ka_v2::refresh_accumulator(acc_new, pos.features(c.into()), c as usize)
        }
    }

    //Apply a move to the first hidden layer (linear layer) state
    pub fn do_move(&mut self, m: Move, pos: &Position) {
        let mut acc = Accumulator::new();
        self.update_color_state(self.states.last().unwrap(), &mut acc, pos, m, Color::WHITE);
        self.update_color_state(self.states.last().unwrap(), &mut acc, pos, m, Color::BLACK);
        self.states.push(acc);
    }

    pub fn undo_move(&mut self) {
        self.states.pop();
    }

    //Evaluate the current state of the input transformers
    pub fn evaluate_position(&self, pos: &Position, to_move: Color) -> i32 {
        let bucket = (pos.board.occupation.count_ones() - 1) / 4;
        sf_half_ka_v2::evaluate_state(self.states.last().unwrap(), bucket as usize, to_move as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::chess::Position;
    use nnue::{
        layers::Accumulator,
        features::{EnumerateFeatures, Perspective},
        make_model,
    };

    make_model!{sf_half_ka_v2, {64*64*11} => 1024 => 16 => 32 => 1, 8}

    #[test]
    fn evaluate_position() {
        let pos = Position::new();
        sf_half_ka_v2::load_model(&std::path::Path::new("/home/jannik/Downloads/Stockfish/src/nn-33c9d39e5eb6.nnue")).unwrap();
        let mut acc = Accumulator::new();
        let fw = pos.features(Perspective::WHITE);
        let fb = pos.features(Perspective::BLACK);
        sf_half_ka_v2::refresh_accumulator(&mut acc, fw, 0);
        sf_half_ka_v2::refresh_accumulator(&mut acc, fb, 1);
        let eval = sf_half_ka_v2::evaluate_state(&acc, 7, 0);
        assert_eq!(35, eval);
    }
}
