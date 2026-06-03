mod halfkav2;

use crate::chess::{Color, Move, Piece, Position};
use nnue::{
    features::{EnumerateFeatures, MoveFeatures, Perspective},
    make_model,
};

make_model! {sf_half_ka_v2, 45056 => 1024 => 16 => 32 => 1, 8}

use sf_half_ka_v2::Accumulator;

#[derive(Clone)]
pub struct NNUEState {
    //state of the first hidden layer
    states: Vec<Accumulator>,
}

impl NNUEState {
    pub fn new(pos: &Position) -> NNUEState {
        let mut acc = Accumulator::new();
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Perspective::WHITE), 0);
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Perspective::BLACK), 1);
        NNUEState { states: vec![acc] }
    }

    //Input is the up-to-date position, after the move is done (or undone).
    pub fn update_color_state(
        &self,
        acc: &Accumulator,
        acc_new: &mut Accumulator,
        pos: &Position,
        m: Move,
        c: Color,
    ) {
        //if the King has not moved the update is simple
        if m.piece != Piece::King
            || pos
                .get_board()
                .get_bb(c.other(), Piece::King)
                .is_set(m.from)
            || pos.get_board().get_bb(c.other(), Piece::King).is_set(m.to)
        {
            let kp = pos.get_board().get_bb(c, Piece::King).least_square();

            let our_piece = pos.get_board().get_color_bb(c).is_set(m.to);

            //select updated features
            let p = pos.get_board().piece_at(m.to);
            let (added, removed) = (m,p).changed_features(c.into(), kp.into(), our_piece);
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
        self.update_color_state(self.states.last().unwrap(), &mut acc, pos, m, Color::White);
        self.update_color_state(self.states.last().unwrap(), &mut acc, pos, m, Color::Black);
        self.states.push(acc);
    }

    pub fn undo_move(&mut self) {
        self.states.pop();
    }

    //Evaluate the current state of the input transformers
    pub fn evaluate_position(&self, pos: &Position, to_move: Color) -> i32 {
        let bucket = (pos.board.occupation().count() - 1) / 4;
        assert!(bucket < 8);
        sf_half_ka_v2::evaluate_state(
            self.states.last().unwrap(),
            bucket as usize,
            to_move as usize,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::chess::Position;
    use nnue::{
        features::{EnumerateFeatures, Perspective},
        make_model,
    };

    make_model!(sf_half_ka_v2, 45056 => 1024 => 16 => 32 => 1, 8);

    #[test]
    #[ignore]
    fn evaluate_position() {
        let pos = Position::new();
        sf_half_ka_v2::load_model(std::path::Path::new("nn-33c9d39e5eb6.nnue")).unwrap();
        let mut acc = sf_half_ka_v2::Accumulator::new();
        let fw = pos.features(Perspective::WHITE);
        let fb = pos.features(Perspective::BLACK);
        sf_half_ka_v2::refresh_accumulator(&mut acc, fw, 0);
        sf_half_ka_v2::refresh_accumulator(&mut acc, fb, 1);
        let eval = sf_half_ka_v2::evaluate_state(&acc, 7, 0);
        assert_eq!(35, eval);
    }
}
