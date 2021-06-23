pub struct Config {
    pub is_initialized: bool,
    pub is_ready: bool,
    pub stop: bool,
}

impl Config {
    pub fn new() -> Config {
        Config {
            is_initialized: false,
            is_ready: true,
            stop: false,
        }
    }
}
