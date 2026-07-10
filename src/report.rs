use crate::chess::Move;
use crate::evaluate::{Bound, eval};

pub trait ResultReport {
    const REPORT: bool = false;

    #[allow(clippy::too_many_arguments)]
    fn report_update(
        &self,
        _eval: i32,
        _bound: Bound,
        _depth: usize,
        _seldepth: usize,
        _nodes: u64,
        _time: u64,
        _hashfull: usize,
        _pv: [Move; 256],
    ) {
    }
    fn report_result(&self, _eval: i32, _mv: Move) {}
}

pub trait Reporter: ResultReport + Clone + Send + 'static {}
impl<T> Reporter for T where T: ResultReport + Clone + Send + 'static {}

pub fn print_eval(val: i32, bound: Bound) {
    let (type_str, val) = if val.abs() >= eval::INFTY || val.abs() < eval::MATE_IN_MAX {
        ("score cp", val)
    } else {
        let m = val.signum() * eval::MATE_NOW - val;
        ("score mate", (m / 2) + (m % 2))
    };
    let bound_str = match bound {
        Bound::Upper => "upperbound ",
        Bound::Lower => "lowerbound ",
        Bound::Exact => "",
    };
    print!("{} {} {}", type_str, val, bound_str);
}

#[derive(Copy, Clone, Default)]
pub struct StdOutUCIResult {}

unsafe impl Send for StdOutUCIResult {}

impl ResultReport for StdOutUCIResult {
    const REPORT: bool = true;

    fn report_update(
        &self,
        eval: i32,
        bound: Bound,
        depth: usize,
        seldepth: usize,
        nodes: u64,
        time: u64,
        hashfull: usize,
        pv: [Move; 256],
    ) {
        print!("info depth {depth} seldepth {seldepth} ");
        print_eval(eval, bound);
        let nps = nodes * 1000 / time.max(1);
        print!("nodes {nodes} nps {nps} time {time} hashfull {hashfull}");
        if pv[0] != Move::ZERO {
            print!(" pv");
            for m in &pv {
                if *m == Move::ZERO {
                    break;
                }
                print!(" {m}");
            }
        }
        println!();
    }

    fn report_result(&self, _eval: i32, mv: Move) {
        if mv != Move::ZERO {
            println!("bestmove {mv}");
        } else {
            println!("bestmove 0000");
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct NullReport {}

unsafe impl Send for NullReport {}

impl ResultReport for NullReport {}
