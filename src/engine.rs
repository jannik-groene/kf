use std::sync::mpsc::{Sender, Receiver};
use std::fmt::Display;

use crate::{
    search::{SearchManager, SearchInfo},
    chess::{Position, Color},
};

#[derive(Clone,PartialEq)]
enum OptionValue {
    CHECK(bool),
    SPIN(i64),
//    COMBO(String),
//    BUTTON,
    STRING(String),
}

impl Display for OptionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::CHECK(b) => write!(f, "{}", b),
            Self::SPIN(n) => write!(f, "{}", n),
//            Self::COMBO(s) => write!(f, "{}", s),
//            Self::BUTTON => write!(f, ""),
            Self::STRING(s) => write!(f, "{}", s),
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
    options: Vec<ConfigOption>
}

impl EngineConfig {
    fn new() -> EngineConfig {
        let options = vec![ ConfigOption {id: "Hash".to_string(),
                                          value: OptionValue::SPIN(128),
                                          default: Some(OptionValue::SPIN(128)),
                                          min: Some(OptionValue::SPIN(0)),
                                          max: Some(OptionValue::SPIN(8192)),
                                          vars: None},
                            ConfigOption {id: "Threads".to_string(),
                                          value: OptionValue::SPIN(4),
                                          default: Some(OptionValue::SPIN(4)),
                                          min: Some(OptionValue::SPIN(1)),
                                          max: Some(OptionValue::SPIN(8)),
                                          vars: None},
                            ConfigOption {id: "UseNNUE".to_string(),
                                          value: OptionValue::CHECK(false),
                                          default: Some(OptionValue::CHECK(false)),
                                          min: None,
                                          max: None,
                                          vars: None},
                            ConfigOption {id: "NNUEPath".to_string(),
                                          value: OptionValue::STRING("nn-33c9d39e5eb6.nnue".to_string()),
                                          default: Some(OptionValue::STRING("nn-33c9d39e5eb6.nnue".to_string())),
                                          min: None,
                                          max: None,
                                          vars: None}
                          ];
        EngineConfig {options,}
    }
    fn print_config(&self) {
        for option in self.options.iter() {
            print!("option name {} type ", option.id);
            match option.value {
                OptionValue::SPIN(_) => print!("spin"),
                OptionValue::CHECK(_) => print!("check"),
//                OptionValue::COMBO(_) => print!("combo"),
//                OptionValue::BUTTON => print!("button"),
                OptionValue::STRING(_) => print!("string"),
            }
            if option.default.is_some() {
                print!(" default {}", option.default.as_ref().unwrap());
            }
            if option.min.is_some() {
                print!(" min {}", option.min.as_ref().unwrap());
            }
            if option.max.is_some() {
                print!(" max {}", option.max.as_ref().unwrap());
            }
            if option.vars.is_some() {
                for v in option.vars.as_ref().unwrap() {
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
        if option.is_none() {return;}
        match &mut option.unwrap().value {
            OptionValue::SPIN(n) => {
                let val = value.parse::<i64>();
                if let Ok(k) = val {
                    *n = k;
                }
            },
            OptionValue::CHECK(b) => {
                if value == "true" {
                    *b = true;
                } else if value == "false" {
                    *b = false;
                }
            },
            OptionValue::STRING(s) => {
                *s = value.to_string()
            },
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
    UCIINPUT(String),
    SEARCHUPDATE(SearchInfo),
    SEARCHENDED(u64),
    TIMERENDED(u64),
}

impl Engine {
    pub fn new() -> Engine {
        let config = EngineConfig::new();
        let mut search = SearchManager::new();
        let threads = match config.get_option("Threads").unwrap() { OptionValue::SPIN(n) => n, _ => 1 };
        let hash_size = match config.get_option("Hash").unwrap() { OptionValue::SPIN(n) => n, _ => 1 };
        let use_nnue = match config.get_option("UseNNUE").unwrap() { OptionValue::CHECK(b) => b, _ => false };
        search.set_threads(threads as usize);
        search.set_hash_size(hash_size as usize);
        search.set_use_nnue(use_nnue);
        if use_nnue {
            let nnue_path = match config.get_option("NNUEPath").unwrap() { OptionValue::STRING(s) => s, _ => "".to_string() };
            let path = std::path::Path::new(&nnue_path);
            crate::nnue::load_model(path).unwrap();
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
        self.search.search(self.channel.0.clone(),depth, self.search_id);
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
            OptionValue::SPIN(n) => n,
            _ => unreachable!(),
        };
        self.search.set_threads(threads as usize);
        let hash_size = match self.config.get_option("Hash").unwrap() {
            OptionValue::SPIN(n) => n,
            _ => unreachable!(),
        };
        self.search.set_hash_size(hash_size as usize);
        let use_nnue = match self.config.get_option("UseNNUE").unwrap() {
            OptionValue::CHECK(b) => b,
            _ => unreachable!(),
        };
        self.search.set_use_nnue(use_nnue);
        if use_nnue {
            let nnue_path = match self.config.get_option("NNUEPath").unwrap() {
                OptionValue::STRING(s) => s,
                _ => unreachable!(),
            };
            let path = std::path::Path::new(&nnue_path);
            crate::nnue::load_model(path).unwrap();
        }
    }
    pub fn reset_hash(&mut self) {
        self.search.reset_hash();
    }
    fn perft_step(pos: &mut Position, d: u8) -> usize {
        if d == 0 {
            1
        } else if d == 1 {
            pos.get_moves().len()
        } else {
            let mut total = 0;
            for m in pos.get_moves() {
                pos.do_move(m);
                total += Self::perft_step(pos, d-1);
                pos.undo_move();
            }
            total
        }
    }
    pub fn perft(&self, d: u8) {
        let mut pos = self.search.root_position();
        let now = std::time::Instant::now();
        let mut total = 0;
        for m in pos.get_moves() {
            pos.do_move(m);
            let m_count = Self::perft_step(&mut pos, d-1);
            println!("Root move {} positions {}", m, m_count);
            total += m_count;
            pos.undo_move();
        }
        println!("Total {}", total);
        println!("Time {} µs", (std::time::Instant::now()-now).as_micros());
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
    assert!(e.config.get_option("Threads").unwrap() == OptionValue::SPIN(8));
}
