use std::io;
use std::sync::{RwLock, Arc};

pub mod config;
pub mod engine;

#[cfg(test)]
mod tests;

fn main() {
    let conf = Arc::new(RwLock::new(config::Config::new()));
    let sin = io::stdin();
    //UCI Loop
    loop {
        let mut s = String::new();
        sin.read_line(&mut s).unwrap();
        if handle_input(s, Arc::clone(&conf)) {
            break;
        }
    }
}

fn handle_input(s: String, conf: Arc<RwLock<config::Config>>) -> bool {
    let mut tokens = s.split(r"\w");
    match tokens.next() {
        Some("uci") => parse_uci(conf),
        Some("debug") => parse_debug(),
        Some("isready") => parse_isready(conf),
        Some("setoption") => parse_setoption(),
        Some("register") => parse_register(),
        Some("ucinewgame") => parse_ucinewgame(),
        Some("position") => parse_position(),
        Some("go") => parse_go(),
        Some("stop") => parse_stop(),
        Some("parse_ponderhit") => parse_ponderhit(),
        Some("quit") => return true,
        _ => eprintln!("Did not recognize command \"{}\"", s)
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

fn parse_position() {
}

fn parse_go() {
}

fn parse_stop() {
}

fn parse_ponderhit() {
}
