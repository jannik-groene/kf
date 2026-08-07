use crate::chess::{Color, Move, MoveType, Piece, Position, Square};

pub static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../../f3f2fd88.nnue")) };

pub fn evaluate_position(pos: &Position) -> i32 {
    let accs = [
        Accumulator::from_pos(pos, &NNUE, Color::White),
        Accumulator::from_pos(pos, &NNUE, Color::Black),
    ];
    NNUE.evaluate(
        &accs[pos.color() as usize],
        &accs[pos.color().other() as usize],
        pos.get_board().occupation().count() as usize,
    )
}

const OUT_BUCKETS: usize = 4;
const BUCKET_SIZE: usize = 32 / OUT_BUCKETS;
const HIDDEN_SIZE: usize = 128;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

#[inline]
/// Square Clipped ReLU - Activation Function.
/// Note that this takes the i16s in the accumulator to i32s.
/// Range is 0.0 .. 1.0 (in other words, 0 to QA*QA quantized).
fn screlu(x: i16) -> i32 {
    let y = i32::from(x).clamp(0, i32::from(QA));
    y * y
}

/// This is the quantised format that bullet outputs.
#[repr(C, align(64))]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    /// Values have quantization of QA.
    feature_weights: [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    /// Values have quantization of QA.
    feature_bias: Accumulator,
    /// Column-Major `1 x (2 * HIDDEN_SIZE)`
    /// matrix, we use it like this to make the
    /// code nicer in `Network::evaluate`.
    /// Values have quantization of QB.
    output_weights: [[i16; 2 * HIDDEN_SIZE]; OUT_BUCKETS],
    /// Scalar output bias.
    /// Value has quantization of QA * QB.
    output_bias: [i16; OUT_BUCKETS],
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, popcnt: usize) -> i32 {
        // Initialise output.
        let mut output = 0;

        let bucket = (popcnt - 1) / BUCKET_SIZE;

        // Side-To-Move Accumulator -> Output.
        for (&input, &weight) in us
            .vals
            .iter()
            .zip(&self.output_weights[bucket][..HIDDEN_SIZE])
        {
            output += screlu(input) * i32::from(weight);
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (&input, &weight) in them
            .vals
            .iter()
            .zip(&self.output_weights[bucket][HIDDEN_SIZE..])
        {
            output += screlu(input) * i32::from(weight);
        }

        // Reduce quantization from QA * QA * QB to QA * QB.
        output /= i32::from(QA);

        // Add bias.
        output += i32::from(self.output_bias[bucket]);

        // Apply eval scale.
        output *= SCALE;

        // Remove quantisation altogether.
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
}

fn feature_index(p: Piece, sq: Square, us: bool) -> usize {
    (p as usize + 6 * (!us) as usize) * 64 + sq as usize
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub fn new(net: &Network) -> Self {
        net.feature_bias
    }

    pub fn from_pos(pos: &Position, net: &Network, perspective: Color) -> Self {
        let mut acc = Accumulator::new(net);
        const PIECES: [Piece; 6] = [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ];

        for p in PIECES.iter() {
            for sq in pos.get_board().get_bb(perspective, *p) {
                acc.add_feature(feature_index(*p, sq.relative(perspective), true), net);
            }
            for sq in pos.get_board().get_bb(perspective.other(), *p) {
                acc.add_feature(feature_index(*p, sq.relative(perspective), false), net);
            }
        }
        acc
    }

    pub fn do_move(&mut self, m: Move, pos: &Position, perspective: Color, net: &Network) {
        let changes = change_set(m, pos, perspective);
        for c in changes {
            match c {
                ChangedFeature::Removed(idx) => self.remove_feature(idx, net),
                ChangedFeature::Added(idx) => self.add_feature(idx, net),
                ChangedFeature::Nothing => {}
            }
        }
    }

    // Move should already be undone in the Position!
    pub fn undo_move(&mut self, m: Move, pos: &Position, perspective: Color, net: &Network) {
        let changes = change_set(m, pos, perspective);
        for c in changes {
            match c {
                ChangedFeature::Removed(idx) => self.add_feature(idx, net),
                ChangedFeature::Added(idx) => self.remove_feature(idx, net),
                ChangedFeature::Nothing => {}
            }
        }
    }

    /// Add a feature to an accumulator.
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i += *d
        }
    }

    /// Remove a feature from an accumulator.
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i -= *d
        }
    }
}

enum ChangedFeature {
    Added(usize),
    Removed(usize),
    Nothing,
}

fn change_set(m: Move, pos: &Position, perspective: Color) -> [ChangedFeature; 4] {
    let us = pos.color() == perspective;
    let to = m.to().relative(perspective);
    let from = m.from().relative(perspective);
    match m.typ() {
        MoveType::Normal => {
            let mover = pos.get_board().piece_at(m.from()).unwrap();
            [
                ChangedFeature::Removed(feature_index(mover, from, us)),
                ChangedFeature::Added(feature_index(mover, to, us)),
                ChangedFeature::Nothing,
                ChangedFeature::Nothing,
            ]
        }
        MoveType::Capture => {
            let mover = pos.get_board().piece_at(m.from()).unwrap();
            let cap = pos.get_board().piece_at(m.to()).unwrap();
            [
                ChangedFeature::Removed(feature_index(mover, from, us)),
                ChangedFeature::Removed(feature_index(cap, to, !us)),
                ChangedFeature::Added(feature_index(mover, to, us)),
                ChangedFeature::Nothing,
            ]
        }
        MoveType::Enpassant => {
            let ep_sq = m
                .to()
                .file()
                .ep_cap_square()
                .relative(pos.color().other())
                .relative(perspective);
            [
                ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
                ChangedFeature::Removed(feature_index(Piece::Pawn, ep_sq, !us)),
                ChangedFeature::Added(feature_index(Piece::Pawn, to, us)),
                ChangedFeature::Nothing,
            ]
        }
        MoveType::Castle => {
            let f0 = ChangedFeature::Removed(feature_index(Piece::King, from, us));
            let f1 = ChangedFeature::Added(feature_index(Piece::King, to, us));
            if m.to() == Square::C1 {
                [
                    f0,
                    f1,
                    ChangedFeature::Removed(feature_index(
                        Piece::Rook,
                        Square::A1.relative(perspective),
                        us,
                    )),
                    ChangedFeature::Added(feature_index(
                        Piece::Rook,
                        Square::D1.relative(perspective),
                        us,
                    )),
                ]
            } else if m.to() == Square::G1 {
                [
                    f0,
                    f1,
                    ChangedFeature::Removed(feature_index(
                        Piece::Rook,
                        Square::H1.relative(perspective),
                        us,
                    )),
                    ChangedFeature::Added(feature_index(
                        Piece::Rook,
                        Square::F1.relative(perspective),
                        us,
                    )),
                ]
            } else if m.to() == Square::C8 {
                [
                    f0,
                    f1,
                    ChangedFeature::Removed(feature_index(
                        Piece::Rook,
                        Square::A8.relative(perspective),
                        us,
                    )),
                    ChangedFeature::Added(feature_index(
                        Piece::Rook,
                        Square::D8.relative(perspective),
                        us,
                    )),
                ]
            } else {
                [
                    f0,
                    f1,
                    ChangedFeature::Removed(feature_index(
                        Piece::Rook,
                        Square::H8.relative(perspective),
                        us,
                    )),
                    ChangedFeature::Added(feature_index(
                        Piece::Rook,
                        Square::F8.relative(perspective),
                        us,
                    )),
                ]
            }
        }
        MoveType::PromotionN => [
            ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
            ChangedFeature::Added(feature_index(Piece::Knight, to, us)),
            ChangedFeature::Nothing,
            ChangedFeature::Nothing,
        ],
        MoveType::PromotionB => [
            ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
            ChangedFeature::Added(feature_index(Piece::Bishop, to, us)),
            ChangedFeature::Nothing,
            ChangedFeature::Nothing,
        ],
        MoveType::PromotionR => [
            ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
            ChangedFeature::Added(feature_index(Piece::Rook, to, us)),
            ChangedFeature::Nothing,
            ChangedFeature::Nothing,
        ],
        MoveType::PromotionQ => [
            ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
            ChangedFeature::Added(feature_index(Piece::Queen, to, us)),
            ChangedFeature::Nothing,
            ChangedFeature::Nothing,
        ],
        MoveType::PromotionCaptureN => {
            let cap = pos.get_board().piece_at(m.to()).unwrap();
            let fc = ChangedFeature::Removed(feature_index(cap, to, !us));
            [
                ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
                ChangedFeature::Added(feature_index(Piece::Knight, to, us)),
                fc,
                ChangedFeature::Nothing,
            ]
        }
        MoveType::PromotionCaptureB => {
            let cap = pos.get_board().piece_at(m.to()).unwrap();
            let fc = ChangedFeature::Removed(feature_index(cap, to, !us));
            [
                ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
                ChangedFeature::Added(feature_index(Piece::Bishop, to, us)),
                fc,
                ChangedFeature::Nothing,
            ]
        }
        MoveType::PromotionCaptureR => {
            let cap = pos.get_board().piece_at(m.to()).unwrap();
            let fc = ChangedFeature::Removed(feature_index(cap, to, !us));
            [
                ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
                ChangedFeature::Added(feature_index(Piece::Rook, to, us)),
                fc,
                ChangedFeature::Nothing,
            ]
        }
        MoveType::PromotionCaptureQ => {
            let cap = pos.get_board().piece_at(m.to()).unwrap();
            let fc = ChangedFeature::Removed(feature_index(cap, to, !us));
            [
                ChangedFeature::Removed(feature_index(Piece::Pawn, from, us)),
                ChangedFeature::Added(feature_index(Piece::Queen, to, us)),
                fc,
                ChangedFeature::Nothing,
            ]
        }
    }
}

#[test]
fn nnue_startpos() {
    use crate::chess::{Move, MoveType};
    let mut pos = Position::new();
    let mut accs_incremental = [
        Accumulator::from_pos(&pos, &NNUE, Color::White),
        Accumulator::from_pos(&pos, &NNUE, Color::Black),
    ];
    assert_eq!(accs_incremental[0], accs_incremental[1]);
    println!("{:?}", accs_incremental);
    let moves = [
        Move::new(Square::E2, Square::E4, MoveType::Normal),
        Move::new(Square::E7, Square::E5, MoveType::Normal),
        Move::new(Square::D2, Square::D3, MoveType::Normal),
        Move::new(Square::B8, Square::C6, MoveType::Normal),
        Move::new(Square::G1, Square::F3, MoveType::Normal),
        Move::new(Square::G8, Square::F6, MoveType::Normal),
        Move::new(Square::G2, Square::F3, MoveType::Normal),
        Move::new(Square::D7, Square::D5, MoveType::Normal),
    ];

    for m in moves {
        accs_incremental[0].do_move(m, &pos, Color::White, &NNUE);
        accs_incremental[1].do_move(m, &pos, Color::Black, &NNUE);
        pos.do_move(m);
    }

    let accs = [
        Accumulator::from_pos(&pos, &NNUE, Color::White),
        Accumulator::from_pos(&pos, &NNUE, Color::Black),
    ];

    println!(
        "{}",
        NNUE.evaluate(
            &accs[pos.color() as usize],
            &accs[pos.color().other() as usize],
            pos.get_board().occupation().count() as usize,
        )
    );
    assert_eq!(accs, accs_incremental);
}
