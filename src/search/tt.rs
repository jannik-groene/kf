use crate::{
    chess::Move,
    evaluate::{Bound, Eval, Value},
};
use std::sync::Arc;
use std::cell::UnsafeCell;

#[derive(Clone)]
pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: Arc<UnsafeCell<Vec<(TTEntry, TTEntry)>>>,
    size: usize,
}

unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let mut hash_vec = Vec::with_capacity(size);
        for _ in 0..size {
            hash_vec.push((TTEntry::UNCHECKED, TTEntry::UNCHECKED));
        }
        hash_vec.shrink_to_fit();
        TranspositionTable {
            size,
            hash: Arc::new(UnsafeCell::new(hash_vec)),
        }
    }
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }
    #[inline]
    pub fn get(&self, zobrist_key: u64) -> Option<TTEntry> {
        if self.size == 0 {
            return None;
        }
        let entry = unsafe { (& *self.hash.get())[zobrist_key as usize % self.size] };
        if entry.0.zobrist_hash ^ (entry.0.depth_and_move as u64) ^ entry.0.eval == zobrist_key && entry.0.depth() != 0 {
            Some(entry.0)
        } else if entry.1.zobrist_hash ^ (entry.1.depth_and_move as u64) ^ entry.1.eval == zobrist_key && entry.1.depth() != 0 {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline]
    pub fn set(&mut self, zobrist_key: u64, entry: TTEntry) {
        //do not commit invalid scores or low depths to the hashtable
        if self.size == 0 || matches!(entry.eval().value(), Value::Infty | Value::NegInfty) {
            return;
        }
        let hash_entry = unsafe { (& *self.hash.get())[zobrist_key as usize % self.size] };
        //Mate scores may be seen as having infinite depth
        if (hash_entry.0.depth() < entry.depth()
            || (matches!(entry.eval().value(), Value::Mate(_)) && entry.eval() > hash_entry.0.eval()))
            && (hash_entry.0.depth() + 2 < entry.depth()
                || entry.eval().bound() == Bound::Exact
                || hash_entry.0.eval().bound() != Bound::Exact)
        {
            unsafe { (&mut *self.hash.get())[zobrist_key as usize % self.size].0 = entry; }
        } else {
            unsafe { (&mut *self.hash.get())[zobrist_key as usize % self.size].1 = entry; }
        }
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct TTEntry {
    zobrist_hash: u64,
    eval: u64,
    depth_and_move: u32,
}

impl TTEntry {
    const UNCHECKED: TTEntry = TTEntry {
        zobrist_hash: 0,
        eval: 0,
        depth_and_move: 0,
    };
    #[inline]
    pub fn mov(&self) -> Option<Move> {
        Move::decompress((self.depth_and_move >> 8) as u16)
    }
    pub fn new(eval: Eval, depth: u8, zobrist_hash: u64, mov: Move) -> TTEntry {
        let dam = ((mov.compress() as u32) << 8) ^ (depth as u32);
        let ev = eval.pack_for_tt();
        TTEntry {
            zobrist_hash: zobrist_hash ^ dam as u64 ^ ev,
            depth_and_move: dam,
            eval: ev,
        }
    }
    #[inline]
    pub fn eval(&self) -> Eval {
        Eval::from_packed(self.eval)
    }
    #[inline]
    pub fn depth(&self) -> u8 {
        (self.depth_and_move & 0xff) as u8
    }
}

#[test]
fn write_and_read_tt() {
    use crate::chess::Square;
    let mut hash = TranspositionTable::new(10000);
    let entry = TTEntry::new(
        -Eval::MATE_NOW,
        3,
        1234628935786765,
        Move {
            from: Square::B1,
            to: Square::C1,
            typ: crate::chess::MoveType::Normal,
        },
    );
    hash.set(1234628935786765, entry);
    assert!(hash.get(1234628935786765).unwrap() == entry);
    let entry2 = TTEntry::new(
        -Eval::MATE_NOW,
        3,
        1234628935786798,
        Move {
            from: Square::B1,
            to: Square::C1,
            typ: crate::chess::MoveType::Normal,
        },
    );
    let entry4 = TTEntry::new(
        Eval::DRAW,
        2,
        1234628935786798,
        Move {
            from: Square::E1,
            to: Square::A2,
            typ: crate::chess::MoveType::Normal,
        },
    );
    hash.set(1234628935786798, entry2);
    hash.set(1234628935786798, entry4);
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = TTEntry::new(
        -Eval::MATE_NOW,
        3,
        1234628935786700,
        Move {
            from: Square::B1,
            to: Square::C1,
            typ: crate::chess::MoveType::Normal,
        },
    );
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set(1234628935786700, entry3));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
