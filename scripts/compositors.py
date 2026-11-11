#!/usr/bin/env python3
"""Reads what scripts/compositors.sh gathered in one compositor of the series, and compares it with
the table of docs/compositors.md (docs/adr/0195-a-series-of-headless-compositors.md).

    scripts/compositors.py protocols <wayland-info's output>
    scripts/compositors.py trace <a program's log, written with WAYLAND_DEBUG=client>
    scripts/compositors.py compare <docs/compositors.md> <compositor> <observations>

`protocols` and `trace` print observations, one a line, `<row>: <value>`, each row named as the
table names it; `compare` checks each observation against the compositor's column of the table,
and fails on the first that differs, which then needs the table, or the code, changed.
"""

import re
import sys

# A request or an event that libwayland prints for WAYLAND_DEBUG: `-> ` before a request, then the
# object's interface, its id, and the message with its arguments.
MESSAGE = re.compile(
    r"(?P<request>-> )?(?P<interface>\w+)#?@?(?P<id>\d+)\.(?P<message>\w+)\((?P<arguments>.*)\)"
)


def protocols(path):
    """The presentation clock and the output's refresh, from wayland-info's output."""
    with open(path, encoding="utf-8") as file:
        text = file.read()
    clock = re.search(r"presentation clock id: \d+ \((\w+)\)", text)
    print(f"Presentation times: {clock.group(1) if clock else 'none'}")
    refresh = re.search(r"refresh: ([\d.]+) Hz", text)
    print(f"Output's refresh: {round(float(refresh.group(1))) if refresh else 'none'} Hz")


def trace(path):
    """How the program used the compositor's protocols, from its log."""
    requests, events = [], []
    bound = set()
    with open(path, encoding="utf-8", errors="replace") as file:
        for line in file:
            message = MESSAGE.search(line)
            if not message:
                continue
            (requests if message["request"] else events).append(message)
            if message["interface"] == "wl_registry" and message["message"] == "bind":
                bound.add(message["arguments"].split('"')[1] if '"' in message["arguments"] else "")

    modes = [
        m["arguments"]
        for m in events
        if m["interface"] == "zxdg_toplevel_decoration_v1" and m["message"] == "configure"
    ]
    if "zxdg_decoration_manager_v1" not in bound:
        decorations = "the program's, no protocol"
    elif modes and modes[-1].strip() == "2":
        decorations = "the compositor's"
    else:
        decorations = "the program's, asked"
    print(f"Decorations: {decorations}")

    print(
        f"Fractional scale's protocol: {'used' if 'wp_fractional_scale_manager_v1' in bound else 'absent'}"
    )

    shapes = [
        m
        for m in requests
        if m["interface"] == "wp_cursor_shape_device_v1" and m["message"] == "set_shape"
    ]
    images = [
        m
        for m in requests
        if m["interface"] == "wl_pointer"
        and m["message"] == "set_cursor"
        and "wl_surface" in m["arguments"]
    ]
    entered = [m for m in events if m["interface"] == "wl_pointer" and m["message"] == "enter"]
    if shapes:
        cursor = "by shape"
    elif images:
        cursor = "by image"
    elif entered:
        cursor = "none set"
    else:
        cursor = "not run, the pointer never entered"
    print(f"Cursor: {cursor}")

    feedback = any(
        m["interface"] == "wp_presentation" and m["message"] == "feedback" for m in requests
    )
    print(f"Presentation feedback asked: {'yes' if feedback else 'no'}")

    if "ext_data_control_manager_v1" in bound:
        control = "ext"
    elif "zwlr_data_control_manager_v1" in bound:
        control = "wlr"
    else:
        control = "none"
    print(f"Clipboard's protocol: {control}")


def table(path):
    """The table of docs/compositors.md whose header names the compositors: its rows by their
    first cell, each a dictionary of its cells by the compositors' names."""
    rows, header = {}, None
    with open(path, encoding="utf-8") as file:
        for line in file:
            if not line.startswith("|"):
                if header and rows:
                    break
                continue
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if header is None:
                if "Weston" in cells:
                    header = [cell.lower().split()[0] if cell else "" for cell in cells]
                continue
            if set(cells[0]) <= set("-: "):
                continue
            rows[cells[0].strip("*")] = dict(zip(header, cells))
    return rows


def compare(path, compositor, observations):
    """Compares each observation with the table's cell, and fails if one differs."""
    rows = table(path)
    differ = []
    with open(observations, encoding="utf-8") as file:
        for line in file:
            row, _, value = line.rstrip("\n").partition(": ")
            if row not in rows:
                differ.append(f"{row}, which the table has no row for")
                continue
            expected = rows[row].get(compositor, "")
            # A cell that the series does not observe: what it says is read elsewhere.
            if expected.startswith("—"):
                continue
            if expected == value:
                print(f"{row}: {value} (same)")
            else:
                print(f"{row}: {value} (DIFFERS: the table says {expected!r})")
                differ.append(row)
    if differ:
        sys.exit(f"{compositor} differs from {path} in: {', '.join(differ)}")


def main():
    command = sys.argv[1:2]
    if command == ["protocols"] and len(sys.argv) == 3:
        protocols(sys.argv[2])
    elif command == ["trace"] and len(sys.argv) == 3:
        trace(sys.argv[2])
    elif command == ["compare"] and len(sys.argv) == 5:
        compare(*sys.argv[2:])
    else:
        sys.exit(__doc__.split("\n\n")[1])


if __name__ == "__main__":
    main()
