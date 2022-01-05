use crate::chess::Color;
use bitintr::Pext;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Index, Not};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BitBoard {
    bits: u64,
}

impl BitAnd for BitBoard {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        BitBoard {
            bits: self.bits & rhs.bits,
        }
    }
}

impl BitAndAssign for BitBoard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.bits &= rhs.bits;
    }
}

impl BitOr for BitBoard {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        BitBoard {
            bits: self.bits | rhs.bits,
        }
    }
}

impl BitOrAssign for BitBoard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl BitXor for BitBoard {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        BitBoard {
            bits: self.bits ^ rhs.bits,
        }
    }
}

impl BitXorAssign for BitBoard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.bits ^= rhs.bits;
    }
}

impl Not for BitBoard {
    type Output = Self;
    #[inline]
    fn not(self) -> Self::Output {
        BitBoard { bits: !self.bits }
    }
}

impl From<Square> for BitBoard {
    #[inline]
    fn from(sq: Square) -> Self {
        BitBoard {
            bits: 1 << u8::from(sq),
        }
    }
}

impl From<u64> for BitBoard {
    #[inline]
    fn from(bits: u64) -> Self {
        BitBoard { bits }
    }
}

impl BitBoard {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const FULL: Self = Self { bits: u64::MAX };
    #[inline]
    pub const fn new(bits: u64) -> BitBoard {
        BitBoard { bits }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }
    #[inline]
    pub fn least_square(&self) -> Square {
        self.bits.trailing_zeros().into()
    }
    #[inline]
    pub fn count(&self) -> u32 {
        self.bits.count_ones()
    }
    #[inline]
    pub fn set(&mut self, sq: Square) {
        *self |= sq.into()
    }
    #[inline]
    pub fn unset(&mut self, sq: Square) {
        *self &= !BitBoard::from(sq);
    }
    #[inline]
    pub fn is_set(&self, sq: Square) -> bool {
        *self & sq.into() != Self::EMPTY
    }
    #[inline]
    pub fn from_file(file: File) -> Self {
        Self::new(0b100000001000000010000000100000001000000010000000100000001 << file as u8)
    }
    #[inline]
    pub fn from_rank(rank: Rank) -> Self {
        Self::new(0b11111111 << (8 * rank as u8))
    }
    #[inline]
    pub fn shifted_forward(&self, c: Color) -> Self {
        BitBoard {
            bits: match c {
                Color::White => self.bits << 8,
                Color::Black => self.bits >> 8,
            },
        }
    }
    #[inline]
    pub fn shifted_by(&self, i: i32) -> Self {
        if i >= 0 {
            BitBoard {
                bits: self.bits << i,
            }
        } else {
            BitBoard {
                bits: self.bits >> i.abs(),
            }
        }
    }
    #[inline]
    pub fn forward_of(s: Square, c: Color) -> Self {
        let offset = (s.relative(c).rank() as u8 + 1) * 8;
        if offset >= 64 {
            return BitBoard::EMPTY;
        }
        match c {
            Color::White => BitBoard {
                bits: u64::MAX << offset,
            },
            Color::Black => BitBoard {
                bits: u64::MAX >> offset,
            },
        }
    }
    #[inline]
    pub fn pext(self, mask: u64) -> usize {
        self.bits.pext(mask) as usize
    }
}

pub struct BitBoardIterator {
    board: BitBoard,
}

impl Iterator for BitBoardIterator {
    type Item = Square;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.board == BitBoard::EMPTY {
            None
        } else {
            let sq = self.board.least_square();
            self.board.bits &= self.board.bits.wrapping_sub(1);
            Some(sq)
        }
    }
}

impl IntoIterator for BitBoard {
    type Item = Square;
    type IntoIter = BitBoardIterator;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        BitBoardIterator { board: self }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Square {
    A1,
    B1,
    C1,
    D1,
    E1,
    F1,
    G1,
    H1,
    A2,
    B2,
    C2,
    D2,
    E2,
    F2,
    G2,
    H2,
    A3,
    B3,
    C3,
    D3,
    E3,
    F3,
    G3,
    H3,
    A4,
    B4,
    C4,
    D4,
    E4,
    F4,
    G4,
    H4,
    A5,
    B5,
    C5,
    D5,
    E5,
    F5,
    G5,
    H5,
    A6,
    B6,
    C6,
    D6,
    E6,
    F6,
    G6,
    H6,
    A7,
    B7,
    C7,
    D7,
    E7,
    F7,
    G7,
    H7,
    A8,
    B8,
    C8,
    D8,
    E8,
    F8,
    G8,
    H8,
}

macro_rules! from_square {
    ($($T:ty)*) => {
        $(
        impl From<Square> for $T {
            #[inline]
            fn from(sq: Square) -> Self {
                sq as $T
            }
        }
        )*
    }
}

macro_rules! from_type_for_square {
    ($($T:ty)*) => {
        $(
        impl From<$T> for Square {
            #[inline]
            fn from(t: $T) -> Self {
                assert!(t < 64);
                unsafe {std::mem::transmute(t as u8)}
            }
        }
        )*
    }
}

from_type_for_square!(u8 u16 u32 u64 usize);
from_square!(u8 u16 u32 u64 usize);

impl Square {
    pub fn new<T: Into<u8>>(t: T) -> Square {
        let sq = t.into() as u8;
        assert!(sq < 64);
        unsafe { std::mem::transmute(sq) }
    }
    pub fn from_string(s: &str) -> Self {
        let mut bytes = s.bytes();
        let idx = (bytes.next().unwrap() - b'a') + 8 * (bytes.next().unwrap() - b'1');
        Self::new(idx)
    }
    #[inline]
    pub fn file(&self) -> File {
        File::new::<u8>(<Self as Into<u8>>::into(*self) % 8)
    }
    #[inline]
    pub fn rank(&self) -> Rank {
        Rank::new::<u8>(<Self as Into<u8>>::into(*self) / 8)
    }
    #[inline]
    pub fn flipped(&self) -> Self {
        (u8::from(*self) ^ 56).into()
    }
    #[inline]
    pub fn relative(self, c: Color) -> Self {
        match c {
            Color::White => self,
            Color::Black => self.flipped(),
        }
    }
    #[inline]
    pub fn shifted_by(&self, shift: i8) -> Self {
        if shift >= 0 {
            Self::new(*self as u8 + shift as u8)
        } else {
            Self::new(*self as u8 - shift.abs() as u8)
        }
    }
    #[inline]
    pub fn advance(&self, c: Color) -> Self {
        match c {
            Color::White => self.shifted_by(8),
            Color::Black => self.shifted_by(-8),
        }
    }
}

const FILES: [char; 8] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
const RANKS: [char; 8] = ['1', '2', '3', '4', '5', '6', '7', '8'];

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            FILES[self.file() as usize],
            RANKS[self.rank() as usize]
        )
    }
}

impl<T, const N: usize> Index<Square> for [T; N] {
    type Output = T;
    fn index(&self, index: Square) -> &Self::Output {
        &self[index as usize]
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl File {
    pub fn new<T: Into<u8>>(t: T) -> File {
        let f = t.into() as u8;
        assert!(f < 8);
        unsafe { std::mem::transmute(f) }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rank {
    First,
    Second,
    Third,
    Fourth,
    Fifth,
    Sixth,
    Seventh,
    Eighth,
}

impl Rank {
    pub fn new<T: Into<u8>>(t: T) -> Rank {
        let r = t.into() as u8;
        assert!(r < 8);
        unsafe { std::mem::transmute(r) }
    }
    pub fn relative(&self, c: Color) -> Rank {
        match c {
            Color::White => *self,
            Color::Black => Self::new(7 - *self as u8),
        }
    }
}
