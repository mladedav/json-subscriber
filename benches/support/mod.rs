use std::{
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

use criterion::{Criterion, Throughput};
use tracing::Dispatch;

type Group<'a> = criterion::BenchmarkGroup<'a, criterion::measurement::WallTime>;
#[allow(
    dead_code,
    reason = "This is used from benches, just the module tree is confused."
)]
pub(super) fn bench_thrpt(
    c: &mut Criterion,
    name: &'static str,
    mut f: impl FnMut(&mut Group<'_>, &usize),
) {
    const N_SPANS: &[usize] = &[1, 10, 50];

    let mut group = c.benchmark_group(name);
    for spans in N_SPANS {
        group.throughput(Throughput::Elements(*spans as u64));
        f(&mut group, spans);
    }
    group.finish();
}

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

    pub(super) fn run(&self) -> Duration {
        self.start.wait();
        let t0 = Instant::now();
        self.end.wait();
        t0.elapsed()
    }
}
