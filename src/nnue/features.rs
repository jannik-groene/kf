use arrayvec::ArrayVec;

use crate::chess::{SquareIndex, Color};

pub trait Feature {
    fn index(&self) -> usize;
}

pub trait EnumerateFeatures<T> {
    fn features(&self, c: Color) -> ArrayVec<T, 32>;
}

pub trait MoveFeatures<T> {
    fn changed_features(&self, perspective: Color,
                        ksq: SquareIndex, our_piece: bool) -> (Vec<T>, Vec<T>);
}

