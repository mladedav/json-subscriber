use std::{any::Any, hint::black_box, io::sink};

use criterion::{criterion_group, criterion_main, Bencher, BenchmarkId, Criterion, Throughput};
use tracing::Dispatch;

mod support;

fn make_dispatch(current_span: bool, span_list: bool) -> tracing::Dispatch {
    let collector = json_subscriber::fmt::Subscriber::builder()
        .with_writer(sink)
        .with_current_span(current_span)
        .with_span_list(span_list)
        .finish();
    tracing::Dispatch::new(collector)
}

fn create_span_with_fields(n: usize) -> tracing::Span {
    tracing::info_span!(
        "span",
        n,
        text = "lorem ipsum",
        number = 42,
        float = 4.2,
        detail = debug([0, 1, 2])
    )
}

fn nested_entered_spans(depth: usize) -> Vec<tracing::span::EnteredSpan> {
    let mut guards = Vec::with_capacity(depth);
    for level in 0..depth {
        guards.push(create_span_with_fields(level).entered());
    }
    guards
}


fn bench_operations(criterion: &mut Criterion) {
    fn run_bench(
        bencher: &mut Bencher,
        input: usize,
        dispatch: &Dispatch,
        setup: impl Fn(usize) -> Box<dyn Any>,
        operation: impl Fn(usize),
    ) {
        tracing::dispatcher::with_default(dispatch, || {
            let _setup = setup(input);
            bencher.iter(|| {
                operation(black_box(input));
            })
        });
    }

    fn prepare_bench(
        setup: impl Fn(usize) -> Box<dyn Any> + Clone + 'static,
        operation: impl Fn(usize) + Clone + 'static,
    ) -> [(&'static str, Box<dyn FnMut(&mut Bencher<'_>, &usize)>); 4] {
        [
            (
                "no_span_output",
                Box::new({
                    let operation = operation.clone();
                    let setup = setup.clone();
                    move |bencher, &input| {
                        run_bench(
                            bencher,
                            input,
                            &make_dispatch(false, false),
                            setup.clone(),
                            operation.clone(),
                        );
                    }
                }),
            ),
            (
                "current_span",
                Box::new({
                    let operation = operation.clone();
                    let setup = setup.clone();
                    move |bencher, &input| {
                        run_bench(
                            bencher,
                            input,
                            &make_dispatch(true, false),
                            setup.clone(),
                            operation.clone(),
                        );
                    }
                }),
            ),
            (
                "span_list",
                Box::new({
                    let operation = operation.clone();
                    let setup = setup.clone();
                    move |bencher, &input| {
                        run_bench(
                            bencher,
                            input,
                            &make_dispatch(false, true),
                            setup.clone(),
                            operation.clone(),
                        );
                    }
                }),
            ),
            (
                "current_span_and_span_list",
                Box::new({
                    let operation = operation.clone();
                    let setup = setup.clone();
                    move |bencher, &input| {
                        run_bench(
                            bencher,
                            input,
                            &make_dispatch(true, true),
                            setup.clone(),
                            operation.clone(),
                        );
                    }
                }),
            ),
        ]
    }

    bench_throughput_group(
        criterion,
        "new_span",
        [100],
        prepare_bench(
            |_| Box::new(()),
            |n| {
                for _ in 0..n {
                    black_box(create_span_with_fields(black_box(n)));
                }
            },
        ),
    );

    bench_throughput_group(
        criterion,
        "event_at_root",
        [100],
        prepare_bench(
            |_| Box::new(()),
            |n| {
                for _ in 0..n {
                    tracing::info!(
                        text = black_box("lorem ipsum"),
                        number = black_box(42),
                        float = black_box(4.2),
                        detail = black_box(debug([0, 1, 2])),
                        "hello"
                    );
                }
            },
        ),
    );

    bench_throughput_group(
        criterion,
        "event_in_span",
        [1, 10, 100],
        prepare_bench(
            |n| Box::new(create_span_with_fields(n).entered()),
            |n| {
                for _ in 0..n {
                    tracing::info!("Hello");
                }
            },
        ),
    );

    // Input is parent-span depth, not work units, so throughput-as-elements would
    // misreport. Use the plain group helper so criterion just plots time vs. depth.
    bench_input_group(
        criterion,
        "event_in_nested_span",
        [1, 5, 25],
        prepare_bench(
            |depth| Box::new(nested_entered_spans(depth)),
            |_| {
                tracing::info!("Hello");
            },
        ),
    );

    bench_throughput_group(
        criterion,
        "record_value_in_span",
        [1, 10, 100],
        prepare_bench(
            |n| Box::new(create_span_with_fields(n).entered()),
            |n| {
                let span = tracing::Span::current();
                for _ in 0..n {
                    span.record("n", black_box(n));
                }
            },
        ),
    );

}

fn bench_input_group<const INPUTS: usize, const COUNT: usize>(
    c: &mut Criterion,
    group_name: &'static str,
    inputs: [usize; INPUTS],
    mut benches: [(&'static str, Box<dyn FnMut(&mut Bencher<'_>, &usize)>); COUNT],
) {
    let mut group = c.benchmark_group(group_name);
    for input in inputs {
        for (name, operation) in &mut benches {
            group.bench_with_input(BenchmarkId::new(*name, input), &input, operation);
        }
    }
    group.finish();
}

fn bench_throughput_group<const INPUTS: usize, const COUNT: usize>(
    c: &mut Criterion,
    group_name: &'static str,
    inputs: [usize; INPUTS],
    mut benches: [(&'static str, Box<dyn FnMut(&mut Bencher<'_>, &usize)>); COUNT],
) {
    let mut group = c.benchmark_group(group_name);
    for input in inputs {
        group.throughput(Throughput::Elements(input as u64));
        for (name, operation) in &mut benches {
            group.bench_with_input(BenchmarkId::new(*name, input), &input, operation);
        }
    }
    group.finish();
}

criterion_group!(benches, bench_operations);
criterion_main!(benches);
