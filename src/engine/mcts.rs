use std::sync::{Arc, RwLock};
use std::time::{Instant, Duration};
use rand::thread_rng;
use rand::prelude::*;

use super::chess;

struct MCSTNode {
    move_ : chess::Move,
    children: Vec<MCSTNode>,
    visits: f64,
    result: f64,
    prior: f64,
}

impl MCSTNode {
    fn policy_value(&mut self, pos: &mut chess::Position) -> f64 {
        match pos.board.piece_at(self.move_.to) {
            Some(t) => (t.value()-self.move_.piece.value()) as f64/64. + 0.5,
            None => 0.5,
        }
    }
}

struct MCSTRoot {
    children: Vec<MCSTNode>,
    visits: f64,
    result: f64,
}

trait SearchNode {
    fn evaluate(&mut self, pos: &mut chess::Position) -> f64;
}

#[inline(always)]
fn rollout(pos: &mut chess::Position) -> f64 {
    let moves = pos.get_moves().len();
    if moves == 0 && pos.in_check() {
        0.
    } else if moves == 0 {
        0.5
    } else {
        0.8/(1.+(-10.0f64.ln()*pos.material_balance() as f64/4.).exp())
    }
}

impl SearchNode for MCSTNode {
    #[inline(always)]
    fn evaluate(&mut self, pos: &mut chess::Position) -> f64 {
        pos.do_move(self.move_);
        //Initialize at the first visit
        if self.visits == 0. {
            let res = rollout(pos);
            self.prior = res;
            self.result = res;
            self.visits = 1.;
            return res;
        } else if self.visits == 1. {
            self.children = pos.get_moves()
                               .iter()
                               .map(|m| MCSTNode {
                                      move_: *m,
                                      children: Vec::new(),
                                      visits: 0.,
                                      result: 0.,
                                      prior: 0.,
                                   })
                               .collect();
        }
        //Handle the case of us being terminal
        if self.children.len() == 0 {
//            println!("Visited terminal node with result {} after move {}.", self.result/self.visits, self.move_);
            self.result += self.prior;
            self.visits += 1.;
            self.prior
        //Search using UCB
        } else {
            let res = self.children.ucb_max(self.visits, pos).unwrap().evaluate(&mut pos.clone());
            //1-res, as we have the opposite color
            self.result += 1.-res;
            self.visits += 1.;
            1.-res
        }
    }

}

impl SearchNode for MCSTRoot {
    #[inline(always)]
    fn evaluate(&mut self, pos: &mut chess::Position) -> f64 {
        //Initialize at the first visit
        if self.visits == 0. {
            let res = rollout(pos);
            self.result += res;
            self.visits += 1.;
            return res;
        }
        if self.visits == 1. {
            self.children = pos.get_moves()
                               .iter()
                               .map(|m| MCSTNode {
                                      move_: *m,
                                      children: Vec::new(),
                                      visits: 0.,
                                      result: 0.,
                                      prior: 0.,
                                   })
                               .collect();

        }
        //Handle the case of us being terminal
        if self.children.len() == 0 {
            self.result += self.result/self.visits;
            self.visits += 1.;
            self.result
        //Search using UCB
        } else {
            let res = self.children.ucb_max(self.visits, pos).unwrap().evaluate(&mut pos.clone());
            //1-res, as we have the opposite color
            self.result += 1.-res;
            self.visits += 1.;
            1.-res
        }
    }
}

#[inline(always)]
fn ucb_value(node: &mut MCSTNode, pvisits: f64, pos: &mut chess::Position) -> f64{
    //(1.-node.result/node.visits) + 0.2*node.policy_value(pos)*pvisits.sqrt()/(1.+node.visits)
    (1.-node.result/node.visits) + 0.8*(pvisits.ln()/node.visits).sqrt() + thread_rng().gen::<f64>()/(8. + node.visits)
}

trait UCBMax {
    fn ucb_max(&mut self, pvisits: f64, pos: &mut chess::Position) -> Option<&mut MCSTNode>;
    fn score_max(&self) -> Option<&MCSTNode>;
}

impl UCBMax for Vec<MCSTNode> {
    #[inline(always)]
    fn ucb_max(&mut self, pvisits: f64, pos: &mut chess::Position) -> Option<&mut MCSTNode> {
        let mut max: f64 = f64::NEG_INFINITY;
        let mut max_elem: Option<&mut MCSTNode> = None;
        for n in self.iter_mut() {
            let tmp = if n.visits == 0. { f64::INFINITY } else { ucb_value(n, pvisits, pos) };
            if tmp > max {
                max_elem = Some(n);
                max = tmp
            }
        }
        max_elem
    }
    fn score_max(&self) -> Option<&MCSTNode>  {
        let mut max: f64 = f64::NEG_INFINITY;
        let mut max_elem: Option<&MCSTNode> = None;
        for n in self.iter() {
            let tmp = if n.visits == 0. { f64::NEG_INFINITY } else { 1.-n.result/n.visits };
            if tmp > max {
                max_elem = Some(n);
                max = tmp
            }
        }
        max_elem
    }
}

pub struct MCSTree {
    base_pos: chess::Position,
    root: MCSTRoot,
}

impl MCSTree {
    pub fn new(base_pos: chess::Position) -> MCSTree {
        MCSTree {
            base_pos,
            root: MCSTRoot {
                children: Vec::new(),
                visits: 0.,
                result: 0.,
            }
        }
    }
    pub fn get_pv(&self) -> Vec<chess::Move> {
        let mut depth = 0;
        let mut n = self.root.children.score_max();
        let mut moves = Vec::new();
        while n.is_some() && depth < 10 {
            moves.push(n.unwrap().move_);
            n = n.unwrap().children.score_max();
            depth += 1;
        }
        moves
    }
    pub fn get_node_count(&self) -> u64 {
        self.root.visits as u64
    }
    pub fn best_move(&mut self) -> chess::Move {
        self.root.children.score_max().unwrap().move_
    }
    pub fn search_timed(&mut self, time: Duration) {
        let now = Instant::now();
        while now.elapsed() < time {
            self.root.evaluate(&mut self.base_pos.clone());
        }
    }
    pub fn root_eval(&self) -> f64 {
        return self.root.result / self.root.visits;
    }
    pub fn search(&mut self, stop: Arc<RwLock<bool>>, depth: usize) {
        while !*stop.read().unwrap() {
            self.root.evaluate(&mut self.base_pos.clone());
        }
    }
}
