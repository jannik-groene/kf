use crate::{
    chess::{Color, Move, Position},
    engine::{Engine, EngineIO},
    report::Reporter,
    search::SearchLimit,
};
use std::sync::mpsc::Sender;

const VERSION_STRING: &str = "kf-0.1.1-0cf4cf94";

fn read_input(ch: &Sender<EngineIO>) {
    let sin = std::io::stdin();
    loop {
        let mut s = String::new();
        sin.read_line(&mut s).unwrap();
        match ch.send(EngineIO::UciInput(s)) {
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

struct TimeSpec {
    ustime: Option<u64>,
    usinc: Option<u64>,
    movestogo: Option<u64>,
    movetime: Option<u64>,
}

//Timer for execution. To achieve high precision we first sleep the thread to within a safetymargin
//of the target and the spin until we reach the target time.
fn time_limit(time: &TimeSpec) -> Option<SearchLimit> {
    const SAFETY_DELTA: std::time::Duration = std::time::Duration::from_millis(2);
    //We play with increment
    if let (Some(time), Some(inc)) = (time.ustime, time.usinc) {
        let max_time = std::time::Duration::from_millis(time);
        // The target is the hard bound; soft bound will be checked in the iid loop
        let mut target = std::time::Duration::from_millis(inc * 4 / 5) + max_time / 4;
        target = target.min(max_time);
        Some(SearchLimit::from_soft_hard_bound(
            target / 6,
            target,
            SAFETY_DELTA,
        ))
    }
    //We play with time per x moves, we split evenly and spend a little more earlier
    else if let (Some(time), Some(togo)) = (time.ustime, time.movestogo) {
        let duration = std::time::Duration::from_millis(time);
        if togo == 1 {
            Some(SearchLimit::from_soft_hard_bound(
                duration / 2,
                duration,
                SAFETY_DELTA,
            ))
        } else {
            Some(SearchLimit::from_soft_hard_bound(
                duration * 2 / (3 * togo as u32),
                duration / 2,
                SAFETY_DELTA,
            ))
        }
    }
    //sudden death
    else if let Some(time) = time.ustime {
        //we simply spend 4% of our time in each move
        Some(SearchLimit::Time {
            soft: std::time::Duration::from_micros(time * 40),
            hard: std::time::Duration::from_millis(time) / 3,
        })
    }
    //We play with fixed time per move
    else {
        time.movetime.map(|time| {
            SearchLimit::from_movetime(std::time::Duration::from_millis(time), SAFETY_DELTA)
        })
    }
}

pub trait UCIHandler {
    fn uci_loop(&mut self);
    fn handle_input(&mut self, s: String);
    fn handle_uci(&self);
    fn handle_debug(&mut self, tokens: Vec<&str>);
    fn handle_isready(&self);
    fn handle_setoption(&mut self, tokens: Vec<&str>);
    fn handle_register(&self);
    fn handle_ucinewgame(&mut self);
    fn handle_position(&mut self, tokens: Vec<&str>);
    fn handle_go(&mut self, tokens: Vec<&str>);
    fn handle_stop(&mut self);
    fn handle_ponderhit(&self);
    fn handle_perft(&self, tokens: Vec<&str>);
    fn handle_eval(&self);
}

impl<T: Reporter> UCIHandler for Engine<T> {
    fn uci_loop(&mut self) {
        let tx = self.get_sender();
        std::thread::spawn(move || read_input(&tx));
        loop {
            if let Ok(io) = self.receiver().recv() {
                match io {
                    EngineIO::UciInput(s) => self.handle_input(s),
                }
            }
        }
    }
    fn handle_input(&mut self, s: String) {
        let tokens: Vec<&str> = s.split_whitespace().collect();
        if tokens.is_empty() {
            return;
        }
        match tokens[0] {
            "uci" => self.handle_uci(),
            "debug" => self.handle_debug(tokens),
            "isready" => self.handle_isready(),
            "setoption" => self.handle_setoption(tokens),
            "register" => self.handle_register(),
            "ucinewgame" => self.handle_ucinewgame(),
            "position" => self.handle_position(tokens),
            "go" => self.handle_go(tokens),
            "stop" => self.handle_stop(),
            "handle_ponderhit" => self.handle_ponderhit(),
            "quit" => std::process::exit(0),
            "perft" => self.handle_perft(tokens),
            "eval" => self.handle_eval(),
            _ => eprintln!("Did not recognize command \"{}\"", tokens[0]),
        }
    }
    fn handle_uci(&self) {
        println!("id name {VERSION_STRING}");
        println!("id author Jannik Gröne");
        self.print_config();
        println!("uciok");
    }
    fn handle_debug(&mut self, _tokens: Vec<&str>) {
        println!("{}", self.position().get_board());
    }
    //If we are not ready we will not parse in the first place.
    fn handle_isready(&self) {
        println!("readyok");
    }
    fn handle_setoption(&mut self, tokens: Vec<&str>) {
        let option = tokens.get(2);
        let value = tokens.get(4);
        if option.is_none() || value.is_none() {
            return;
        }
        self.config.set_option(option.unwrap(), value.unwrap());
        self.apply_options();
    }
    //You can't, but you can try..
    fn handle_register(&self) {}
    fn handle_ucinewgame(&mut self) {
        self.reset_all();
    }
    fn handle_position(&mut self, tokens: Vec<&str>) {
        let (offset, mut pos) = if tokens[1] == "startpos" {
            (2, Position::new())
        } else if tokens[1] == "fen" {
            let fen_tokens: Vec<&str> = tokens
                .iter()
                .skip(2)
                .take_while(|&s| *s != "moves")
                .copied()
                .collect();
            (
                2 + fen_tokens.len(),
                Position::from_fen(fen_tokens[0..].join(" ")).unwrap(),
            )
        } else {
            return;
        };
        self.set_position(pos.clone());
        if tokens.len() > offset && tokens[offset] == "moves" {
            for m in &tokens[offset + 1..] {
                //TODO: Somewhat hacky fix
                let mv = Move::from_str(m, pos.get_board());
                self.do_move(mv);
                pos.do_move(mv);
            }
        }
    }
    fn handle_go(&mut self, tokens: Vec<&str>) {
        if tokens.len() < 2 {
            return;
        }
        if tokens[1] == "depth" {
            if tokens.len() < 3 {
                return;
            }
            if let Ok(d) = tokens[2].parse::<u8>() {
                self.start_search(SearchLimit::Depth(d));
            }
            return;
        }
        if tokens[1] == "infinite" {
            self.start_search(SearchLimit::Infinite);
            return;
        }
        let mut wtime = None;
        let mut btime = None;
        let mut winc = None;
        let mut binc = None;
        let mut movetime = None;
        let mut movestogo = None;
        for chunk in tokens[1..].chunks(2) {
            match chunk[0] {
                "wtime" => wtime = chunk[1].parse::<u64>().ok(),
                "btime" => btime = chunk[1].parse::<u64>().ok(),
                "winc" => winc = chunk[1].parse::<u64>().ok(),
                "binc" => binc = chunk[1].parse::<u64>().ok(),
                "movetime" => movetime = chunk[1].parse::<u64>().ok(),
                "movestogo" => movestogo = chunk[1].parse::<u64>().ok(),
                _ => {}
            }
        }
        let time = match self.color() {
            Color::White => TimeSpec {
                ustime: wtime,
                usinc: winc,
                movestogo,
                movetime,
            },
            Color::Black => TimeSpec {
                ustime: btime,
                usinc: binc,
                movestogo,
                movetime,
            },
        };
        let limit = time_limit(&time);
        if let Some(l) = limit {
            self.start_search(l);
        } else {
            println!("info string invalid time spec ignored");
            self.start_search(SearchLimit::Infinite);
        }
    }
    fn handle_stop(&mut self) {
        self.stop_search();
    }
    fn handle_ponderhit(&self) {}
    fn handle_perft(&self, tokens: Vec<&str>) {
        let start = std::time::Instant::now();
        self.perft(tokens[1].parse::<u8>().unwrap());
        let delta = start.elapsed();
        println!("\n{}ms elapsed.", delta.as_millis());
    }
    fn handle_eval(&self) {
        println!("Eval: {}", self.eval());
    }
}
