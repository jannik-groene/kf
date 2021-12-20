use std::ops::{Index, IndexMut};
use crate::chess::Color;

pub struct FeatureTransformer<const INPUTS: usize, const OUTPUTS: usize, const PSQT_BUCKETS: usize> {
    pub weights      : Box<[[i16; OUTPUTS]; INPUTS]>,
    pub biases       : [i16; OUTPUTS],
    pub psqt_weights : Box<[[i32; PSQT_BUCKETS]; INPUTS]>,
}


impl<const INPUTS: usize, const OUTPUTS: usize, const PSQT_BUCKETS: usize> FeatureTransformer<INPUTS, OUTPUTS, PSQT_BUCKETS> {
    pub fn new() -> FeatureTransformer<INPUTS,OUTPUTS,PSQT_BUCKETS> {
        FeatureTransformer {
            weights      : Box::new([[0; OUTPUTS]; INPUTS]),
            biases       : [0; OUTPUTS],
            psqt_weights : Box::new([[0; PSQT_BUCKETS]; INPUTS]),
        }
    }
}

#[derive(Clone, Copy)]
pub struct LinearLayer<const INPUTS: usize, const OUTPUTS: usize> {
    pub weights : [[i8; INPUTS]; OUTPUTS],
    pub biases  : [i32; OUTPUTS],
}

impl<const INPUTS: usize, const OUTPUTS: usize> LinearLayer<INPUTS, OUTPUTS> {
    pub fn new() -> LinearLayer<INPUTS,OUTPUTS> {
        LinearLayer {
            weights : [[0; INPUTS]; OUTPUTS],
            biases  : [0; OUTPUTS],
        }
    }
}

#[inline]
pub fn affine_transform<const INPUTS: usize, const OUTPUTS: usize>(ll: &LinearLayer<INPUTS, OUTPUTS>, inputs: &[i8; INPUTS]) -> [i32; OUTPUTS] {
    let mut outputs = ll.biases;
    #[cfg(target_arch="x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            if INPUTS % 32 == 0 && OUTPUTS % 4 == 0 {
                    return unsafe {crate::nnue::intrinsics::affine_transform(ll, inputs)};
            }
        }
    }
    for i in 0..OUTPUTS {
        for j in 0..INPUTS {
            outputs[i] +=  ll.weights[i][j] as i32 * inputs[j] as i32;
        }
    }
    outputs

}

#[derive(Clone, Copy)]
pub struct AccumulatorPerspective<const SIZE: usize, const PSQT: usize> {
    pub state: [i16; SIZE],
    pub psqt:  [i32; PSQT],
}

#[derive(Clone)]
pub struct Accumulator<const SIZE: usize, const PSQT: usize> {
    state: [AccumulatorPerspective<SIZE, PSQT>; 2],
}

impl<const SIZE: usize, const PSQT: usize> Accumulator<SIZE, PSQT> {
    pub fn new() -> Accumulator<SIZE, PSQT> {
        Accumulator { state: [AccumulatorPerspective{state: [0;SIZE], psqt: [0;PSQT]}; 2] }
    }
}

impl<const SIZE: usize, const PSQT: usize> Index<Color> for Accumulator<SIZE, PSQT> {
    type Output = AccumulatorPerspective<SIZE, PSQT>;

    fn index(&self, c: Color) -> &Self::Output {
        &self.state[c as usize]
    }
}

impl<const SIZE: usize, const PSQT: usize> IndexMut<Color> for Accumulator<SIZE, PSQT> {
    fn index_mut(&mut self, c: Color) -> &mut Self::Output {
        &mut self.state[c as usize]
    }
}

impl<const SIZE: usize, const PSQT: usize> Index<usize> for Accumulator<SIZE, PSQT> {
    type Output = AccumulatorPerspective<SIZE, PSQT>;

    fn index(&self, i: usize) -> &Self::Output {
        &self.state[i]
    }
}

impl<const SIZE: usize, const PSQT: usize> IndexMut<usize> for Accumulator<SIZE, PSQT> {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.state[i]
    }
}
