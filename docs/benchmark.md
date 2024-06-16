## Comparison with `tracing-subscriber`

You can run this the suite using `tracing-subscriber` with:

```
RUSTFLAGS='--cfg bench_tracing_baseline' cargo bench -- --save-baseline tracing-subscriber
```

And then run the benchmark again using this library:

```
cargo bench -- --baseline tracing-subscriber
```

### Results

#### v0.1.1

```
new_span/single_thread/1
                        time:   [187.69 ns 188.37 ns 189.08 ns]
                        thrpt:  [5.2888 Melem/s 5.3086 Melem/s 5.3279 Melem/s]
                 change:
                        time:   [-52.385% -52.213% -52.015%] (p = 0.00 < 0.05)
                        thrpt:  [+108.40% +109.26% +110.02%]
                        Performance has improved.
Found 2 outliers among 100 measurements (2.00%)
  2 (2.00%) high mild
new_span/multithreaded/1
                        time:   [5.0995 µs 5.1833 µs 5.2690 µs]
                        thrpt:  [189.79 Kelem/s 192.93 Kelem/s 196.10 Kelem/s]
                 change:
                        time:   [-78.716% -77.920% -77.127%] (p = 0.00 < 0.05)
                        thrpt:  [+337.19% +352.89% +369.85%]
                        Performance has improved.
Found 4 outliers among 100 measurements (4.00%)
  2 (2.00%) low mild
  2 (2.00%) high mild
new_span/single_thread/10
                        time:   [1.9265 µs 1.9310 µs 1.9357 µs]
                        thrpt:  [5.1662 Melem/s 5.1788 Melem/s 5.1908 Melem/s]
                 change:
                        time:   [-50.981% -50.829% -50.664%] (p = 0.00 < 0.05)
                        thrpt:  [+102.69% +103.37% +104.00%]
                        Performance has improved.
Found 13 outliers among 100 measurements (13.00%)
  9 (9.00%) high mild
  4 (4.00%) high severe
new_span/multithreaded/10
                        time:   [9.4159 µs 9.5547 µs 9.7191 µs]
                        thrpt:  [1.0289 Melem/s 1.0466 Melem/s 1.0620 Melem/s]
                 change:
                        time:   [-74.784% -73.499% -72.480%] (p = 0.00 < 0.05)
                        thrpt:  [+263.37% +277.34% +296.57%]
                        Performance has improved.
Found 7 outliers among 100 measurements (7.00%)
  1 (1.00%) low mild
  4 (4.00%) high mild
  2 (2.00%) high severe
new_span/single_thread/50
                        time:   [10.237 µs 10.278 µs 10.323 µs]
                        thrpt:  [4.8433 Melem/s 4.8649 Melem/s 4.8842 Melem/s]
                 change:
                        time:   [-48.086% -47.936% -47.756%] (p = 0.00 < 0.05)
                        thrpt:  [+91.410% +92.072% +92.624%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  6 (6.00%) high mild
new_span/multithreaded/50
                        time:   [26.990 µs 27.319 µs 27.667 µs]
                        thrpt:  [1.8072 Melem/s 1.8302 Melem/s 1.8526 Melem/s]
                 change:
                        time:   [-74.114% -73.534% -72.993%] (p = 0.00 < 0.05)
                        thrpt:  [+270.27% +277.84% +286.30%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  1 (1.00%) low mild
  5 (5.00%) high mild

event/root/single_threaded/1
                        time:   [497.48 ns 498.85 ns 500.33 ns]
                        thrpt:  [1.9987 Melem/s 2.0046 Melem/s 2.0101 Melem/s]
                 change:
                        time:   [-58.535% -58.270% -57.920%] (p = 0.00 < 0.05)
                        thrpt:  [+137.64% +139.64% +141.17%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  4 (4.00%) high mild
  2 (2.00%) high severe
event/root/multithreaded/1
                        time:   [5.8425 µs 5.9341 µs 6.0291 µs]
                        thrpt:  [165.86 Kelem/s 168.52 Kelem/s 171.16 Kelem/s]
                 change:
                        time:   [-78.009% -76.339% -73.910%] (p = 0.00 < 0.05)
                        thrpt:  [+283.28% +322.64% +354.73%]
                        Performance has improved.
Found 4 outliers among 100 measurements (4.00%)
  2 (2.00%) low mild
  1 (1.00%) high mild
  1 (1.00%) high severe
event/unique_parent/single_threaded/1
                        time:   [641.67 ns 643.06 ns 644.60 ns]
                        thrpt:  [1.5513 Melem/s 1.5551 Melem/s 1.5584 Melem/s]
                 change:
                        time:   [-75.884% -75.830% -75.770%] (p = 0.00 < 0.05)
                        thrpt:  [+312.71% +313.74% +314.66%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  5 (5.00%) high mild
  1 (1.00%) high severe
event/unique_parent/multithreaded/1
                        time:   [7.1797 µs 7.2862 µs 7.4150 µs]
                        thrpt:  [134.86 Kelem/s 137.25 Kelem/s 139.28 Kelem/s]
                 change:
                        time:   [-78.334% -77.411% -76.535%] (p = 0.00 < 0.05)
                        thrpt:  [+326.17% +342.70% +361.56%]
                        Performance has improved.
Found 4 outliers among 100 measurements (4.00%)
  1 (1.00%) low mild
  3 (3.00%) high mild
event/shared_parent/multithreaded/1
                        time:   [7.4960 µs 7.6215 µs 7.7475 µs]
                        thrpt:  [129.07 Kelem/s 131.21 Kelem/s 133.40 Kelem/s]
                 change:
                        time:   [-77.724% -76.392% -75.098%] (p = 0.00 < 0.05)
                        thrpt:  [+301.58% +323.59% +348.92%]
                        Performance has improved.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild
event/multi-parent/multithreaded/1
                        time:   [10.216 µs 10.376 µs 10.548 µs]
                        thrpt:  [94.801 Kelem/s 96.376 Kelem/s 97.885 Kelem/s]
                 change:
                        time:   [-76.674% -75.840% -74.968%] (p = 0.00 < 0.05)
                        thrpt:  [+299.49% +313.91% +328.71%]
                        Performance has improved.
Found 12 outliers among 100 measurements (12.00%)
  2 (2.00%) low mild
  6 (6.00%) high mild
  4 (4.00%) high severe
event/root/single_threaded/10
                        time:   [4.8850 µs 4.9149 µs 4.9525 µs]
                        thrpt:  [2.0192 Melem/s 2.0346 Melem/s 2.0471 Melem/s]
                 change:
                        time:   [-58.129% -57.648% -57.100%] (p = 0.00 < 0.05)
                        thrpt:  [+133.10% +136.12% +138.83%]
                        Performance has improved.
event/root/multithreaded/10
                        time:   [13.448 µs 13.855 µs 14.321 µs]
                        thrpt:  [698.29 Kelem/s 721.77 Kelem/s 743.61 Kelem/s]
                 change:
                        time:   [-72.857% -71.089% -69.055%] (p = 0.00 < 0.05)
                        thrpt:  [+223.16% +245.89% +268.42%]
                        Performance has improved.
Found 5 outliers among 100 measurements (5.00%)
  4 (4.00%) high mild
  1 (1.00%) high severe
event/unique_parent/single_threaded/10
                        time:   [6.6395 µs 6.6628 µs 6.6878 µs]
                        thrpt:  [1.4953 Melem/s 1.5009 Melem/s 1.5061 Melem/s]
                 change:
                        time:   [-75.174% -75.093% -75.019%] (p = 0.00 < 0.05)
                        thrpt:  [+300.31% +301.49% +302.81%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  6 (6.00%) high mild
event/unique_parent/multithreaded/10
                        time:   [14.951 µs 15.161 µs 15.379 µs]
                        thrpt:  [650.25 Kelem/s 659.60 Kelem/s 668.86 Kelem/s]
                 change:
                        time:   [-81.822% -81.345% -80.810%] (p = 0.00 < 0.05)
                        thrpt:  [+421.11% +436.05% +450.12%]
                        Performance has improved.
Found 3 outliers among 100 measurements (3.00%)
  2 (2.00%) high mild
  1 (1.00%) high severe
event/shared_parent/multithreaded/10
                        time:   [18.349 µs 18.607 µs 18.881 µs]
                        thrpt:  [529.62 Kelem/s 537.44 Kelem/s 544.98 Kelem/s]
                 change:
                        time:   [-80.082% -79.607% -79.118%] (p = 0.00 < 0.05)
                        thrpt:  [+378.89% +390.36% +402.06%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  1 (1.00%) low mild
  4 (4.00%) high mild
  1 (1.00%) high severe
event/multi-parent/multithreaded/10
                        time:   [33.293 µs 33.730 µs 34.199 µs]
                        thrpt:  [292.40 Kelem/s 296.47 Kelem/s 300.36 Kelem/s]
                 change:
                        time:   [-83.615% -83.094% -82.570%] (p = 0.00 < 0.05)
                        thrpt:  [+473.72% +491.49% +510.33%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  5 (5.00%) high mild
  1 (1.00%) high severe
event/root/single_threaded/50
                        time:   [25.595 µs 25.671 µs 25.755 µs]
                        thrpt:  [1.9414 Melem/s 1.9477 Melem/s 1.9535 Melem/s]
                 change:
                        time:   [-57.180% -57.038% -56.893%] (p = 0.00 < 0.05)
                        thrpt:  [+131.98% +132.76% +133.54%]
                        Performance has improved.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild
event/root/multithreaded/50
                        time:   [37.453 µs 39.038 µs 40.700 µs]
                        thrpt:  [1.2285 Melem/s 1.2808 Melem/s 1.3350 Melem/s]
                 change:
                        time:   [-74.368% -73.402% -72.358%] (p = 0.00 < 0.05)
                        thrpt:  [+261.77% +275.97% +290.13%]
                        Performance has improved.
Found 9 outliers among 100 measurements (9.00%)
  7 (7.00%) high mild
  2 (2.00%) high severe
event/unique_parent/single_threaded/50
                        time:   [32.737 µs 32.868 µs 33.005 µs]
                        thrpt:  [1.5149 Melem/s 1.5212 Melem/s 1.5273 Melem/s]
                 change:
                        time:   [-75.443% -75.361% -75.273%] (p = 0.00 < 0.05)
                        thrpt:  [+304.41% +305.86% +307.22%]
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  6 (6.00%) high mild
event/unique_parent/multithreaded/50
                        time:   [47.636 µs 48.315 µs 49.058 µs]
                        thrpt:  [1.0192 Melem/s 1.0349 Melem/s 1.0496 Melem/s]
                 change:
                        time:   [-83.557% -82.962% -82.335%] (p = 0.00 < 0.05)
                        thrpt:  [+466.11% +486.91% +508.18%]
                        Performance has improved.
Found 2 outliers among 100 measurements (2.00%)
  1 (1.00%) low mild
  1 (1.00%) high mild
event/shared_parent/multithreaded/50
                        time:   [67.386 µs 68.607 µs 69.865 µs]
                        thrpt:  [715.67 Kelem/s 728.79 Kelem/s 742.00 Kelem/s]
                 change:
                        time:   [-82.195% -81.690% -81.210%] (p = 0.00 < 0.05)
                        thrpt:  [+432.20% +446.14% +461.64%]
                        Performance has improved.
Found 4 outliers among 100 measurements (4.00%)
  3 (3.00%) low mild
  1 (1.00%) high mild
event/multi-parent/multithreaded/50
                        time:   [268.57 µs 272.25 µs 275.93 µs]
                        thrpt:  [181.21 Kelem/s 183.65 Kelem/s 186.17 Kelem/s]
                 change:
                        time:   [-89.114% -88.884% -88.642%] (p = 0.00 < 0.05)
                        thrpt:  [+780.41% +799.63% +818.58%]
                        Performance has improved.
Found 10 outliers among 100 measurements (10.00%)
  3 (3.00%) low severe
  3 (3.00%) low mild
  4 (4.00%) high mild
```
