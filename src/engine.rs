use std::fmt::Display;
use std::io::{stdout, Write};
use std::sync::mpsc::{Receiver, Sender};

use crate::{
    chess::{Color, Position, Move},
    search::SearchManager,
};

#[derive(Clone, PartialEq)]
enum OptionValue {
    Check(bool),
    Spin(i64),
    //    COMBO(String),
    //    BUTTON,
    String(String),
}

impl Display for OptionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Check(b) => write!(f, "{}", b),
            Self::Spin(n) => write!(f, "{}", n),
            //            Self::COMBO(s) => write!(f, "{}", s),
            //            Self::BUTTON => write!(f, ""),
            Self::String(s) => write!(f, "{}", s),
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
            ConfigOption {
                id: "UseNNUE".to_string(),
                value: OptionValue::Check(false),
                default: Some(OptionValue::Check(false)),
                min: None,
                max: None,
                vars: None,
            },
            ConfigOption {
                id: "NNUEPath".to_string(),
                value: OptionValue::String("nn-33c9d39e5eb6.nnue".to_string()),
                default: Some(OptionValue::String("nn-33c9d39e5eb6.nnue".to_string())),
                min: None,
                max: None,
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
                OptionValue::Check(_) => print!("check"),
                //                OptionValue::COMBO(_) => print!("combo"),
                //                OptionValue::BUTTON => print!("button"),
                OptionValue::String(_) => print!("string"),
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
            OptionValue::Check(b) => {
                if value == "true" {
                    *b = true;
                } else if value == "false" {
                    *b = false;
                }
            }
            OptionValue::String(s) => *s = value.to_string(),
        }
    }
}

pub struct Engine {
    search: SearchManager,
    pub config: EngineConfig,
    //ID of the current search
    search_id: u64,
    channel: (Sender<EngineIO>, Receiver<EngineIO>),
}

#[derive(Clone)]
pub enum EngineIO {
    UciInput(String),
    SearchEnded(u64),
    TimerEnded(u64),
}

impl Engine {
    pub fn new() -> Engine {
        let config = EngineConfig::new();
        let mut search = SearchManager::new();
        let threads = match config.get_option("Threads").unwrap() {
            OptionValue::Spin(n) => n,
            _ => 1,
        };
        let hash_size = match config.get_option("Hash").unwrap() {
            OptionValue::Spin(n) => n,
            _ => 1,
        };
        let use_nnue = match config.get_option("UseNNUE").unwrap() {
            OptionValue::Check(b) => b,
            _ => false,
        };
        search.set_threads(threads as usize);
        search.set_hash_size(hash_size as usize);
        search.set_use_nnue(use_nnue);
        if use_nnue {
            let nnue_path = match config.get_option("NNUEPath").unwrap() {
                OptionValue::String(s) => s,
                _ => "".to_string(),
            };
            let path = std::path::Path::new(&nnue_path);
            crate::evaluate::nnue::load_model(path).unwrap();
        }
        Engine {
            search,
            config,
            search_id: 1,
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
    pub fn start_search(&mut self, depth: Option<u8>) {
        self.search_id += 1;
        self.search
            .search(depth);
    }
    pub fn stop_search(&mut self) {
        self.search.stop();
    }
    pub fn increase_search_id(&mut self) {
        self.search_id += 1;
    }
    pub fn color(&self) -> Color {
        self.search.color()
    }
    pub fn search_id(&self) -> u64 {
        self.search_id
    }
    pub fn apply_options(&mut self) {
        let threads = match self.config.get_option("Threads").unwrap() {
            OptionValue::Spin(n) => n,
            _ => unreachable!(),
        };
        self.search.set_threads(threads as usize);
        let hash_size = match self.config.get_option("Hash").unwrap() {
            OptionValue::Spin(n) => n,
            _ => unreachable!(),
        };
        self.search.set_hash_size(hash_size as usize);
        let use_nnue = match self.config.get_option("UseNNUE").unwrap() {
            OptionValue::Check(b) => b,
            _ => unreachable!(),
        };
        self.search.set_use_nnue(use_nnue);
        if use_nnue {
            let nnue_path = match self.config.get_option("NNUEPath").unwrap() {
                OptionValue::String(s) => s,
                _ => unreachable!(),
            };
            let path = std::path::Path::new(&nnue_path);
            crate::evaluate::nnue::load_model(path).unwrap();
        }
    }
    pub fn reset_hash(&mut self) {
        self.search.reset_hash();
    }
    fn perft_step(mut pos: Position, d: u8) -> usize {
        if d == 0 {
            1
        } else if d == 1 {
            pos.get_moves::<true>().len()
        } else {
            let mut total = 0;
            for m in pos.get_moves::<true>() {
                total += Self::perft_step(pos.from_move(m), d - 1);
            }
            total
        }
    }
    pub fn perft(&self, d: u8) {
        let mut pos = self.search.root_position();
        let mut total = 0;
        let moves = pos.get_moves::<true>();
        let rootmove_len = (moves.len() as f64).log10().floor() as usize + 1;
        let mut counts = Vec::with_capacity(moves.len());
        for (i, m) in moves.iter().enumerate() {
            let m_count = Self::perft_step(pos.from_move(*m), d - 1);
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
    pub fn eval(&self) -> crate::evaluate::Eval {
        crate::evaluate::evaluate(&self.search.root_position())
    }
    pub fn position(&self) -> crate::chess::Position {
        self.search.root_position()
    }
    pub fn do_move(&mut self, m: Move) {
        self.search.do_move(m);
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn get_and_set_options() {
    let mut e = Engine::new();
    e.print_config();
    e.config.set_option("Threads", "8");
    assert!(e.config.get_option("Threads").unwrap() == OptionValue::Spin(8));
}
