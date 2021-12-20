mod model;
mod features;
mod halfkav2;
mod layers;
mod intrinsics;

use crate::chess::{SquareIndex, SquareIndexMethods, Move, Position, Piece, Color};
use features::{EnumerateFeatures, MoveFeatures};
use layers::Accumulator;
use crate::make_model;

make_model!{sf_half_ka_v2, {64*64*11}, 512, 8,
            (l1, 1024, 16),
            (l2,   16, 32),
            (l3,   32,  1)}

#[derive(Clone)]
pub struct NNUEState {
    //state of the first hidden layer
    states: Vec<Accumulator<512,8>>,
}

impl NNUEState {
    pub fn new(pos: &Position) -> NNUEState {
        let mut acc = Accumulator::new();
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Color::WHITE), 0);
        sf_half_ka_v2::refresh_accumulator(&mut acc, pos.features(Color::BLACK), 1);
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
            let (added, removed) = m.changed_features(c, kp, our_piece);
            sf_half_ka_v2::update_accumulator(acc, acc_new, added, removed, c as usize)
        }
        //if the king has moved we need to update all weights in the position
        else {
            sf_half_ka_v2::refresh_accumulator(acc_new, pos.features(c), c as usize)
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

//#[cfg(test)]
//mod tests {
//    use rand::{seq::SliceRandom, thread_rng};
//    use super::NNUEState;
//    use crate::chess::{Position, Move, MoveType, Piece};
//
//    fn do_and_undo(pos: &mut Position, nnue: &mut NNUEState, count: usize) {
//        if count == 0 {return;}
//        //let eval = nnue.evaluate_position(pos.color());
//        let moves = pos.get_moves();
//        let m = match moves.choose(&mut thread_rng()) {
//            Some(m) => m,
//            None => return,
//        };
//        pos.do_move(*m);
//        nnue.do_move(*m, &pos);
//        do_and_undo(pos, nnue, count-1);
//        pos.undo_move();
//        nnue.undo_move(*m, &pos);
//        //let eval2 = nnue.evaluate_position(pos.color());
//        //if eval != eval2 {println!("evals {}, {}, {}, {:?}, {:?}, {}", eval, eval2, *m, m.piece, m.typ, count)};
//        //assert!(eval2 == eval);
//    }
//
//    #[test]
//    fn load_model() {
//        let now = std::time::Instant::now();
//        let path = std::path::Path::new("/home/jannik/Code/kf_training/model.nnue");
//        super::load_weights(&path);
//        println!("loaded!, {}", now.elapsed().as_millis());
//        let pos = Position::new();
//        let mut nnue = NNUEState::new(&pos);
//        let now_init = std::time::Instant::now();
//        println!("initialized!, {}", now_init.elapsed().as_nanos());
//        println!("{}", nnue.evaluate_position(pos.color()));
//        let pos = Position::from_fen(String::from("8/4k3/p3b1p1/3n2R1/1P5P/5K2/3B4/8 b - - 6 65")).unwrap();
//        let now_init = std::time::Instant::now();
//        nnue.initialize_state(&pos);
//        println!("initialized!, {}", now_init.elapsed().as_nanos());
//        println!("{}", nnue.evaluate_position(pos.color()));
//        let mut pos = Position::new();
//        nnue.initialize_state(&pos);
//        let now_loop = std::time::Instant::now();
//        for _i in 0..10000 {
//            do_and_undo(&mut pos, &mut nnue, 20);
//        }
//        println!("Took {} ms", now_loop.elapsed().as_millis());
//        panic!()
//    }
//    #[test]
//    fn test_evals() {
//        let mut pos = Position::new();
//        let path = std::path::Path::new("/home/jannik/Code/kf_training/model.nnue");
//        super::load_weights(&path);
//        let mut nnue = NNUEState::new(&pos);
//        let eval = nnue.evaluate_position(pos.color());
//        pos.do_null_move();
//        assert!(eval == nnue.evaluate_position(pos.color()));
//        let test_fens = [
//            String::from("3k4/8/8/8/P6P/8/8/3K4 w - - 0 1"), //winning
//            String::from("3k4/8/8/8/P6P/8/8/3K4 b - - 0 1"), //losing
//            String::from("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPPKPPP/RNBQ1BNR b kq - 0 1"), //bongcloud
//            String::from("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPPKPPP/RNBQ1BNR w kq - 0 1"), //bongcloud
//            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp3/8/NP4PN/P1PPPP1P/R1BQKB1R w KQkq - 0 1"), //losing
//            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp3/8/NP4PN/P1PPPP1P/R1BQKB1R b KQkq - 0 1"), //winning
//            String::from("r1bqkb1r/ppp2ppp/2n2n2/3pp1K1/8/NP4PN/P1PPPP1P/R1BQ1B1R b kq - 0 1"), //winning
//        ];
//        for fen in test_fens {
//            pos = Position::from_fen(fen).unwrap();
//            nnue.initialize_state(&pos);
//            println!("eval {}", nnue.evaluate_position(pos.color()));
//        }
//        panic!()
//    }
//    #[test]
//    fn simple_move() {
//        let path = std::path::Path::new("/home/jannik/Code/kf_training/model.nnue");
//        super::load_weights(&path);
//        let mut pos = Position::new();
//        let mut nnue = NNUEState::new(&pos);
//        let m1 = Move{from: 12, to: 20, piece: Piece::PAWN, typ: MoveType::MOVE};
//        let m2 = Move{from: 52, to: 44, piece: Piece::PAWN, typ: MoveType::MOVE};
//        let m3 = Move{from:  4, to: 12, piece: Piece::KING, typ: MoveType::MOVE};
//        nnue.initialize_state(&pos);
//        pos.do_move(m1);
//        nnue.do_move(m1, &pos);
//        pos.do_move(m2);
//        nnue.do_move(m2, &pos);
//        pos.do_move(m3);
//        nnue.do_move(m3, &pos);
//        pos.undo_move();
//        nnue.undo_move(m3, &pos);
//        pos.undo_move();
//        nnue.undo_move(m2, &pos);
//        let nnue2 = NNUEState::new(&pos);
//    }
//}
