use std::sync::{Arc, RwLock};
use super::super::evaluate::eval::{Eval, Value};
use super::super::chess::{Move, CompressedMove};

#[derive(Clone)]
pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: Arc<RwLock<Vec<(TTEntry, TTEntry)>>>,
    size: usize,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let mut hash_vec = vec![(TTEntry::UNCHECKED, TTEntry::UNCHECKED); size];
        hash_vec.shrink_to_fit();
        TranspositionTable {
            size,
            hash: Arc::new(RwLock::new(hash_vec)),
        }
    }
    #[inline(always)]
    pub fn get(&self, zobrist_key: u64) -> Option<TTEntry> {
        let entry = self.hash.read().unwrap()[zobrist_key as usize % self.size];
        if entry.0.depth != 0 && entry.0.zobrist_hash == zobrist_key {
            Some(entry.0)
        } else if entry.1.depth != 0 && entry.1.zobrist_hash == zobrist_key {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline(always)]
    pub fn set(&mut self, zobrist_key: u64, entry: TTEntry, pv: bool) {
        //do not commit invalid scores or low depths to the hashtable
        if matches!(entry.eval.value(), Value::INFTY | Value::NEGINFTY) {
            return;
        }
        let mut hash = self.hash.write().unwrap();
        //Mate scores may be seen as having infinite depth
        if hash[zobrist_key as usize % self.size].0.depth < entry.depth
            || (matches!(entry.eval.value(), Value::MATE(_))
                  && entry.eval > hash[zobrist_key as usize % self.size].0.eval)
            || pv {
            hash.get_mut(zobrist_key as usize % self.size).unwrap().0 = entry;
        } else {
            hash.get_mut(zobrist_key as usize % self.size).unwrap().1 = entry;
        }
    }
    pub fn reset(&mut self) {
        self.hash = Arc::new(RwLock::new(Vec::new()));
        self.hash = Arc::new(RwLock::new(vec![(TTEntry::UNCHECKED, TTEntry::UNCHECKED); self.size]));
        self.hash.write().unwrap().shrink_to_fit();
    }
}

#[derive(Clone,PartialEq,Copy)]
pub struct TTEntry {
    eval: Eval,
    depth: u8,
    zobrist_hash: u64,
    mov: CompressedMove,
}

impl TTEntry {
    const UNCHECKED: TTEntry = TTEntry {eval: Eval::MIN, depth: 0, zobrist_hash: 0, mov: CompressedMove{to:0, from:0, piece_and_type:0}};
    #[inline(always)]
    pub fn mov(&self) -> Option<Move> {
        self.mov.decompress()
    }
    pub fn new(eval: Eval, depth: u8, zobrist_hash: u64, mov: Move) -> TTEntry {
        TTEntry {
            eval,
            depth,
            zobrist_hash,
            mov: mov.compress(),
        }
    }
    #[inline(always)]
    pub fn eval(&self) -> Eval {
        self.eval
    }
    #[inline(always)]
    pub fn depth(&self) -> u8 {
        self.depth
    }
}

#[test]
fn write_and_read_tt() {
    let mut hash = TranspositionTable::new(10000);
    let entry = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786765, Move{from: 1, to: 2, piece: super::super::chess::Piece::KING, typ: super::super::chess::MoveType::MOVE});
    hash.set(1234628935786765, entry.clone(), false);
    assert!(hash.get(1234628935786765).unwrap() == entry);
    let entry2 = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786798, Move{from: 1, to: 2, piece: super::super::chess::Piece::KING, typ: super::super::chess::MoveType::MOVE});
    let entry4 = TTEntry::new(Eval::DRAW, 2, 1234628935786798, Move{from: 4, to: 8, piece: super::super::chess::Piece::QUEEN, typ: super::super::chess::MoveType::MOVE});
    hash.set(1234628935786798, entry2.clone(), false);
    hash.set(1234628935786798, entry4.clone(), false);
    assert!(hash.get(1234628935786798).unwrap() == entry2);
    let entry3 = TTEntry::new(-Eval::MATE_NOW, 3, 1234628935786700, Move{from: 1, to: 2, piece: super::super::chess::Piece::KING, typ: super::super::chess::MoveType::MOVE});
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set(1234628935786700, entry3.clone(), false));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
