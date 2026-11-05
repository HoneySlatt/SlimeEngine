#!/usr/bin/env python3
"""Reads a trace of a run (SLIME_TRACE, in the Chrome Trace Event format) and prints how its frames
were paced: their rate, and the intervals between the starts of the main thread's frames, their
mean, standard deviation, median, 99th percentile and longest
(docs/adr/0187-a-hidden-window-at-rest.md).

    SLIME_TRACE=trace.json cargo run --release --example window -- --frames 600
    scripts/frame-pacing.py trace.json

The first frames, while the window and the GPU settle, are left out: `--skip <count>`, 10 by
default.
"""

import argparse
import json
import statistics
import sys


def frame_starts(path):
    """The times, in milliseconds, at which the main thread's frames start."""
    with open(path, encoding="utf-8") as file:
        text = file.read().rstrip().rstrip(",")
    # The trace is written as the process runs, without its closing bracket.
    if not text.endswith("]"):
        text += "]"
    try:
        events = json.loads(text)
    except json.JSONDecodeError as error:
        sys.exit(f"{path}: not a whole trace, as one cut off by a process killed: {error}")
    # Threads are numbered in the order they first enter a span: the main thread by its name.
    main = {
        event["tid"]
        for event in events
        if event.get("name") == "thread_name" and event.get("args", {}).get("name") == "main"
    }
    return [
        event["ts"] / 1000
        for event in events
        if event.get("name") == "frame" and event.get("ph") == "B" and event.get("tid") in main
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("trace")
    parser.add_argument("--skip", type=int, default=10)
    args = parser.parse_args()
    starts = frame_starts(args.trace)[args.skip :]
    if len(starts) < 3:
        sys.exit(f"{args.trace}: {len(starts)} frames after the first {args.skip}: too few")
    intervals = sorted(b - a for a, b in zip(starts, starts[1:]))
    span = (starts[-1] - starts[0]) / 1000
    print(f"frames: {len(starts)} in {span:.2f} s, {(len(starts) - 1) / span:.2f} a second")
    print(
        f"intervals: mean {statistics.mean(intervals):.3f} ms, "
        f"standard deviation {statistics.stdev(intervals):.3f} ms, "
        f"median {statistics.median(intervals):.3f} ms, "
        f"99th percentile {intervals[int(0.99 * (len(intervals) - 1))]:.3f} ms, "
        f"longest {intervals[-1]:.3f} ms"
    )


if __name__ == "__main__":
    main()
