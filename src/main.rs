use std::io;
use std::sync::{RwLock, Arc};
use std::env;
use std::time::Duration;
use std::fmt::Display;

pub mod config;
pub mod engine;

#[cfg(test)]
mod tests;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        uci_loop();
    }
    if args[1] == "af" {
        let sin = io::stdin();
        loop {
            let mut comm = String::new();
            sin.read_line(&mut comm).unwrap();
            if comm == "quit" {
                break;
            }
            let pos = engine::chess::Position::from_fen(comm).unwrap();
            let mut stree = engine::mcts::MCSTree::new(pos);
            stree.search_timed(Duration::from_secs(5));
            println!("Eval {}\nBest Move {}", ((stree.root_eval()-0.5)*3.1).tan()*2.9, stree.best_move());
            print!("PV: ");
            for m in stree.get_pv() {
                print!("{}, ", m);
            }
            print!("\n");
            println!("Visited {} nodes ({} nps)", stree.get_node_count(), stree.get_node_count()/7);
        }
    }
}

fn uci_loop() {
    let conf = Arc::new(RwLock::new(config::Config::new()));
    let sin = io::stdin();
    let mut search = engine::alphabeta::ABSearch::new(engine::chess::Position::new(),6);
    //UCI Loop
    let mut pos: engine::chess::Position = engine::chess::Position::new();
    loop {
        let mut s = String::new();
        sin.read_line(&mut s).unwrap();
        if handle_input(s, Arc::clone(&conf), &mut pos, &mut search) {
            break;
        }
    }
}

fn handle_input(s: String, conf: Arc<RwLock<config::Config>>, pos: &mut engine::chess::Position, search: &mut engine::alphabeta::ABSearch) -> bool {
    let mut tokens:Vec<&str> = s.trim().split_whitespace().collect();
    match tokens[0] {
        "uci" => parse_uci(conf),
        "debug" => parse_debug(),
        "isready" => parse_isready(conf),
        "setoption" => parse_setoption(),
        "register" => parse_register(),
        "ucinewgame" => parse_ucinewgame(),
        "position" => parse_position(pos, tokens, search),
        "go" => parse_go(pos, tokens, search),
        "stop" => parse_stop(),
        "parse_ponderhit" => parse_ponderhit(),
        "quit" => std::process::exit(0),
        _ => eprintln!("Did not recognize command \"{}\"", tokens[0])
    }
    false
}

fn parse_uci(conf: Arc<RwLock<config::Config>>) {
    match conf.write() {
        Ok(mut c) => {
            if c.is_initialized {return;}
            c.is_initialized = true;
            println!("uciok")
        }
        _ => {}
    }
}

fn parse_debug() {
}

fn parse_isready(conf: Arc<RwLock<config::Config>>) {
    while !conf.read().unwrap().is_ready {}
    println!("readyok");
}

fn parse_setoption() {
}

fn parse_register() {
}

fn parse_ucinewgame() {
}

fn parse_position(pos: &mut engine::chess::Position, tokens: Vec<&str>, search: &mut engine::alphabeta::ABSearch) {
    let mut offset = 0;
    if tokens[1] == "startpos" {
        *pos = engine::chess::Position::new();
        offset = 2;
    } else if tokens[1] == "fen" {
        let fen = tokens[2..=7].join(" ");
        *pos = engine::chess::Position::from_fen(fen).unwrap();
        offset = 8;
    }
    if tokens.len() > offset && tokens[offset] == "moves" {
        for m in tokens[offset+1..].iter() {
            pos.do_move(engine::chess::Move::from_str(m, pos));
        }
    }
    search.set_position(pos.clone());
}

fn parse_go(pos: &mut engine::chess::Position, tokens: Vec<&str>, search: &mut engine::alphabeta::ABSearch) {
    //let mut stree = engine::mcts::MCSTree::new(pos.clone());
    //let mut dur = Duration::from_secs(1);
    //if tokens[0] == "movetime" {
    //    dur = Duration::from_millis(tokens[1].parse().unwrap());
    //}
    //stree.search_timed(dur);
    //println!("info score cp {} nodes {} nps {}", (((stree.root_eval()-0.5)*3.1).tan()*290.) as i64, stree.get_node_count(), stree.get_node_count()/dur.as_secs());
    //println!("info pv {}", stree.get_pv().iter().map(|m| format!("{}", *m)).collect::<Vec<_>>().join(" "));
    //println!("bestmove {}", stree.best_move());
    let (s,m) = search.search(6);
    println!("info {}", s);
    if m.is_some() {
        println!("bestmove {}", m.unwrap());
    }
}

fn parse_stop() {
}

fn parse_ponderhit() {
}
