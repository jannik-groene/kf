use crate::chess::{Color, Move, Piece};

pub struct History {
    pub killer: KillerHistory,
    pub quiet: QuietHistory,
    pub continuation: ContinuationHistory,
    pub capture: CaptureHistory,
}

impl History {
    pub fn new() -> Self {
        History {
            killer: KillerHistory::new(),
            quiet: QuietHistory::new(),
            continuation: ContinuationHistory::new(),
            capture: CaptureHistory::new(),
        }
    }

    #[inline]
    pub fn beta_cutoff(&mut self, c: Color, m: Move, d: i16) {
        if m.is_capture() {
            return;
        }
        self.killer.register(m, d as usize);
        self.quiet.register(c, m, (d as i32) * 100 - 75);
    }

    #[inline]
    pub fn alpha_cutoff(
        &mut self,
        c: Color,
        pm: Move,
        pp: Option<Piece>,
        m: Move,
        p: Piece,
        d: i16,
    ) {
        self.quiet.register(c, m, 20 * d as i32 - 15);
        if pm != Move::ZERO
            && let Some(lp) = pp
        {
            self.continuation
                .register(c, lp, pm, p, m, 20 * d as i32 - 15);
        }
    }
}

#[derive(Copy, Clone)]
struct KillerBucket {
    scores: [u64; 2],
    moves: [Move; 2],
}

impl KillerBucket {
    const fn new() -> Self {
        KillerBucket {
            scores: [0; 2],
            moves: [Move::ZERO; 2],
        }
    }
}

pub struct KillerHistory {
    moves: Box<[KillerBucket; 256]>,
}

impl KillerHistory {
    pub fn new() -> Self {
        KillerHistory {
            moves: Box::new([KillerBucket::new(); 256]),
        }
    }

    pub fn register(&mut self, m: Move, d: usize) {
        if d > 255 {
            return;
        }
        // Increase score of values already contributing
        if self.moves[d].moves[0] == m {
            self.moves[d].scores[0] += 1;
        } else if self.moves[d].moves[1] == m {
            self.moves[d].scores[1] += 1;
        }
        // else replace the lower scored bucket
        else if self.moves[d].scores[0] <= self.moves[d].scores[1] {
            self.moves[d].moves[0] = m;
            self.moves[d].scores[0] = 1;
        } else {
            self.moves[d].moves[1] = m;
            self.moves[d].scores[1] = 1;
        }
    }

    pub const fn invalidate(&mut self, d: usize) {
        if d > 254 {
            return;
        }
        self.moves[d + 1].scores = [0; 2];
    }

    pub fn get(&self, d: usize) -> &[Move; 2] {
        &self.moves[d].moves
    }
}

pub struct QuietHistory {
    // scores[color][from][to]
    scores: Box<[[[i16; 64]; 64]; 2]>,
}

impl QuietHistory {
    const MAX_BONUS: i32 = (1 << 14) - 1;

    pub fn new() -> Self {
        QuietHistory {
            scores: Box::new([[[0; 64]; 64]; 2]),
        }
    }

    pub fn register(&mut self, c: Color, m: Move, bonus: i32) {
        let bonus = bonus.clamp(-Self::MAX_BONUS, Self::MAX_BONUS);
        self.scores[c as usize][m.from() as usize][m.to() as usize] += (bonus
            - self.scores[c as usize][m.from() as usize][m.to() as usize] as i32 * bonus.abs()
                / Self::MAX_BONUS)
            as i16;
    }

    pub fn get_score(&self, c: Color, m: Move) -> i32 {
        self.scores[c as usize][m.from() as usize][m.to() as usize] as i32
    }
}

pub struct ContinuationHistory {
    // scores[color][piece][to][piece][to]
    scores: Box<[[[[[i16; 64]; 6]; 64]; 6]; 2]>,
}

impl ContinuationHistory {
    const MAX_BONUS: i32 = (1 << 14) - 1;

    pub fn new() -> Self {
        ContinuationHistory {
            scores: Box::new([[[[[0; 64]; 6]; 64]; 6]; 2]),
        }
    }

    pub fn register(&mut self, c: Color, p1: Piece, m1: Move, p2: Piece, m2: Move, bonus: i32) {
        let bonus = bonus.clamp(-Self::MAX_BONUS, Self::MAX_BONUS);
        self.scores[c as usize][p1 as usize][m1.to() as usize][p2 as usize][m2.to() as usize] +=
            (bonus
                - self.scores[c as usize][p1 as usize][m1.to() as usize][p2 as usize]
                    [m2.to() as usize] as i32
                    * bonus.abs()
                    / Self::MAX_BONUS) as i16;
    }

    pub fn get_score(&self, c: Color, p1: Piece, m1: Move, p2: Piece, m2: Move) -> i32 {
        self.scores[c as usize][p1 as usize][m1.to() as usize][p2 as usize][m2.to() as usize] as i32
    }
}

pub struct CaptureHistory {
    scores: Box<[[[i16; 6]; 64]; 6]>,
}

impl CaptureHistory {
    const MAX_BONUS: i32 = (1 << 13) - 1;

    pub fn new() -> Self {
        Self {
            scores: Box::new([[[0; 6]; 64]; 6]),
        }
    }

    pub fn register(&mut self, piece: Piece, m: Move, capture: Piece, bonus: i32) {
        let bonus = bonus.clamp(-Self::MAX_BONUS, Self::MAX_BONUS);
        self.scores[piece as usize][m.to() as usize][capture as usize] += (bonus
            - self.scores[piece as usize][m.to() as usize][capture as usize] as i32 * bonus.abs()
                / Self::MAX_BONUS)
            as i16;
    }

    pub fn get(&self, p: Piece, m: Move, c: Piece) -> i32 {
        self.scores[p as usize][m.to() as usize][c as usize] as i32
    }
}
