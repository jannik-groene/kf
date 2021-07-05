pub mod engine;
mod uci;

use uci::UCIHandler;

#[cfg(test)]
mod tests;

fn main() {
    let mut engine = engine::Engine::new();
    engine.uci_loop();
}

//fn uci_loop() {
//    let sin = io::stdin();
//    let mut search = engine::alphabeta::ABSearchManager::new();
//    //UCI Loop
//    let mut pos: engine::chess::Position = engine::chess::Position::new();
//    loop {
//        let mut s = String::new();
//        sin.read_line(&mut s).unwrap();
//        if handle_input(s, &mut pos, &mut search) {
//            break;
//        }
//    }
//}
//
//fn handle_input(s: String, pos: &mut engine::chess::Position, search: &mut engine::alphabeta::ABSearchManager) -> bool {
//    let mut tokens:Vec<&str> = s.trim().split_whitespace().collect();
//    match tokens[0] {
//        "uci" => parse_uci(),
//        "debug" => parse_debug(),
//        "isready" => parse_isready(),
//        "setoption" => parse_setoption(),
//        "register" => parse_register(),
//        "ucinewgame" => parse_ucinewgame(),
//        "position" => parse_position(pos, tokens, search),
//        "go" => parse_go(pos, tokens, search),
//        "stop" => parse_stop(),
//        "parse_ponderhit" => parse_ponderhit(),
//        "quit" => std::process::exit(0),
//        _ => eprintln!("Did not recognize command \"{}\"", tokens[0])
//    }
//    false
//}
//
//fn parse_uci() {
//    println!("uciok");
//}
//
//fn parse_debug() {
//}
//
//fn parse_isready() {
//    println!("readyok");
//}
//
//fn parse_setoption() {
//}
//
//fn parse_register() {
//}
//
//fn parse_ucinewgame() {
//}
//
//fn parse_position(pos: &mut engine::chess::Position, tokens: Vec<&str>, search: &mut engine::alphabeta::ABSearchManager) {
//    let mut offset = 0;
//    if tokens[1] == "startpos" {
//        *pos = engine::chess::Position::new();
//        offset = 2;
//    } else if tokens[1] == "fen" {
//        let fen = tokens[2..=7].join(" ");
//        *pos = engine::chess::Position::from_fen(fen).unwrap();
//        offset = 8;
//    }
//    if tokens.len() > offset && tokens[offset] == "moves" {
//        for m in tokens[offset+1..].iter() {
//            pos.do_move(engine::chess::Move::from_str(m, pos));
//        }
//    }
//    search.set_position(pos.clone());
//}
//
//fn parse_go(pos: &mut engine::chess::Position, tokens: Vec<&str>, search: &mut engine::alphabeta::ABSearchManager) {
//    //let mut stree = engine::mcts::MCSTree::new(pos.clone());
//    //let mut dur = Duration::from_secs(1);
//    //if tokens[0] == "movetime" {
//    //    dur = Duration::from_millis(tokens[1].parse().unwrap());
//    //}
//    //stree.search_timed(dur);
//    //println!("info score cp {} nodes {} nps {}", (((stree.root_eval()-0.5)*3.1).tan()*290.) as i64, stree.get_node_count(), stree.get_node_count()/dur.as_secs());
//    //println!("info pv {}", stree.get_pv().iter().map(|m| format!("{}", *m)).collect::<Vec<_>>().join(" "));
//    //println!("bestmove {}", stree.best_move());
//    let (s,m) = search.search(None, Some(Duration::from_secs(15)));
//    println!("info {}", s);
//    if m.is_some() {
//        println!("bestmove {}", m.unwrap());
//    }
//}
//
//fn parse_stop() {
//}
//
//fn parse_ponderhit() {
//}
