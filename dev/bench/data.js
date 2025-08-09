window.BENCHMARK_DATA = {
  "lastUpdate": 1754753575502,
  "repoUrl": "https://github.com/mladedav/json-subscriber",
  "entries": {
    "Rust Benchmark (1.89.0)": [
      {
        "commit": {
          "author": {
            "name": "mladedav",
            "username": "mladedav"
          },
          "committer": {
            "name": "mladedav",
            "username": "mladedav"
          },
          "id": "35cb79819c07e2901a9821e7a466153b0cd64b8d",
          "message": "ci: add automatic benchmark runs",
          "timestamp": "2025-08-08T11:36:57Z",
          "url": "https://github.com/mladedav/json-subscriber/pull/29/commits/35cb79819c07e2901a9821e7a466153b0cd64b8d"
        },
        "date": 1754753574995,
        "tool": "cargo",
        "benches": [
          {
            "name": "new_span/single_thread/1",
            "value": 289,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "new_span/multithreaded/1",
            "value": 27190,
            "range": "± 1201",
            "unit": "ns/iter"
          },
          {
            "name": "new_span/single_thread/10",
            "value": 2862,
            "range": "± 60",
            "unit": "ns/iter"
          },
          {
            "name": "new_span/multithreaded/10",
            "value": 29430,
            "range": "± 1105",
            "unit": "ns/iter"
          },
          {
            "name": "new_span/single_thread/50",
            "value": 14353,
            "range": "± 50",
            "unit": "ns/iter"
          },
          {
            "name": "new_span/multithreaded/50",
            "value": 54600,
            "range": "± 1832",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/single_threaded/1",
            "value": 810,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/multithreaded/1",
            "value": 26390,
            "range": "± 891",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/single_threaded/1",
            "value": 1119,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/multithreaded/1",
            "value": 28960,
            "range": "± 817",
            "unit": "ns/iter"
          },
          {
            "name": "event/shared_parent/multithreaded/1",
            "value": 27260,
            "range": "± 1635",
            "unit": "ns/iter"
          },
          {
            "name": "event/multi-parent/multithreaded/1",
            "value": 31037,
            "range": "± 1325",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/single_threaded/10",
            "value": 8088,
            "range": "± 46",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/multithreaded/10",
            "value": 40328,
            "range": "± 1464",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/single_threaded/10",
            "value": 11219,
            "range": "± 52",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/multithreaded/10",
            "value": 47782,
            "range": "± 1364",
            "unit": "ns/iter"
          },
          {
            "name": "event/shared_parent/multithreaded/10",
            "value": 45754,
            "range": "± 2012",
            "unit": "ns/iter"
          },
          {
            "name": "event/multi-parent/multithreaded/10",
            "value": 74959,
            "range": "± 2632",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/single_threaded/50",
            "value": 40885,
            "range": "± 701",
            "unit": "ns/iter"
          },
          {
            "name": "event/root/multithreaded/50",
            "value": 105725,
            "range": "± 4205",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/single_threaded/50",
            "value": 55746,
            "range": "± 170",
            "unit": "ns/iter"
          },
          {
            "name": "event/unique_parent/multithreaded/50",
            "value": 133803,
            "range": "± 4686",
            "unit": "ns/iter"
          },
          {
            "name": "event/shared_parent/multithreaded/50",
            "value": 126199,
            "range": "± 6988",
            "unit": "ns/iter"
          },
          {
            "name": "event/multi-parent/multithreaded/50",
            "value": 423251,
            "range": "± 23033",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}