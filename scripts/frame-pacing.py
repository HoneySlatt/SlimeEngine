#!/usr/bin/env python3
"""Reads a trace of a run (SLIME_TRACE, in the Chrome Trace Event format) and prints how its frames
were paced: their rate, and the intervals between the starts of the main thread's frames, their
mean, standard deviation, median, 99th percentile and longest
(docs/adr/0187-a-hidden-window-at-rest.md); and, when a person's input reached the window, the
intervals from each input to the present of the image after it, their median, 99th percentile and
longest, in milliseconds and in frames of the budget (docs/adr/0189-first-measures-of-time.md).

    SLIME_TRACE=trace.json cargo run --release --example window -- --frames 600
    scripts/frame-pacing.py trace.json

The first frames, while the window and the GPU settle, are left out: `--skip <count>`, 10 by
default. Input to present is counted in frames of the editor's budget, unless `--frame
<milliseconds>` gives a game's (docs/ROADMAP.md, section 4, "Input to present").
"""

import argparse
import json
import math
import statistics
import sys


def events_of(path):
    """The events of the trace."""
    with open(path, encoding="utf-8") as file:
        text = file.read().rstrip().rstrip(",")
    # The trace is written as the process runs, without its closing bracket.
    if not text.endswith("]"):
        text += "]"
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        sys.exit(f"{path}: not a whole trace, as one cut off by a process killed: {error}")


def frame_starts(events):
    """The times, in milliseconds, at which the main thread's frames start."""
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


def inputs_to_present(events):
    """The intervals, in milliseconds, from each input to the present of the image after it."""
    return sorted(
        event["dur"] / 1000
        for event in events
        if event.get("name") == "input to present" and event.get("ph") == "X"
    )


def percentile(values, share):
    """The value of sorted `values` at or below which `share` of them lie, by the nearest rank: the
    largest of fewer than 100 values for the 99th percentile."""
    return values[max(0, math.ceil(share * len(values)) - 1)]


def in_frames(value, frame):
    """`value`, in milliseconds, and in frames of `frame` milliseconds."""
    return f"{value:.3f} ms, {value / frame:.2f} frames"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("trace")
    parser.add_argument("--skip", type=int, default=10)
    parser.add_argument("--frame", type=float, default=1000 / 240)
    args = parser.parse_args()
    events = events_of(args.trace)
    starts = frame_starts(events)[args.skip :]
    inputs = inputs_to_present(events)
    if len(starts) < 3 and not inputs:
        sys.exit(f"{args.trace}: {len(starts)} frames after the first {args.skip}: too few")
    if len(starts) >= 3:
        intervals = sorted(b - a for a, b in zip(starts, starts[1:]))
        span = (starts[-1] - starts[0]) / 1000
        print(f"frames: {len(starts)} in {span:.2f} s, {(len(starts) - 1) / span:.2f} a second")
        print(
            f"intervals: mean {statistics.mean(intervals):.3f} ms, "
            f"standard deviation {statistics.stdev(intervals):.3f} ms, "
            f"median {statistics.median(intervals):.3f} ms, "
            f"99th percentile {percentile(intervals, 0.99):.3f} ms, "
            f"longest {intervals[-1]:.3f} ms"
        )
    if inputs:
        print(f"input to present: {len(inputs)} images after an input, frames of {args.frame:.2f} ms")
        print(
            f"  median {in_frames(statistics.median(inputs), args.frame)}; "
            f"99th percentile {in_frames(percentile(inputs, 0.99), args.frame)}; "
            f"longest {in_frames(inputs[-1], args.frame)}"
        )


if __name__ == "__main__":
    main()
