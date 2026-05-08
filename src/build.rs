use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use std::arch::x86_64::{_pext_u64,_pdep_u64};
use rand::{self, Rng, SeedableRng};

fn write_pawn_attacks(f: &mut File) {
    writeln!(f, "const PAWN_ATTACKS: [[u64; 64]; 2] = [[").unwrap();
    for sq in 0..64 {
        let mut attacks = 0u64;
        if (8..56).contains(&sq) && sq % 8 != 0 {
            attacks |= 1 << (sq + 7);
        }
        if (8..56).contains(&sq) && sq % 8 != 7 {
            attacks |= 1 << (sq + 9);
        }
        write!(f, "0x{:x},", attacks).unwrap();
    }
    writeln!(f, "],\n[").unwrap();
    for sq in 0..64 {
        let mut attacks = 0u64;
        if (8..56).contains(&sq) && sq % 8 != 7 {
            attacks |= 1 << (sq - 7);
        }
        if (8..56).contains(&sq) && sq % 8 != 0 {
            attacks |= 1 << (sq - 9);
        }
        write!(f, "0x{:x},", attacks).unwrap();
    }
    writeln!(f, "\n]];").unwrap();
}

fn write_knight_attacks(f: &mut File) {
    let moves = [6, 15, 17, 10, -10, -17, -15, -6];

    writeln!(f, "const KNIGHT_ATTACKS: [u64; 64] = [").unwrap();
    for sq in 0i32..64 {
        let mut attacks = 0u64;
        for m in moves {
            if (0..64).contains(&(sq + m)) && ((sq + m) % 8 - sq % 8).abs() <= 2 {
                attacks |= 1 << (sq + m);
            }
        }
        write!(f, "0x{:x},", attacks).unwrap();
    }
    writeln!(f, "\n];").unwrap();
}

fn write_neighbours(f: &mut File) {
    let moves = [-1, 7, 8, 9, 1, -9, -8, -7];
    writeln!(f, "const NEIGHBOURS: [u64; 64] = [").unwrap();
    for sq in 0i32..64 {
        let mut attacks = 0u64;
        for m in moves {
            if (0..64).contains(&(sq + m)) && ((sq + m) % 8 - sq % 8).abs() <= 1 {
                attacks |= 1 << (sq + m);
            }
        }
        write!(f, "0x{:x},", attacks).unwrap();
    }
    writeln!(f, "\n];").unwrap();

    let moves = [
        -2, 6, 14, 15, 16, 17, 18, 10, 2, -10, -18, -17, -16, -15, -14, -6,
    ];
    writeln!(f, "const NEXT_NEIGHBOURS: [u64; 64] = [").unwrap();
    for sq in 0i32..64 {
        let mut attacks = 0u64;
        for m in moves {
            if (0..64).contains(&(sq + m)) && ((sq + m) % 8 - sq % 8).abs() <= 2 {
                attacks |= 1 << (sq + m);
            }
        }
        write!(f, "0x{:x},", attacks).unwrap();
    }
    writeln!(f, "\n];").unwrap();
}

const BORDER: u64 =
    0b1111_1111_1000_0001_1000_0001_1000_0001_1000_0001_1000_0001_1000_0001_1111_1111;

fn go_in_direction(mut base: i32, dir: i32, blockers: u64) -> u64 {
    let mut attacks = 0;
    while (0..64).contains(&(base + dir))
        && ((base + dir) % 8 - base % 8).abs() <= 1
        && (1 << base) & blockers == 0
    {
        attacks |= 1 << (base + dir);
        base += dir;
    }
    attacks
}


fn write_bishop_attacks(f: &mut File) {
    let dirs = [7, 9, -7, -9];

    let mut masks = Vec::new();
    writeln!(f, "const BISHOP_MASKS: [u64; 64] = [").unwrap();
    for sq in 0..64 {
        let mut mask = 0;
        for d in dirs {
            mask |= go_in_direction(sq, d, 0) & !BORDER;
        }
        write!(f, "0x{:x},", mask).unwrap();
        masks.push(mask);
    }
    writeln!(f, "\n];").unwrap();

    let mut offsets = vec![0];
    writeln!(f, "const BISHOP_ATTACKS: [u64; 5248] = [").unwrap();
    for (sq, m) in masks.iter().enumerate() {
        #[cfg(target_feature="bmi2")] {
            let max = unsafe { _pext_u64(u64::MAX, *m) };
            for i in 0..=max {
                let mut attacks = 0;
                let blockers = unsafe { _pdep_u64(i, *m) };
                for d in dirs {
                    attacks |= go_in_direction(sq as i32, d, blockers);
                }
                write!(f, "0x{:x},", attacks).unwrap();
            }
            writeln!(f).unwrap();
            offsets.push(offsets.last().unwrap_or(&0) + max + 1);
        }
    }
    writeln!(f, "\n];").unwrap();

    offsets.pop();
    writeln!(f, "const BISHOP_ATTACK_OFFSETS: [usize; 64] = [").unwrap();
    for offset in offsets {
        write!(f, "0x{:x},", offset).unwrap();
    }
    writeln!(f, "\n];").unwrap();
}

fn write_rook_attacks(f: &mut File) {
    let dirs = [-1, 8, 1, -8];
    let borders: [u64; 4] = [0x0101010101010101, 0xff << 56, 0x8080808080808080, 0xff];

    let mut masks = Vec::new();
    writeln!(f, "const ROOK_MASKS: [u64; 64] = [").unwrap();
    for sq in 0..64 {
        let mut mask = 0;
        for (d, b) in dirs.iter().zip(borders) {
            mask |= go_in_direction(sq, *d, 0) & !b;
        }
        write!(f, "0x{:x},", mask).unwrap();
        masks.push(mask);
    }
    writeln!(f, "\n];").unwrap();

    let mut offsets = vec![0];
    writeln!(f, "static ROOK_ATTACKS: [u64; 102400] = [").unwrap();
    for (sq, m) in masks.iter().enumerate() {
        #[cfg(target_feature="bmi2")] {
            let max = unsafe { _pext_u64(u64::MAX, *m) };
            for i in 0..=max {
                let mut attacks = 0;
                let blockers = unsafe { _pdep_u64(i, *m) };
                for d in dirs {
                    attacks |= go_in_direction(sq as i32, d, blockers);
                }
                write!(f, "0x{:x},", attacks).unwrap();
            }
            writeln!(f).unwrap();
            offsets.push(offsets.last().unwrap_or(&0) + max + 1);
        }
    }
    writeln!(f, "\n];").unwrap();

    offsets.pop();
    writeln!(f, "const ROOK_ATTACK_OFFSETS: [usize; 64] = [").unwrap();
    for offset in offsets {
        write!(f, "0x{:x},", offset).unwrap();
    }
    writeln!(f, "\n];").unwrap();
}

fn write_rays(f: &mut File) {
    writeln!(f, "const RAYS: [[u64; 64]; 64] = [").unwrap();
    for a in 0..64 {
        writeln!(f, "[").unwrap();
        for b in 0..64 {
            let (xa, ya) = (a % 8, a / 8);
            let (xb, yb) = (b % 8, b / 8);
            let ray = if a == b {
                0
            } else if xa == xb {
                (1 << a) | go_in_direction(a, 8, 0) | go_in_direction(a, -8, 0)
            } else if ya == yb {
                (1 << a) | go_in_direction(a, 1, 0) | go_in_direction(a, -1, 0)
            } else if xa - xb == ya - yb {
                //diagonal
                (1 << a) | go_in_direction(a, 9, 0) | go_in_direction(a, -9, 0)
            } else if xa - xb == yb - ya {
                //anti-diagonal
                (1 << a) | go_in_direction(a, 7, 0) | go_in_direction(a, -7, 0)
            } else {
                0
            };
            write!(f, "0x{:x},", ray).unwrap();
        }
        writeln!(f, "],\n").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

fn write_connecting_rays(f: &mut File) {
    writeln!(f, "const CONNECTING_RAYS: [[u64; 64]; 64] = [").unwrap();
    for a in 0..64 {
        writeln!(f, "[").unwrap();
        for b in 0..64 {
            let from = i32::min(a, b);
            let to = i32::max(a, b);
            let ray = if from == to {
                0
            } else if from % 8 == to % 8 {
                go_in_direction(from, 8, 1 << (to - 8))
            } else if from / 8 == to / 8 {
                go_in_direction(from, 1, 1 << (to - 1))
            } else if from % 8 - to % 8 == from / 8 - to / 8 {
                //diagonal
                go_in_direction(from, 9, 1 << (to - 9))
            } else if from % 8 - to % 8 == to / 8 - from / 8 {
                //anti-diagonal
                go_in_direction(from, 7, 1 << (to - 7))
            } else {
                0
            };
            write!(f, "0x{:x},", ray).unwrap();
        }
        writeln!(f, "],\n").unwrap();
    }
    writeln!(f, "];\n").unwrap();
}

fn write_zobrist_numbers(f: &mut File) {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(31415926);

    for piece in ["KING", "QUEEN", "BISHOP", "KNIGHT", "ROOK", "PAWN"] {
        for color in ["WHITE", "BLACK"] {
            writeln!(f, "const {}_{}_ZOBRIST: [u64; 64] = [", color, piece).unwrap();
            for _ in 0..64 {
                write!(f, "0x{:x},", rng.gen::<u64>()).unwrap();
            }
            writeln!(f, "];\n").unwrap();
        }
    }

    writeln!(f, "const CASTLING_ZOBRIST: [u64; 4] = [").unwrap();
    for _ in 0..4 {
        write!(f, "0x{:x},", rng.gen::<u64>()).unwrap();
    }
    writeln!(f, "\n];").unwrap();

    writeln!(f, "const ENPASSANT_ZOBRIST: [u64; 8] = [").unwrap();
    for _ in 0..8 {
        write!(f, "0x{:x},", rng.gen::<u64>()).unwrap();
    }
    writeln!(f, "\n];").unwrap();

    writeln!(f, "const COLOR_ZOBRIST: u64 = 0x{:x};", rng.gen::<u64>()).unwrap();
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    let mut f = File::create(&dest_path).unwrap();

    write_pawn_attacks(&mut f);
    write_knight_attacks(&mut f);
    write_neighbours(&mut f);
    write_bishop_attacks(&mut f);
    write_rook_attacks(&mut f);

    write_rays(&mut f);
    write_connecting_rays(&mut f);

    write_zobrist_numbers(&mut f);
}
