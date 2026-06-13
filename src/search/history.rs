use crate::chess::{Color, Move};

pub struct History {
    pub killer: KillerHistory,
    pub quiet: QuietHistory,
}

impl History {
    pub fn new() -> Self {
        History { killer: KillerHistory::new(), quiet: QuietHistory::new() }
    }

    #[inline]
    pub fn beta_cutoff(&mut self, c: Color, m: Move, d: i16) {
        if m.is_capture() { return; }
        self.killer.register(m, d as usize);
        self.quiet.register(c, m, (d as i32)*100);
    }

    #[inline]
    pub fn alpha_cutoff(&mut self, c: Color, m: Move, d: i16) {
        self.quiet.register(c, m, 100 * d as i32);
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
    scores: Box<[[[i32; 64]; 64]; 2]>,
}

impl QuietHistory {
    const MAX_BONUS: i32 = 1 << 20;

    pub fn new() -> Self {
        QuietHistory {
            scores: Box::new([[[0; 64]; 64]; 2]),
        }
    }

    pub fn register(&mut self, c: Color, m: Move, bonus: i32) {
        self.scores[c as usize][m.from() as usize][m.to() as usize] += bonus
            - self.scores[c as usize][m.from() as usize][m.to() as usize] * bonus.abs()
                / Self::MAX_BONUS;
    }

    pub fn get_score(&self, c: Color, m: Move) -> i32 {
        self.scores[c as usize][m.from() as usize][m.to() as usize]
    }
}
