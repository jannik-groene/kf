use std::fmt::Display;
use std::io::{Write, stdout};
use std::sync::mpsc::{Receiver, Sender};

use crate::chess::{All, MoveList};
use crate::report::{Reporter, StdOutUCIResult};
pub use crate::search::SearchLimit;
use crate::{
    chess::{Color, Move, Position},
    search::SearchManager,
};

#[derive(Clone, PartialEq)]
enum OptionValue {
    // Check(bool),
    Spin(i64),
    // Combo(String),
    // Button,
    // String(String),
}

impl Display for OptionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            // Self::Check(b) => write!(f, "{}", b),
            Self::Spin(n) => write!(f, "{}", n),
            // Self::Combo(s) => write!(f, "{}", s),
            // Self::Button => write!(f, ""),
            // Self::String(s) => write!(f, "{}", s),
        }
    }
}

struct ConfigOption {
    id: String,
    value: OptionValue,
    default: Option<OptionValue>,
    min: Option<OptionValue>,
    max: Option<OptionValue>,
    vars: Option<Vec<OptionValue>>,
}

pub struct EngineConfig {
    options: Vec<ConfigOption>,
}

impl EngineConfig {
    fn new() -> EngineConfig {
        let options = vec![
            ConfigOption {
                id: "Hash".to_string(),
                value: OptionValue::Spin(128),
                default: Some(OptionValue::Spin(128)),
                min: Some(OptionValue::Spin(0)),
                max: Some(OptionValue::Spin(8192)),
                vars: None,
            },
            ConfigOption {
                id: "Threads".to_string(),
                value: OptionValue::Spin(1),
                default: Some(OptionValue::Spin(1)),
                min: Some(OptionValue::Spin(1)),
                max: Some(OptionValue::Spin(8)),
                vars: None,
            },
        ];
        EngineConfig { options }
    }
    fn print_config(&self) {
        for option in self.options.iter() {
            print!("option name {} type ", option.id);
            match option.value {
                OptionValue::Spin(_) => print!("spin"),
                // OptionValue::Check(_) => print!("check"),
                // OptionValue::Combo(_) => print!("combo"),
                // OptionValue::Button => print!("button"),
                // OptionValue::String(_) => print!("string"),
            }
            if let Some(default) = &option.default {
                print!(" default {}", default);
            }
            if let Some(min) = &option.min {
                print!(" min {}", min);
            }
            if let Some(max) = &option.max {
                print!(" max {}", max);
            }
            if let Some(vars) = &option.vars {
                for v in vars {
                    print!(" var {}", v);
                }
            }
            println!();
        }
    }
    fn get_option(&self, id: &str) -> Option<OptionValue> {
        let option = self.options.iter().find(|o| o.id == id);
        option.map(|o| o.value.clone())
    }
    pub fn set_option(&mut self, id: &str, value: &str) {
        let option = self.options.iter_mut().find(|o| o.id == id);
        if option.is_none() {
            return;
        }
        match &mut option.unwrap().value {
            OptionValue::Spin(n) => {
                let val = value.parse::<i64>();
                if let Ok(k) = val {
                    *n = k;
                }
            }
        }
    }
}

pub struct Engine<T: Reporter> {
    search: SearchManager<T>,
    pub config: EngineConfig,
    channel: (Sender<EngineIO>, Receiver<EngineIO>),
}

#[derive(Clone)]
pub enum EngineIO {
    UciInput(String),
}

impl<T: Reporter> Engine<T> {
    pub fn new(reporter: T) -> Self {
        let config = EngineConfig::new();
        let mut search = SearchManager::new(reporter);
        let OptionValue::Spin(threads) = config.get_option("Threads").unwrap();
        let OptionValue::Spin(hash_size) = config.get_option("Hash").unwrap();
        search.set_threads(threads as usize);
        search.set_hash_size(hash_size as usize);
        Self {
            search,
            config,
            channel: std::sync::mpsc::channel(),
        }
    }
    pub fn get_sender(&self) -> Sender<EngineIO> {
        self.channel.0.clone()
    }
    pub fn receiver(&self) -> &Receiver<EngineIO> {
        &self.channel.1
    }
    pub fn print_config(&self) {
        self.config.print_config();
    }
    pub fn set_position(&mut self, pos: Position) {
        self.search.set_position(pos);
    }
    pub fn start_search(&mut self, limit: SearchLimit) {
        self.search.search(limit);
    }
    pub fn stop_search(&mut self) {
        self.search.stop();
    }
    pub fn color(&self) -> Color {
        self.search.color()
    }
    pub fn apply_options(&mut self) {
        let OptionValue::Spin(threads) = self.config.get_option("Threads").unwrap();
        self.search.set_threads(threads as usize);
        let OptionValue::Spin(hash_size) = self.config.get_option("Hash").unwrap();
        self.search.set_hash_size(hash_size as usize);
    }
    pub fn reset_all(&mut self) {
        self.search.reset_hash();
        self.search.reset_thread_data();
    }
    fn perft_step(pos: &mut Position, d: u8) -> usize {
        let mut moves = MoveList::new();
        pos.get_moves::<All>(&mut moves);
        if d == 0 {
            1
        } else if d == 1 {
            moves.len()
        } else {
            let mut total = 0;
            for m in moves {
                pos.do_move(m);
                total += Self::perft_step(pos, d - 1);
                pos.undo_move();
            }
            total
        }
    }
    pub fn perft(&self, d: u8) {
        let mut pos = self.search.root_position();
        let mut total = 0;
        let mut moves = MoveList::new();
        pos.get_moves::<All>(&mut moves);
        let rootmove_len = (moves.len() as f64).log10().floor() as usize + 1;
        let mut counts = Vec::with_capacity(moves.len());
        for (i, m) in moves.iter().enumerate() {
            pos.do_move(*m);
            let m_count = Self::perft_step(&mut pos, d - 1);
            pos.undo_move();
            counts.push(m_count);
            total += m_count;
            print!(
                "\rMove {:>3$}/{}: {}",
                i + 1,
                moves.len(),
                total,
                rootmove_len
            );
            stdout().flush().unwrap();
        }
        let numlen = (total as f64).log10().floor() as usize + 1;
        print!("\r{:>1$}\r", "", 14 + numlen);
        for (m, c) in moves.iter().zip(counts) {
            println!("{:<5}  {:>2$}", format!("{}", m), c, numlen);
        }
        println!("{:->1$}", "", numlen + 7);
        println!("Total  {}", total);
    }
    pub fn eval(&self) -> i32 {
        crate::evaluate::nnue::evaluate_position(&self.search.root_position())
    }
    pub fn position(&self) -> crate::chess::Position {
        self.search.root_position()
    }
    pub fn do_move(&mut self, m: Move) {
        self.search.do_move(m);
    }
}

impl Default for Engine<StdOutUCIResult> {
    fn default() -> Self {
        Self::new(StdOutUCIResult::default())
    }
}

#[test]
fn get_and_set_options() {
    let mut e = Engine::new(StdOutUCIResult::default());
    e.print_config();
    e.config.set_option("Threads", "8");
    assert!(e.config.get_option("Threads").unwrap() == OptionValue::Spin(8));
}
