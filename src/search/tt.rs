use crate::{
    chess::{CompressedMove, Move},
    evaluate::{Bound, Eval, Value},
};
use std::sync::Arc;

use parking_lot::{Mutex, const_mutex};

#[derive(Clone)]
pub struct TranspositionTable {
    //We run with two buckets. One replace on depth, on always replace
    hash: Arc<Vec<Mutex<(TTEntry, TTEntry)>>>,
    size: usize,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let mut hash_vec = Vec::with_capacity(size);
        for _ in 0..size {
            hash_vec.push(const_mutex((TTEntry::UNCHECKED, TTEntry::UNCHECKED)));
        }
        hash_vec.shrink_to_fit();
        TranspositionTable {
            size,
            hash: Arc::new(hash_vec),
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
        let entry = self.hash[zobrist_key as usize % self.size].lock();
        if entry.0.depth != 0 && entry.0.zobrist_hash == zobrist_key {
            Some(entry.0)
        } else if entry.1.depth != 0 && entry.1.zobrist_hash == zobrist_key {
            Some(entry.1)
        } else {
            None
        }
    }
    #[inline]
    pub fn set(&mut self, zobrist_key: u64, entry: TTEntry) {
        //do not commit invalid scores or low depths to the hashtable
        if self.size == 0 || matches!(entry.eval.value(), Value::Infty | Value::NegInfty) {
            return;
        }
        let mut hash_entry = self.hash[zobrist_key as usize % self.size].lock();
        //Mate scores may be seen as having infinite depth
        if (hash_entry.0.depth < entry.depth
            || (matches!(entry.eval.value(), Value::Mate(_)) && entry.eval > hash_entry.0.eval))
            && (hash_entry.0.depth + 5 < entry.depth
                || entry.eval.bound() == Bound::Exact
                || hash_entry.0.eval().bound() != Bound::Exact)
        {
            hash_entry.0 = entry;
        } else {
            hash_entry.1 = entry;
        }
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct TTEntry {
    eval: Eval,
    depth: u8,
    zobrist_hash: u64,
    mov: CompressedMove,
}

impl TTEntry {
    const UNCHECKED: TTEntry = TTEntry {
        eval: Eval::MIN,
        depth: 0,
        zobrist_hash: 0,
        mov: CompressedMove {
            to: 0,
            from: 0,
            piece_and_type: 0,
        },
    };
    #[inline]
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
    #[inline]
    pub fn eval(&self) -> Eval {
        self.eval
    }
    #[inline]
    pub fn depth(&self) -> u8 {
        self.depth
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
            piece: crate::chess::Piece::King,
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
            piece: crate::chess::Piece::King,
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
            piece: crate::chess::Piece::Queen,
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
            piece: crate::chess::Piece::King,
            typ: crate::chess::MoveType::Normal,
        },
    );
    let mut hash_clone = hash.clone();
    std::thread::spawn(move || hash_clone.set(1234628935786700, entry3));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(hash.get(1234628935786700).unwrap() == entry3);
}
