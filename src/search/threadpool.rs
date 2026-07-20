use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{JoinHandle, spawn};
use std::time::Instant;

use super::iterative_deepening;
use super::thread::{SearchHead, SharedData};
use crate::chess::Position;
use crate::report::{NullReport, Reporter};
use crate::search::thread::SearchLimit;

enum ThreadSignal {
    StartSearch(SearchLimit),
    SetPosition(Position),
    EndThread,
}

pub struct ThreadPool<T: Reporter> {
    worker_handles: Vec<JoinHandle<()>>,
    worker_tx: Vec<Sender<ThreadSignal>>,
    shared: Arc<SharedData>,
    reporter: T,
}

impl<T: Reporter> ThreadPool<T> {
    pub fn new(shared: Arc<SharedData>, threads: usize, reporter: T) -> Self {
        assert!(threads > 0);
        let mut worker_handles = Vec::new();
        let mut worker_tx = Vec::new();

        let (tx, rx) = channel();
        worker_tx.push(tx);
        let mut search_head = SearchHead::new(
            Position::new(),
            shared.clone(),
            Instant::now(),
            SearchLimit::Infinite,
        );
        let thread_reporter = reporter.clone();
        worker_handles.push(spawn(move || {
            idle_loop(0, &mut search_head, &rx, thread_reporter)
        }));

        for i in 1..threads {
            let (tx, rx) = channel();
            worker_tx.push(tx);
            let mut search_head = SearchHead::new(
                Position::new(),
                shared.clone(),
                Instant::now(),
                SearchLimit::Infinite,
            );
            worker_handles.push(spawn(move || {
                idle_loop(i, &mut search_head, &rx, NullReport::default())
            }));
        }
        Self {
            worker_handles,
            worker_tx,
            shared,
            reporter,
        }
    }

    pub fn start_searching(&self, pos: &Position, limit: SearchLimit) {
        self.shared.stop_flag.store(false, Ordering::Release);
        self.shared.nodes.store(0, Ordering::Release);
        self.shared.tt.increment_age();

        let _ = self.worker_tx[0].send(ThreadSignal::SetPosition(pos.clone()));
        let _ = self.worker_tx[0].send(ThreadSignal::StartSearch(limit));

        for tx in self.worker_tx.iter().skip(1) {
            let _ = tx.send(ThreadSignal::SetPosition(pos.clone()));
            let _ = tx.send(ThreadSignal::StartSearch(limit));
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
                Instant::now(),
                SearchLimit::Infinite,
            );
            self.worker_handles.push(spawn(move || {
                idle_loop(i, &mut search_head, &rx, NullReport::default())
            }));
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
        let (tx, rx) = channel();
        self.worker_tx.push(tx);
        let mut search_head = SearchHead::new(
            Position::new(),
            self.shared.clone(),
            Instant::now(),
            SearchLimit::Infinite,
        );
        let reporter = self.reporter.clone();
        self.worker_handles
            .push(spawn(move || idle_loop(0, &mut search_head, &rx, reporter)));

        for i in 1..threads {
            let (tx, rx) = channel();
            self.worker_tx.push(tx);
            let mut search_head = SearchHead::new(
                Position::new(),
                self.shared.clone(),
                Instant::now(),
                SearchLimit::Infinite,
            );
            self.worker_handles.push(spawn(move || {
                idle_loop(i, &mut search_head, &rx, NullReport::default())
            }));
        }
    }
}

fn idle_loop<T: Reporter>(
    id: usize,
    search_head: &mut SearchHead,
    rx: &Receiver<ThreadSignal>,
    reporter: T,
) {
    while let Ok(msg) = rx.recv() {
        match msg {
            ThreadSignal::StartSearch(limit) => {
                search_head.start_time = Instant::now();
                search_head.limit = limit;
                iterative_deepening(id, search_head, &reporter);
            }
            ThreadSignal::SetPosition(pos) => search_head.set_pos(pos),
            ThreadSignal::EndThread => break,
        }
    }
}
