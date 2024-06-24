use std::{io::sink, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tracing_subscriber::{layer::SubscriberExt, Registry};

mod support;
use support::MultithreadedBench;

fn mk_dispatch() -> tracing::Dispatch {
    #[cfg(not(bench_bunyan_baseline))]
    {
        json_subscriber_dispatch()
    }

    #[cfg(bench_bunyan_baseline)]
    {
        bunyan_dispatch()
    }
}

#[cfg(not(bench_bunyan_baseline))]
fn json_subscriber_dispatch() -> tracing::Dispatch {
    use tracing::Subscriber;
    use tracing_subscriber::{registry::LookupSpan, Layer};

    struct EnterExitLayer;

    impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for EnterExitLayer {
        fn on_enter(
            &self,
            id: &tracing_core::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            tracing::info!(parent: id, "enter");
        }

        fn on_exit(
            &self,
            id: &tracing_core::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            tracing::info!(parent: id, "exit");
        }
    }

    let collector = Registry::default()
        .with(json_subscriber::bunyan::layer(sink))
        .with(EnterExitLayer);

    tracing::Dispatch::new(collector)
}

#[cfg(bench_bunyan_baseline)]
fn bunyan_dispatch() -> tracing::Dispatch {
    use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};

    let formatting_layer = BunyanFormattingLayer::new("tracing_demo".into(), sink);
    let collector = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);

    tracing::Dispatch::new(collector)
}

fn bench_new_span(c: &mut Criterion) {
    bench_thrpt(c, "new_span", |group, i| {
        group.bench_with_input(BenchmarkId::new("single_thread", i), i, |b, &i| {
            tracing::dispatcher::with_default(&mk_dispatch(), || {
                b.iter(|| {
                    for n in 0..i {
                        let _span = tracing::info_span!("span", n);
                    }
                })
            });
        });
        group.bench_with_input(BenchmarkId::new("multithreaded", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                let dispatch = mk_dispatch();
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(dispatch.clone());
                    let elapsed = bench
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
    });
}

type Group<'a> = criterion::BenchmarkGroup<'a, criterion::measurement::WallTime>;
fn bench_thrpt(c: &mut Criterion, name: &'static str, mut f: impl FnMut(&mut Group<'_>, &usize)) {
    const N_SPANS: &[usize] = &[1, 10, 50];

    let mut group = c.benchmark_group(name);
    for spans in N_SPANS {
        group.throughput(Throughput::Elements(*spans as u64));
        f(&mut group, spans);
    }
    group.finish();
}

fn bench_event(c: &mut Criterion) {
    bench_thrpt(c, "event", |group, i| {
        group.bench_with_input(BenchmarkId::new("root/single_threaded", i), i, |b, &i| {
            let dispatch = mk_dispatch();
            tracing::dispatcher::with_default(&dispatch, || {
                b.iter(|| {
                    for n in 0..i {
                        tracing::info!(n);
                    }
                })
            });
        });
        group.bench_with_input(BenchmarkId::new("root/multithreaded", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                let dispatch = mk_dispatch();
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(dispatch.clone());
                    let elapsed = bench
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
        group.bench_with_input(
            BenchmarkId::new("unique_parent/single_threaded", i),
            i,
            |b, &i| {
                tracing::dispatcher::with_default(&mk_dispatch(), || {
                    let span = tracing::info_span!("unique_parent", foo = false);
                    let _guard = span.enter();
                    b.iter(|| {
                        for n in 0..i {
                            tracing::info!(n);
                        }
                    })
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("unique_parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let mut total = Duration::from_secs(0);
                    let dispatch = mk_dispatch();
                    for _ in 0..iters {
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let elapsed = bench
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("shared_parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let dispatch = mk_dispatch();
                    let mut total = Duration::from_secs(0);
                    for _ in 0..iters {
                        let parent = tracing::dispatcher::with_default(&dispatch, || {
                            tracing::info_span!("shared_parent", foo = "hello world")
                        });
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let elapsed = bench.run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("multi-parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let dispatch = mk_dispatch();
                    let mut total = Duration::from_secs(0);
                    for _ in 0..iters {
                        let parent = tracing::dispatcher::with_default(&dispatch, || {
                            tracing::info_span!("multiparent", foo = "hello world")
                        });
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let elapsed = bench.run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
    });
}

fn bench_record(c: &mut Criterion) {
    bench_thrpt(c, "record", |group, i| {
        group.bench_with_input(
            BenchmarkId::new("record_only/single_threaded", i),
            i,
            |b, &i| {
                tracing::dispatcher::with_default(&mk_dispatch(), || {
                    let span = tracing::info_span!("unique_parent", foo = false);
                    let _guard = span.enter();
                    b.iter(|| {
                        for n in 0..i {
                            span.record("foo", n);
                        }
                    })
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("record_only/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let mut total = Duration::from_secs(0);
                    let dispatch = mk_dispatch();
                    for _ in 0..iters {
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let span = tracing::info_span!("unique_parent", foo = false);
                        let elapsed = bench
                            .thread_with_setup_n(4, move |start| {
                                let span = span.clone();
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    span.record("foo", n);
                                }
                            })
                            .run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("record_and_event/single_threaded", i),
            i,
            |b, &i| {
                tracing::dispatcher::with_default(&mk_dispatch(), || {
                    let span = tracing::info_span!("unique_parent", foo = false);
                    let _guard = span.enter();
                    b.iter(|| {
                        for n in 0..i {
                            span.record("foo", n);
                            tracing::info!(n);
                        }
                    })
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("record_and_event/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let mut total = Duration::from_secs(0);
                    let dispatch = mk_dispatch();
                    for _ in 0..iters {
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let span = tracing::info_span!("unique_parent", foo = false);
                        let elapsed = bench
                            .thread_with_setup_n(4, move |start| {
                                let span = span.clone();
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    span.record("foo", n);
                                    tracing::info!(n);
                                }
                            })
                            .run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
    });
}

criterion_group!(benches, bench_new_span, bench_event, bench_record);
criterion_main!(benches);
