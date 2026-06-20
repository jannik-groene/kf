use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use super::TimeManager;
use super::iterative_deepening;
use super::thread::{SearchHead, SharedData};
use crate::chess::Position;

enum ThreadSignal {
    StartSearch(u8),
    SetLimit(TimeManager),
    SetPosition(Position),
    EndThread,
}

pub struct ThreadPool {
    worker_handles: Vec<JoinHandle<()>>,
    worker_tx: Vec<Sender<ThreadSignal>>,
    shared: Arc<SharedData>,
}

impl ThreadPool {
    pub fn new(shared: Arc<SharedData>, threads: usize) -> Self {
        assert!(threads != 0);
        let mut worker_handles = Vec::new();
        let mut worker_tx = Vec::new();
        for i in 0..threads {
            let (tx, rx) = channel();
            worker_tx.push(tx);
            let mut search_head = SearchHead::new(
                Position::new(),
                shared.clone(),
                TimeManager::new(std::time::Instant::now(), None),
            );
            worker_handles.push(spawn(move || idle_loop(i, &mut search_head, rx)));
        }
        Self {
            worker_handles,
            worker_tx,
            shared,
        }
    }

    pub fn start_searching(
        &self,
        pos: &Position,
        target_depth: Option<u8>,
        time_limit: Option<Duration>,
    ) {
        let depth = target_depth.unwrap_or(u8::MAX);
        let time_manager = TimeManager::new(std::time::Instant::now(), time_limit);

        self.shared.stop_flag.store(false, Ordering::Release);
        self.shared.nodes.store(0, Ordering::Release);

        let _ = self.worker_tx[0].send(ThreadSignal::SetPosition(pos.clone()));
        let _ = self.worker_tx[0].send(ThreadSignal::SetLimit(time_manager));
        let _ = self.worker_tx[0].send(ThreadSignal::StartSearch(depth));

        for tx in self.worker_tx.iter().skip(1) {
            let _ = tx.send(ThreadSignal::SetPosition(pos.clone()));
            let _ = tx.send(ThreadSignal::SetLimit(time_manager));
            let _ = tx.send(ThreadSignal::StartSearch(u8::MAX));
        }
    }

    // Change the number of threads in the threadpool
    pub fn set_threads(&mut self, threads: usize) {
        assert!(threads > 0);
        if threads == self.worker_handles.len() {
            return;
        }
        // Join all threads no longer needed
        while threads < self.worker_handles.len() {
            let tx = self.worker_tx.pop().unwrap();
            let handle = self.worker_handles.pop().unwrap();
            if let Ok(()) = tx.send(ThreadSignal::EndThread) {
                let _ = handle.join();
            }
        }
        let curr_idx = self.worker_handles.len();
        for i in curr_idx..threads {
            let (tx, rx) = channel();
            self.worker_tx.push(tx);
            let mut search_head = SearchHead::new(
                Position::new(),
                self.shared.clone(),
                TimeManager::new(std::time::Instant::now(), None),
            );
            self.worker_handles
                .push(spawn(move || idle_loop(i, &mut search_head, rx)));
        }
    }

    // Reset all thread data, such as position and histories
    // TT must be cleared separately
    pub fn reset_threads(&mut self) {
        let threads = self.worker_handles.len();
        // Clear old threads
        while !self.worker_handles.is_empty() {
            let tx = self.worker_tx.pop().unwrap();
            let handle = self.worker_handles.pop().unwrap();
            if let Ok(()) = tx.send(ThreadSignal::EndThread) {
                let _ = handle.join();
            }
        }
        // Launch the new threads
        for i in 0..threads {
            let (tx, rx) = channel();
            self.worker_tx.push(tx);
            let mut search_head = SearchHead::new(
                Position::new(),
                self.shared.clone(),
                TimeManager::new(std::time::Instant::now(), None),
            );
            self.worker_handles
                .push(spawn(move || idle_loop(i, &mut search_head, rx)));
        }
    }
}

fn idle_loop(id: usize, search_head: &mut SearchHead, rx: Receiver<ThreadSignal>) {
    while let Ok(msg) = rx.recv() {
        match msg {
            ThreadSignal::StartSearch(d) => {
                iterative_deepening(id, search_head, d);
            }
            ThreadSignal::SetLimit(tm) => search_head.time_manager = tm,
            ThreadSignal::SetPosition(pos) => search_head.pos = pos,
            ThreadSignal::EndThread => break,
        }
    }
}
