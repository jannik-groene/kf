pub const INFTY: i32 = 32001;
pub const MATE_NOW: i32 = 32000;
pub const MATE_IN_MAX: i32 = 32000 - 255;
pub const DRAW: i32 = 0;

#[allow(dead_code)]
pub const fn draw(seed: u64) -> i32 {
    (seed % 5) as i32 - 2
}

pub const fn mate_in(ply: usize) -> i32 {
    MATE_NOW - ply as i32
}

#[allow(dead_code)]
pub const fn is_win(val: i32) -> bool {
    val >= MATE_IN_MAX
}

#[allow(dead_code)]
pub const fn is_loss(val: i32) -> bool {
    val <= -MATE_IN_MAX
}

pub const fn is_decisive(val: i32) -> bool {
    val.abs() >= MATE_IN_MAX
}

#[allow(dead_code)]
pub const fn is_mate(val: i32) -> bool {
    val.abs() >= MATE_IN_MAX && val.abs() < INFTY
}

pub fn aspiration_lower(val: i32, fails: usize) -> i32 {
    const ASPIRATION_ADJUSTMENTS: [i32; 5] = [25, 50, 200, 400, 800];

    if fails < 5 {
        (val - ASPIRATION_ADJUSTMENTS[fails]).clamp(-INFTY, INFTY)
    } else {
        -INFTY
    }
}

pub fn aspiration_higher(val: i32, fails: usize) -> i32 {
    const ASPIRATION_ADJUSTMENTS: [i32; 5] = [25, 50, 200, 400, 800];

    if fails < 5 {
        (val + ASPIRATION_ADJUSTMENTS[fails]).clamp(-INFTY, INFTY)
    } else {
        INFTY
    }
}

pub fn pack_for_tt(mut val: i32, ply: usize) -> u64 {
    if is_decisive(val) {
        val += val.signum() * (ply as i32);
    }
    (val.clamp(i16::MIN as i32, i16::MAX as i32) as i16).cast_unsigned() as u64
}

pub fn unpack_tt(val: u64, ply: usize) -> i32 {
    let mut eval = (val as i16) as i32;
    if is_decisive(eval) {
        eval -= eval.signum() * (ply as i32);
    }
    eval
}

#[derive(Clone, PartialEq, Copy, Eq)]
pub enum Bound {
    Exact,
    Upper,
    Lower,
}
