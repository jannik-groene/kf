use crate::{
    chess::Move,
    evaluate::{Bound, eval},
};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

const ZOBRIST_MASK: u64 = 0xffff << 48;
const AGE_MASK: u64 = 0b111111;

const fn hash_index(zobrist: u64, len: u64) -> u64 {
    zobrist % len
}

pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: UnsafeCell<Vec<TTBucket>>,
    age: AtomicU8,
}

unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let len = size * 1024 * 1024 / std::mem::size_of::<TTBucket>();
        let mut hash_vec = Vec::with_capacity(len);
        hash_vec.resize_with(size, TTBucket::new);
        hash_vec.shrink_to_fit();
        TranspositionTable {
            hash: UnsafeCell::new(hash_vec),
            age: AtomicU8::new(0),
        }
    }
    pub fn increment_age(&self) {
        let _ = self
            .age
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |a| {
                Some((a + 1) & AGE_MASK as u8)
            });
    }
    pub fn hash_full(&self) -> usize {
        let count: usize = unsafe {
            (&*self.hash.get())
                .iter()
                .take(1000)
                .map(|e| {
                    e.entries
                        .iter()
                        .filter(|te| te.load(Ordering::Relaxed) != 0)
                        .count()
                })
                .sum()
        };
        count / 4
    }
    pub fn clear(&self) {
        unsafe {
            for entry in &mut *self.hash.get() {
                *entry = TTBucket::new();
            }
        }
    }
    // SAFETY: Make sure you have EXCLUSIVE access when resizing
    pub unsafe fn resize(&self, size: usize) {
        let len = size * 1024 * 1024 / std::mem::size_of::<TTBucket>();
        unsafe {
            (*self.hash.get()).truncate(0);
            (*self.hash.get()).resize_with(len, TTBucket::new);
            (*self.hash.get()).shrink_to_fit();
        }
    }
    #[inline]
    pub fn get(&self, zobrist: u64) -> Option<TTEntry> {
        let size = unsafe { (*self.hash.get()).len() };
        if size == 0 {
            return None;
        }
        let idx = hash_index(zobrist, size as u64);
        unsafe {
            (&*self.hash.get())
                .get_unchecked(idx as usize)
                .load(zobrist)
        }
    }
    #[inline]
    pub fn set(&self, zobrist: u64, eval: i32, bound: Bound, mv: Move, depth: u8, ply: usize) {
        let size = unsafe { (*self.hash.get()).len() };
        //do not commit invalid scores or low depths to the hashtable
        if size == 0 || eval.abs() >= eval::INFTY {
            return;
        }
        let age = self.age.load(Ordering::Relaxed);
        let idx = hash_index(zobrist, size as u64);
        unsafe {
            (&*self.hash.get())
                .get_unchecked(idx as usize)
                .insert(zobrist, eval, bound, mv, depth, ply, age)
        }
    }
}

struct TTBucket {
    entries: [AtomicU64; 4],
}

impl TTBucket {
    const fn new() -> Self {
        Self {
            entries: [const { AtomicU64::new(0) }; 4],
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn insert(
        &self,
        zobrist: u64,
        eval: i32,
        bound: Bound,
        mv: Move,
        depth: u8,
        ply: usize,
        age: u8,
    ) {
        let entry = TTEntry::new(eval, bound, depth, zobrist, mv, ply, age);
        // Prefer storing exact bounds, as these often come from PV nodes
        let exact = bound == Bound::Exact;
        let to_replace = self
            .entries
            .iter()
            .find(|e| (e.load(Ordering::Relaxed) ^ zobrist) & ZOBRIST_MASK == 0)
            .unwrap_or(self.worst_entry(age));

        let replace_entry = TTEntry {
            data: to_replace.load(Ordering::Relaxed),
        };
        let exact_bonus = u8::from(exact && replace_entry.bound() != Bound::Exact);
        if replace_entry.depth() >= depth + 4 + 2 * exact_bonus && age != replace_entry.age() {
            return;
        }
        to_replace.store(entry.data, Ordering::Relaxed);
    }

    fn worst_entry(&self, age: u8) -> &AtomicU64 {
        self.entries
            .iter()
            .min_by_key(|e| {
                let entry = TTEntry {
                    data: e.load(Ordering::Relaxed),
                };
                if entry == TTEntry::UNCHECKED {
                    i32::MIN
                } else {
                    let age_diff = i32::from(age.overflowing_sub(entry.age()).0 & AGE_MASK as u8);
                    let mate_bonus = 2 * i32::from(eval::is_decisive(entry.eval(0)));
                    let exact_bonus = 2 * i32::from(entry.bound() == Bound::Exact);
                    entry.depth() as i32 - 2 * age_diff + mate_bonus + exact_bonus
                }
            })
            .unwrap()
    }

    fn load(&self, zobrist: u64) -> Option<TTEntry> {
        self.entries
            .iter()
            .find(|e| {
                let data = e.load(Ordering::Relaxed);
                data != 0 && (data ^ zobrist) & ZOBRIST_MASK == 0
            })
            .map(|e| TTEntry {
                data: e.load(Ordering::Relaxed),
            })
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct TTEntry {
    data: u64,
}

impl TTEntry {
    const UNCHECKED: Self = Self { data: 0 };
    #[inline]
    pub fn mov(&self) -> Move {
        Move::decompress(((self.data >> 8) & 0xffff) as u16)
    }
    pub fn new(
        eval: i32,
        bound: Bound,
        depth: u8,
        zobrist_hash: u64,
        mov: Move,
        ply: usize,
        age: u8,
    ) -> TTEntry {
        let dam = (u64::from(mov.compress()) << 8) ^ u64::from(depth);
        let ev = eval::pack_for_tt(eval, ply);
        let bd = bound as u64;
        let data =
            (zobrist_hash & ZOBRIST_MASK) ^ (u64::from(age) << 42) ^ (bd << 40) ^ (ev << 24) ^ dam;
        TTEntry { data }
    }
    pub fn age(&self) -> u8 {
        ((self.data >> 42) & AGE_MASK) as u8
    }
    #[inline]
    pub fn eval(&self, ply: usize) -> i32 {
        eval::unpack_tt((self.data >> 24) & 0xffff, ply)
    }
    pub fn bound(&self) -> Bound {
        let bd = (self.data >> 40) & 0b11;
        if bd == 0 {
            Bound::Exact
        } else if bd == 1 {
            Bound::Upper
        } else {
            Bound::Lower
        }
    }
    #[inline]
    pub fn depth(&self) -> u8 {
        (self.data & 0xff) as u8
    }
}

#[test]
fn write_and_read_tt() {
    use crate::chess::Square;
    use std::sync::Arc;
    let hash = Arc::new(TranspositionTable::new(10));
    let mv = Move::new(Square::B1, Square::C1, crate::chess::MoveType::Normal);
    let entry = TTEntry::new(-eval::MATE_NOW, Bound::Upper, 3, 1234628935786765, mv, 5, 0);
    hash.set(1234628935786765, -eval::MATE_NOW, Bound::Upper, mv, 3, 5);
    let ret = hash.get(1234628935786765).unwrap();
    assert!(hash.get(1234628935786765).unwrap() == entry);
    assert!(ret.eval(5) == -eval::MATE_NOW);
    assert!(ret.bound() == Bound::Upper);
    assert!(ret.depth() == 3);
    assert!(ret.mov() == mv);

    let entry2 = TTEntry::new(-eval::MATE_NOW, Bound::Exact, 3, 1234628935786798, mv, 3, 0);
    hash.set(1234628935786798, -eval::MATE_NOW, Bound::Exact, mv, 3, 3);
    hash.set(
        1234628935786798,
        eval::DRAW,
        Bound::Upper,
        Move::new(Square::E1, Square::A2, crate::chess::MoveType::Normal),
        2,
        0,
    );
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = TTEntry::new(-eval::MATE_NOW, Bound::Lower, 3, 1234628935786700, mv, 8, 0);
    let hash_clone = hash.clone();
    std::thread::spawn(move || {
        hash_clone.set(1234628935786700, -eval::MATE_NOW, Bound::Lower, mv, 3, 8)
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
