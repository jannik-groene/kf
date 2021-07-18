use super::engine;
use std::sync::mpsc::Sender;

fn read_input(ch: Sender<engine::EngineIO>) {
    let sin = std::io::stdin();
    loop {
        let mut s = String::new();
        sin.read_line(&mut s).unwrap();
        match ch.send(engine::EngineIO::UCIINPUT(s)) {
            Ok(_) => {},
            Err(_) => break,
        }
    }
}
//Timer for execution. To achieve high precision we first sleep the thread to within a safetymargin
//of the target and the spin until we reach the target time.
fn timer(ustime: Option<u64>, _themtime: Option<u64>,
         usinc: Option<u64>, _theminc: Option<u64>,
         movestogo: Option<u64>, movetime: Option<u64>,
         id: u64, ch: Sender<engine::EngineIO>) {
    const SAFETY_DELTA: std::time::Duration = std::time::Duration::from_millis(1);
    let now = std::time::Instant::now();
    //We play with increment
    let movetime = if ustime.is_some() && usinc.is_some() {
            //We spend about 7% of the remaining time on each move.
            std::time::Duration::from_millis(usinc.unwrap()) +
                                std::time::Duration::from_micros((ustime.unwrap()-usinc.unwrap()) * 70)
        }
        //We play with time per x moves, we split evenly and spend a little more earlier
        else if ustime.is_some() && movestogo.is_some() {
            if movestogo.unwrap() == 1 {
                std::time::Duration::from_millis(ustime.unwrap())
            } else {
                std::time::Duration::from_micros((ustime.unwrap() * 1200)/movestogo.unwrap())
            }
        }
        //sudden death
        else if ustime.is_some() {
            //we simply spend 4% of our time in each move
            std::time::Duration::from_micros(ustime.unwrap() * 40)
        }
        //We play with fixed time per move
        else if movetime.is_some() {
            std::time::Duration::from_millis(movetime.unwrap())
        } else {
            panic!("Invalid time control data!")
        };
    if movetime > SAFETY_DELTA {
        std::thread::sleep(movetime-SAFETY_DELTA);
    }
    //Spin until we reach target time
    while now.elapsed() < movetime - std::time::Duration::from_micros(1) {}
    match ch.send(engine::EngineIO::TIMERENDED(id)) {
        Ok(_) => {},
        Err(_) => {},
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
}

impl UCIHandler for engine::Engine {
    fn uci_loop(&mut self) {
        let mut waiting_for_search_end = false;
        let tx = self.get_sender();
        let mut last_search_update: Option<engine::alphabeta::SearchInfo> = None;
        std::thread::spawn(|| read_input(tx));
        loop {
            match self.receiver().recv() {
                Ok(io) => match io {
                    engine::EngineIO::UCIINPUT(s) => self.handle_input(s),
                    engine::EngineIO::TIMERENDED(id) => {
                        if id == self.search_id() {
                            if last_search_update.is_some() {
                                match last_search_update.as_ref().unwrap().bestmove {
                                    Some(m) => println!("bestmove {}", m),
                                    None => println!("bestmove 0000"),
                                }
                            }
                            self.stop_search();
                            waiting_for_search_end = true;
                        }
                    },
                    engine::EngineIO::SEARCHUPDATE(up) => last_search_update = Some(up),
                    engine::EngineIO::SEARCHENDED(id)  => {
                        if !waiting_for_search_end && id == self.search_id() {
                            if last_search_update.is_some() {
                                match last_search_update.as_ref().unwrap().bestmove {
                                    Some(m) => println!("bestmove {}", m),
                                    None => println!("bestmove 0000"),
                                }
                            }
                        }
                        waiting_for_search_end = false;
                    }
                }
                _ => {},
            }
        }
    }
    fn handle_input(&mut self, s: String) {
        let tokens:Vec<&str> = s.trim().split_whitespace().collect();
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
            _ => eprintln!("Did not recognize command \"{}\"", tokens[0])
        }
    }
    fn handle_uci(&self) {
        println!("id name kf-0.0.3");
        println!("id author Jannik Gröne");
        self.print_config();
        println!("uciok");
    }
    fn handle_debug(&mut self, _tokens: Vec<&str>) {}
    //If we are not ready we will not parse in the first place.
    fn handle_isready(&self) {
        println!("readyok");
    }
    fn handle_setoption(&mut self, tokens: Vec<&str>) {
        let option = tokens.get(2);
        let value = tokens.get(4);
        if option.is_none() || value.is_none() {return;}
        self.config.set_option(option.unwrap(), value.unwrap());
        self.apply_options();
    }
    //You can't, but you can try..
    fn handle_register(&self) {}
    fn handle_ucinewgame(&mut self) {
        self.reset_hash();
    }
    fn handle_position(&mut self, tokens: Vec<&str>) {
        let mut offset = 0;
        let mut pos = if tokens[1] == "startpos" {
            offset = 2;
            engine::chess::Position::new()
        } else if tokens[1] == "fen" {
            let fen = tokens[2..=7].join(" ");
            offset = 8;
            engine::chess::Position::from_fen(fen).unwrap()
        } else {
            return;
        };
        if tokens.len() > offset && tokens[offset] == "moves" {
            for m in tokens[offset+1..].iter() {
                pos.do_move(engine::chess::Move::from_str(m, &pos));
            }
        }
        self.set_position(pos)
    }
    fn handle_go(&mut self, tokens: Vec<&str>) {
        if tokens.len() < 2 {return;}
        if tokens[1] == "depth" {
            if tokens.len() < 3 {return;}
            self.start_search(u8::from_str_radix(tokens[2],10).ok());
            return;
        }
        if tokens[1] == "infinite" {
            self.start_search(None);
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
                "wtime" => wtime = u64::from_str_radix(chunk[1],10).ok(),
                "btime" => btime = u64::from_str_radix(chunk[1],10).ok(),
                "winc" => winc = u64::from_str_radix(chunk[1],10).ok(),
                "binc" => binc = u64::from_str_radix(chunk[1],10).ok(),
                "movetime" => movetime = u64::from_str_radix(chunk[1],10).ok(),
                "movestogo" => movestogo = u64::from_str_radix(chunk[1],10).ok(),
                _ => {},
            }
        }
        let tx = self.get_sender();
        let id = self.search_id() + 1;
        match self.color() {
            engine::chess::Color::WHITE => std::thread::spawn(move || timer(wtime, btime, winc, binc, movestogo, movetime, id, tx)),
            engine::chess::Color::BLACK => std::thread::spawn(move || timer(btime, wtime, binc, winc, movestogo, movetime, id, tx)),
        };
        self.start_search(None);
    }
    fn handle_stop(&mut self) {
        self.stop_search();
    }
    fn handle_ponderhit(&self) {}
}
