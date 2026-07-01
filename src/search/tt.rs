use crate::{
    chess::Move,
    evaluate::{Bound, eval},
};
use std::cell::UnsafeCell;

pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: UnsafeCell<Vec<(TTEntry, TTEntry)>>,
}

unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let mut hash_vec = vec![(TTEntry::UNCHECKED, TTEntry::UNCHECKED); size];
        hash_vec.shrink_to_fit();
        TranspositionTable {
            hash: UnsafeCell::new(hash_vec),
        }
    }
    pub fn clear(&self) {
        unsafe {
            for entry in &mut *self.hash.get() {
                *entry = (TTEntry::UNCHECKED, TTEntry::UNCHECKED);
            }
        }
    }
    // SAFETY: Make sure you have EXCLUSIVE access when resizing
    pub unsafe fn resize(&self, size: usize) {
        unsafe {
            *self.hash.get() = vec![(TTEntry::UNCHECKED, TTEntry::UNCHECKED); size];
        }
    }
    #[inline]
    pub fn get(&self, zobrist_key: u64) -> Option<TTEntry> {
        let size = unsafe { (*self.hash.get()).len() };
        if size == 0 {
            return None;
        }
        let entry = unsafe { (&*self.hash.get())[zobrist_key as usize % size] };
        if entry.0.zobrist_hash ^ entry.0.data == zobrist_key && entry.0.depth() != 0 {
            Some(entry.0)
        } else if entry.1.zobrist_hash ^ entry.1.data == zobrist_key && entry.1.depth() != 0 {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline]
    pub fn set(&self, zobrist_key: u64, eval: i32, bound: Bound, mv: Move, depth: u8, ply: usize) {
        let size = unsafe { (*self.hash.get()).len() };
        //do not commit invalid scores or low depths to the hashtable
        if size == 0 || eval.abs() >= eval::INFTY {
            return;
        }
        let hash_entry = unsafe { (&*self.hash.get())[zobrist_key as usize % size] };
        let entry = TTEntry::new(eval, bound, depth, zobrist_key, mv, ply);
        //Mate scores may be seen as having infinite depth
        if hash_entry.0 == TTEntry::UNCHECKED
            || ((hash_entry.0.depth() < depth
                || (eval::is_decisive(eval) && eval > hash_entry.0.eval(ply)))
                && (hash_entry.0.depth() + 2 < depth
                    || bound == Bound::Exact
                    || hash_entry.0.bound() != Bound::Exact))
        {
            unsafe {
                (&mut *self.hash.get())[zobrist_key as usize % size].0 = entry;
            }
        } else {
            unsafe {
                (&mut *self.hash.get())[zobrist_key as usize % size].1 = entry;
            }
        }
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct TTEntry {
    zobrist_hash: u64,
    data: u64,
}

impl TTEntry {
    const UNCHECKED: TTEntry = TTEntry {
        zobrist_hash: 0,
        data: 0,
    };
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
    ) -> TTEntry {
        let dam = (u64::from(mov.compress()) << 8) ^ u64::from(depth);
        let ev = eval::pack_for_tt(eval, ply);
        let bd = bound as u64;
        let data = (bd << 40) ^ (ev << 24) ^ dam;
        TTEntry {
            zobrist_hash: zobrist_hash ^ data,
            data,
        }
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
    let hash = Arc::new(TranspositionTable::new(10000));
    let mv = Move::new(Square::B1, Square::C1, crate::chess::MoveType::Normal);
    let entry = TTEntry::new(-eval::MATE_NOW, Bound::Exact, 3, 1234628935786765, mv, 5);
    hash.set(1234628935786765, -eval::MATE_NOW, Bound::Exact, mv, 3, 5);
    let ret = hash.get(1234628935786765).unwrap();
    assert!(hash.get(1234628935786765).unwrap() == entry);
    assert!(ret.eval(5) == -eval::MATE_NOW);
    assert!(ret.bound() == Bound::Exact);
    assert!(ret.depth() == 3);
    assert!(ret.mov() == mv);

    let entry2 = TTEntry::new(-eval::MATE_NOW, Bound::Upper, 3, 1234628935786798, mv, 3);
    hash.set(1234628935786798, -eval::MATE_NOW, Bound::Upper, mv, 3, 3);
    hash.set(
        1234628935786798,
        eval::DRAW,
        Bound::Exact,
        Move::new(Square::E1, Square::A2, crate::chess::MoveType::Normal),
        2,
        0,
    );
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = TTEntry::new(-eval::MATE_NOW, Bound::Lower, 3, 1234628935786700, mv, 8);
    let hash_clone = hash.clone();
    std::thread::spawn(move || {
        hash_clone.set(1234628935786700, -eval::MATE_NOW, Bound::Lower, mv, 3, 8)
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
