use crate::{
    chess::Move,
    evaluate::{Bound, Eval, Value},
};
use std::cell::UnsafeCell;

pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: UnsafeCell<Vec<(TTEntry, TTEntry)>>,
    size: usize,
}

unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let mut hash_vec = vec![(TTEntry::UNCHECKED, TTEntry::UNCHECKED); size];
        hash_vec.shrink_to_fit();
        TranspositionTable {
            size,
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
    #[inline]
    pub fn get(&self, zobrist_key: u64) -> Option<TTEntry> {
        if self.size == 0 {
            return None;
        }
        let entry = unsafe { (&*self.hash.get())[zobrist_key as usize % self.size] };
        if entry.0.zobrist_hash ^ entry.0.data == zobrist_key && entry.0.depth() != 0 {
            Some(entry.0)
        } else if entry.1.zobrist_hash ^ entry.1.data == zobrist_key && entry.1.depth() != 0 {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline]
    pub fn set(&self, zobrist_key: u64, entry: TTEntry) {
        //do not commit invalid scores or low depths to the hashtable
        if self.size == 0 || matches!(entry.eval().value(), Value::Infty | Value::NegInfty) {
            return;
        }
        let hash_entry = unsafe { (&*self.hash.get())[zobrist_key as usize % self.size] };
        //Mate scores may be seen as having infinite depth
        if (hash_entry.0.depth() < entry.depth()
            || (matches!(entry.eval().value(), Value::Mate(_))
                && entry.eval() > hash_entry.0.eval()))
            && (hash_entry.0.depth() + 2 < entry.depth()
                || entry.eval().bound() == Bound::Exact
                || hash_entry.0.eval().bound() != Bound::Exact)
        {
            unsafe {
                (&mut *self.hash.get())[zobrist_key as usize % self.size].0 = entry;
            }
        } else {
            unsafe {
                (&mut *self.hash.get())[zobrist_key as usize % self.size].1 = entry;
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
    pub fn mov(&self) -> Option<Move> {
        Move::decompress(((self.data >> 8) & 0xffff) as u16)
    }
    pub fn new(eval: Eval, depth: u8, zobrist_hash: u64, mov: Move) -> TTEntry {
        let dam = ((mov.compress() as u32) << 8) ^ (depth as u32);
        let ev = eval.pack_for_tt();
        let data = (ev << 24) ^ (dam as u64);
        TTEntry {
            zobrist_hash: zobrist_hash ^ data,
            data,
        }
    }
    #[inline]
    pub fn eval(&self) -> Eval {
        Eval::from_packed(self.data >> 24)
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
    let entry = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786765, mv);
    hash.set(1234628935786765, entry);
    let ret = hash.get(1234628935786765).unwrap();
    assert!(hash.get(1234628935786765).unwrap() == entry);
    assert!(ret.eval() == -Eval::MATE_NOW);
    assert!(ret.depth() == 3);
    assert!(ret.mov().unwrap() == mv);

    let entry2 = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786798, mv);
    let entry4 = TTEntry::new(
        Eval::DRAW,
        2,
        1234628935786798,
        Move::new(Square::E1, Square::A2, crate::chess::MoveType::Normal),
    );
    hash.set(1234628935786798, entry2);
    hash.set(1234628935786798, entry4);
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786700, mv);
    let hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set(1234628935786700, entry3));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
