use std::{
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

use tracing::Dispatch;

#[derive(Clone)]
pub(super) struct MultithreadedBench {
    start: Arc<Barrier>,
    end: Arc<Barrier>,
    dispatch: Dispatch,
}

#[allow(dead_code)]
impl MultithreadedBench {
    pub(super) fn new(dispatch: Dispatch) -> Self {
        Self {
            start: Arc::new(Barrier::new(5)),
            end: Arc::new(Barrier::new(5)),
            dispatch,
        }
    }

    pub(super) fn thread(&self, f: impl FnOnce() + Send + 'static) -> &Self {
        self.thread_with_setup(|start| {
            start.wait();
            f()
        })
    }

    pub(super) fn thread_n(
        &self,
        repeat: usize,
        f: impl FnOnce() + Clone + Send + 'static,
    ) -> &Self {
        for _ in 0..repeat {
            self.thread(f.clone());
        }
        self
    }

    pub(super) fn thread_with_setup(&self, f: impl FnOnce(&Barrier) + Send + 'static) -> &Self {
        let this = self.clone();
        thread::spawn(move || {
            let dispatch = this.dispatch.clone();
            tracing::dispatcher::with_default(&dispatch, move || {
                f(&this.start);
                this.end.wait();
            })
        });
        self
    }

    pub(super) fn thread_with_setup_n(
        &self,
        repeat: usize,
        f: impl FnOnce(&Barrier) + Clone + Send + 'static,
    ) -> &Self {
        for _ in 0..repeat {
            self.thread_with_setup(f.clone());
        }
        self
    }

    pub(super) fn run(&self) -> Duration {
        self.start.wait();
        let t0 = Instant::now();
        self.end.wait();
        t0.elapsed()
    }
}
