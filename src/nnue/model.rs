
//We specify the model as follows:
// $features -> number of feaures of the FeatureTransformer,
// $buckets -> number of buckets to use,
// $linear_inputs -> number of inputs of a linear layer
//                   the number of outputs is assumed to be the number of inputs of the next layer
//                   the last layer is presumed to have one output for each layerstack

#[macro_export]
macro_rules! make_model {
    ($name:ident, $features:expr, $accumulator_size:expr, $buckets:expr, $( ($layer_name:ident, $linear_inputs:expr, $linear_outputs:expr) ),*)
        => {
            mod $name {
                use lazy_static::lazy_static;
                use arrayvec::ArrayVec;
                use std::{
                    sync::{Arc, RwLock},
                    io::{BufReader, Read},
                    fs::File,
                };
                use crate::nnue::{
                    layers::{FeatureTransformer, LinearLayer, Accumulator},
                    features::Feature,
                };

                //function to calculate the input padding produced by nnue-pytorch
                const fn input_padding(layer_size: usize) -> usize {
                    if layer_size % 32 != 0 {
                        32 - (layer_size % 32)
                    } else {
                        0
                    }
                }

                #[derive(Clone,Copy)]
                struct LayerStack {
                    $($layer_name: LinearLayer<$linear_inputs, $linear_outputs>,)*
                }

                impl LayerStack {
                    fn new() -> LayerStack {
                        LayerStack {
                            $($layer_name: LinearLayer::new(),)*
                        }
                    }
                }

                struct ModelType {
                    ft          : FeatureTransformer<$features, $accumulator_size, $buckets>,
                    layerstacks : Box<[LayerStack; $buckets]>,
                }

                impl ModelType {
                    pub fn new() -> ModelType {
                        ModelType {
                            ft          : FeatureTransformer::new(),
                            layerstacks : Box::new([LayerStack::new(); $buckets]),
                        }
                    }
                }

                lazy_static! [
                    static ref MODEL: Arc<RwLock<ModelType>> = {
                        Arc::new(RwLock::new(ModelType::new()))
                    };
                ];

                #[allow(dead_code)]
                pub fn update_accumulator<T: Feature>(old_accumulator  : &Accumulator<$accumulator_size,$buckets>,
                                                      new_accumulator  : &mut Accumulator<$accumulator_size,$buckets>,
                                                      added_features   : Vec<T>,
                                                      removed_features : Vec<T>,
                                                      perspective      : usize                                    ) {
                    let data = MODEL.read().unwrap();
                    new_accumulator[perspective] = old_accumulator[perspective];
                    for feature in added_features {
                        for (acc, weight) in new_accumulator[perspective].state.iter_mut().zip(data.ft.weights[feature.index()].iter()) {
                            *acc += *weight;
                        }
                        for (acc, weight) in new_accumulator[perspective].psqt.iter_mut().zip(data.ft.psqt_weights[feature.index()].iter()) {
                            *acc += *weight;
                        }
                    }
                    for feature in removed_features {
                        for (acc, weight) in new_accumulator[perspective].state.iter_mut().zip(data.ft.weights[feature.index()].iter()) {
                            *acc -= *weight;
                        }
                        for (acc, weight) in new_accumulator[perspective].psqt.iter_mut().zip(data.ft.psqt_weights[feature.index()].iter()) {
                            *acc -= *weight;
                        }
                    }
                }

                #[allow(dead_code)]
                pub fn refresh_accumulator<T: Feature> (accumulator     : &mut Accumulator<$accumulator_size,$buckets>,
                                                        features        : ArrayVec<T, 32>,
                                                        perspective     : usize                                        ) {
                    let data = MODEL.read().unwrap();
                    for (acc, bias) in accumulator[perspective].state.iter_mut().zip(data.ft.biases.iter()) {
                        *acc = *bias;
                    }
                    for acc in accumulator[perspective].psqt.iter_mut() {
                        *acc = 0;
                    }
                    for feature in features {
                        for (acc, weight) in accumulator[perspective].state.iter_mut().zip(data.ft.weights[feature.index()].iter()) {
                            *acc += *weight;
                        }
                        for (acc, weight) in accumulator[perspective].psqt.iter_mut().zip(data.ft.psqt_weights[feature.index()].iter()) {
                            *acc += *weight;
                        }
                    }
                }

                trait Clip {
                    type Output;

                    fn clip(&self, perspective: usize) -> Self::Output;
                }

                impl Clip for Accumulator<$accumulator_size,$buckets> {
                    type Output = [i8; 2 * $accumulator_size];

                    #[inline]
                    fn clip(&self, perspective: usize) -> Self::Output {
                        let other_perspective = if perspective == 0 {1} else {0};
                        let mut clipped = [0; 2 * $accumulator_size];
                        for (c, v) in clipped[..$accumulator_size].iter_mut().zip(self[perspective].state.iter()) {
                            *c = (*v).clamp(0,127) as i8;
                        }
                        for (c, v) in clipped[$accumulator_size..].iter_mut().zip(self[other_perspective].state.iter()) {
                            *c = (*v).clamp(0,127) as i8;
                        }
                        clipped
                    }
                }

                impl<const SIZE: usize> Clip for [i32; SIZE] {
                    type Output = [i8; SIZE];

                    #[inline]
                    fn clip(&self, _: usize) -> Self::Output {
                        let mut clipped = [0; SIZE];
                        for (c,v) in clipped.iter_mut().zip(self.iter()) {
                            *c = (*v / 64).clamp(0,127) as i8;
                        }
                        clipped
                    }
                }

                #[inline]
                fn affine_trafo<const INPUTS: usize, const OUTPUTS: usize>(ll: &LinearLayer<INPUTS,OUTPUTS>,
                                                                           inputs: &[i8; INPUTS]) -> [i32; OUTPUTS] {
                    let mut outputs = ll.biases;
                    for i in 0..OUTPUTS {
                        for j in 0..INPUTS {
                            outputs[i] += inputs[j] as i32 * ll.weights[i][j] as i32;
                        }
                    }
                    outputs
                }

                #[allow(dead_code)]
                pub fn evaluate_state(accumulator : &Accumulator<$accumulator_size,$buckets>,
                                      bucket      : usize                                   ,
                                      perspective : usize                                    ) -> i32 {
                    let data = MODEL.read().unwrap();
                    let other_perspective = if perspective == 0 {1} else {0};
                    let psqt = (accumulator[perspective].psqt[bucket] - accumulator[other_perspective].psqt[bucket]) / 2;
                    let state = accumulator;
                    $(
                        let input = state.clip(perspective);
                        let state = affine_trafo(&data.layerstacks[bucket].$layer_name, &input);
                    )*
                    (state[0] + psqt) / 16
                }

                #[allow(dead_code)]
                pub fn load_model(path: &std::path::Path) -> std::io::Result<()> {
                    let file = File::open(path)?;
                    let mut buf = BufReader::new(file);
                    load_header(&mut buf)?;
                    load_ft(&mut buf)?;
                    for i in 0..$buckets {
                        load_layerstack(&mut buf, i)?;
                    }
                    Ok(())
                }

                fn load_header(reader: &mut BufReader<File>) -> std::io::Result<()>{
                    let mut buf_i32 = [0_u8; 4];
                    //Read version number
                    reader.read_exact(&mut buf_i32)?;
                    let version = i32::from_le_bytes(buf_i32);
                    //Read hash
                    reader.read_exact(&mut buf_i32)?;
                    let hash = i32::from_le_bytes(buf_i32);
                    //Read description length
                    reader.read_exact(&mut buf_i32)?;
                    let len = u32::from_le_bytes(buf_i32) as usize;
                    //Read the description
                    let mut description_buffer = vec![0_u8; len];
                    reader.read_exact(&mut description_buffer)?;
                    let description = String::from_utf8(description_buffer);
                    println!("Reading network version {} with hash 0x{:x}:\n{}", version, hash, description.unwrap());
                    Ok(())
                }

                fn load_ft(reader: &mut BufReader<File>) -> std::io::Result<()> {
                    let mut data = MODEL.write().unwrap();
                    let mut buf_i16 = [0_u8; 2];
                    let mut buf_i32 = [0_u8; 4];
                    //Read the FT hash
                    reader.read_exact(&mut buf_i32)?;
                    //Read the biases
                    for bias in data.ft.biases.iter_mut() {
                        reader.read_exact(&mut buf_i16)?;
                        *bias = i16::from_le_bytes(buf_i16);
                    }
                    //Read the weights
                    for weight_vec in data.ft.weights.iter_mut() {
                        for weight in weight_vec.iter_mut() {
                            reader.read_exact(&mut buf_i16)?;
                            *weight = i16::from_le_bytes(buf_i16);
                        }
                    }
                    //Read PSQT weights
                    for weight_vec in data.ft.psqt_weights.iter_mut() {
                        for weight in weight_vec.iter_mut() {
                            reader.read_exact(&mut buf_i32)?;
                            *weight = i32::from_le_bytes(buf_i32);
                        }
                    }
                                        Ok(())
                }

                fn load_layerstack(reader: &mut BufReader<File>, bucket: usize) -> std::io::Result<()> {
                    let mut data = MODEL.write().unwrap();
                    let mut buf_i8 = [0_u8; 1];
                    let mut buf_i32 = [0_u8; 4];
                    //Read hash
                    reader.read_exact(&mut buf_i32)?;
                    $(
                    //Read biases
                    for bias in data.layerstacks[bucket].$layer_name.biases.iter_mut() {
                        reader.read_exact(&mut buf_i32)?;
                        *bias = i32::from_le_bytes(buf_i32);
                    }
                    //Read the weight
                    for weight_vec in data.layerstacks[bucket].$layer_name.weights.iter_mut() {
                        for weight in weight_vec.iter_mut() {
                            reader.read_exact(&mut buf_i8)?;
                            *weight = i8::from_le_bytes(buf_i8);
                        }
                        //Discard padding added by nnue-pytorch
                        reader.seek_relative(input_padding($linear_inputs) as i64)?;
                    }
                    )*
                    Ok(())
                }
            }
            pub fn load_model(path: &std::path::Path) -> std::io::Result<()> {
                $name::load_model(path)
            }
        }
}

#[cfg(test)]
mod tests {
    make_model!{sf_half_ka_v2, {64*64*11}, 512, 8,
                (l1, 1024, 16),
                (l2,   16, 32),
                (l3,   32,  1)}
    use crate::{
        chess::{Position, Color},
        nnue::features::EnumerateFeatures,
        nnue::layers::Accumulator,
    };
    #[test]
    fn build_model() {
        load_model(&std::path::Path::new("/home/jannik/Downloads/Stockfish/src/nn-33c9d39e5eb6.nnue")).unwrap();
        let pos = Position::from_fen(String::from("r2r4/p1p1kppp/2p2n2/5b2/2B5/P1P2N2/R1P2PPP/2B1K2R b K - 5 13")).unwrap();
        //let pos = Position::new();
        let bucket = (pos.board.occupation.count_ones() - 1) / 4;
        let features_w = pos.features(Color::WHITE);
        let features_b = pos.features(Color::BLACK);
        let mut accumulator = Accumulator::new();
        sf_half_ka_v2::refresh_accumulator(&mut accumulator, features_w, 0);
        sf_half_ka_v2::refresh_accumulator(&mut accumulator, features_b, 1);
        let eval = sf_half_ka_v2::evaluate_state(&accumulator,bucket as usize,pos.color() as usize);
        let eval2 = sf_half_ka_v2::evaluate_state(&accumulator,bucket as usize,pos.color().other() as usize);
        println!("Eval {:.2}", eval as f64 / 208.);
        println!("Eval {:.2}", eval2 as f64 / 208.);
    }
    #[test]
    fn load_bytes() -> std::io::Result<()> {
        use std::io::Read;
        let target = [[1_i16,2],[3,4],[5,6]];

        let file = std::fs::File::open(std::path::Path::new("/home/jannik/Code/kf/test.bin"))?;
        let mut buf = std::io::BufReader::new(file);
        let mut i16_buf = [0_u8; 2];
        let mut read = [[0_i16; 2]; 3];
        for i in 0..3 {
            for j in 0..2 {
                buf.read_exact(&mut i16_buf)?;
                read[i][j] = i16::from_le_bytes(i16_buf);
            }
        }
        assert!(target == read);
        Ok(())
    }
}
