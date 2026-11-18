#!/usr/bin/env python3
"""Measures uwui's desktop host's own cost
(docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md):
three programs run in turn in one headless sway of their own, its output at 1920 × 1080 and
240 Hz: the *blank window* on the host's platform layer (`uwui-desktop`'s example `blank`), the
same on winit (`uwui-blank-winit`), and the host's example `application`. A blank window only
clears its window and presents it, again at each input; what the example costs above the first is
the host's and the interface's own.

    scripts/host-cost.py [--gpu] [--starts <count>] [--keep <directory>]

For each program, built in release, it measures:

- its start, from the moment the tool launches it to the end of its first image's present, as its
  trace says (UWUI_TRACE, uwui_desktop::TRACE_ENV_VAR): the median of `--starts` runs, 10 by
  default, after one that warms the system's caches;
- in one run after them, once it rests: its memory, the proportional size of its pages (Pss), its
  resident size and its peak, and its video memory where the driver counts it;
- its rest, over 10 s without input: the frames that its trace shows, every thread's wake-ups, from
  the switches off its core that Linux counts, and its time on the processor;
- its input to present: from the moment the tool sends a notch of the wheel to the end of the
  present of the first image that took it, each image's notches counted in its trace, the oldest
  notch of each image counted. Inputs apart, 100 notches 100 ms apart, the program at rest between
  them, each of which must have an image of its own; inputs in a row, 1,000 notches 180 a second,
  fewer than the display's refreshes, so that the program may rest a little between them; and
  inputs in motion, 1,000 notches 400 a second, more than the display's refreshes, so that every
  refresh finds an input waiting and the frames run without a rest. The notches go down and up in
  turn over the window's list, so that each changes what the three programs show. The same measure
  for the three programs, which an input that waits behind a long frame does not escape;
- its input to shown: the same, to when the first image at or after the one that took it showed,
  as its trace says where its platform tells it (`shown`, from `wp_presentation`); winit's program
  tells none. And the frames that missed the refresh that their time was predicted for (`missed`),
  which only the host traces;
- its binary's size, as built and stripped.

The input reaches the window through the compositor, as a person's does, through the virtual
pointer that wlroots gives, which scripts/compositor-input.py speaks, inside this compositor alone,
never the desktop's. Each program runs with a runtime, state and data directory of its own: nothing
reaches the desktop nor the user's files. Drawn with lavapipe, as the checks are; with `--gpu`, on
the machine's GPU, through sway's GL renderer (CONTRIBUTING.md, "Measuring").

The machine's load is read before the build, and while each program is measured: the processor's
cores that other programs used, a process's children that ended counted in it; and the GPU's busy
share, other programs' alone with lavapipe, and on the GPU while the measured program rests. A program that other work
disturbed, above half a core or 10 % of the GPU, is marked so in the table: take it again on a
machine at rest. It prints a section of
docs/measurements.md; the traces and logs stay in `--keep`'s directory, target/host-cost/<time> by
default. Run it in the development shell.
"""

import argparse
import glob
import importlib.util
import json
import os
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / "target" / "release"

# The programs, in the order they run: their name in the table, and their binary.
PROGRAMS = [
    ("blank, uwui's layer", RELEASE / "examples" / "blank"),
    ("blank, winit", RELEASE / "uwui-blank-winit"),
    ("application", RELEASE / "examples" / "application"),
]

# The quality bar's window of rest (docs/adr/0185-rest-checked-in-ci.md).
REST = 10.0
# How long a program stays without a wake-up before it counts as at rest.
STILL = 1.0
# How long a program may take to show its first image, or to come to rest: lavapipe draws on the
# processor.
SETTLE = 60.0
# Where the wheel turns: in the output's middle across, below its middle down, over the example's
# list, which fills the window under its bar.
OVER_THE_LIST = (960, 600)
# The notches of the wheel in a row, a second: a mouse's wheel turned fast.
IN_A_ROW = 180
# The notches of the wheel in motion, a second: more than the display's 240 refreshes.
IN_MOTION = 400
# Above these, other programs disturbed a measure: cores of the processor, the GPU's busy share.
DISTURBING_CORES = 0.5
DISTURBING_GPU = 10.0


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def on_signal(number, _frame):
    """A termination signal ends the tool through its cleanups."""
    sys.exit(128 + number)


def script(name):
    """A module of scripts/, read without a cache beside it."""
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# The nearest rank's percentile of sorted values, the engine's measures' own.
percentile = script("frame-pacing.py").percentile


def arguments():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--gpu", action="store_true", help="draw on the machine's GPU")
    parser.add_argument("--starts", type=int, default=10, help="starts timed per program")
    parser.add_argument("--keep", type=Path, help="where the traces and logs stay")
    given = parser.parse_args()
    if given.starts < 1:
        parser.error("--starts must be 1 or more")
    return given


def build():
    """Builds the three programs in release."""
    for command in (
        ["-p", "uwui-desktop", "--example", "blank", "--example", "application"],
        ["-p", "uwui-blank-winit"],
    ):
        subprocess.run(["cargo", "build", "--locked", "--release", *command], cwd=ROOT, check=True)


def gpu_busy():
    """The GPU's busy share now, in percent, the busiest card's where the driver gives it."""
    shares = []
    for path in glob.glob("/sys/class/drm/card*/device/gpu_busy_percent"):
        try:
            shares.append(float(Path(path).read_text(encoding="ascii")))
        except (OSError, ValueError):
            pass
    return max(shares) if shares else None


def processes():
    """Each process's parent and its time on the processor so far, in clock ticks, its children's
    that ended included, so that a build's short compilers count in the build that waits for them."""
    found = {}
    for stat in glob.glob("/proc/[0-9]*/stat"):
        try:
            fields = Path(stat).read_text(encoding="ascii").rsplit(")", 1)[1].split()
        except (OSError, IndexError):
            continue
        # After the name: the state, the parent, ..., `utime`, `stime`, `cutime` and `cstime`, the
        # 12th to the 15th.
        found[int(Path(stat).parent.name)] = (
            int(fields[1]),
            sum(int(ticks) for ticks in fields[11:15]),
        )
    return found


def others_ticks():
    """The processor's time so far, in clock ticks, of each process outside this tool's tree."""
    found = processes()
    ours = {os.getpid()}
    grew = True
    while grew:
        grew = False
        for pid, (parent, _) in found.items():
            if parent in ours and pid not in ours:
                ours.add(pid)
                grew = True
    return {pid: ticks for pid, (_, ticks) in found.items() if pid not in ours}


class Load:
    """What other programs put on the machine while a program is measured."""

    def __init__(self):
        self.started = time.monotonic()
        self.ticks = others_ticks()
        # The GPU's busy shares sampled, each with whether the measured program rested then.
        self.shares = []
        self.resting = False
        self.done = threading.Event()
        self.sampler = threading.Thread(target=self.sample, daemon=True)
        self.sampler.start()

    def sample(self):
        while not self.done.wait(0.5):
            share = gpu_busy()
            if share is not None:
                self.shares.append((share, self.resting))

    def end(self, gpu):
        """The cores that other programs used, the GPU's mean busy share, and whether they
        disturbed the measure. With lavapipe, the GPU is other programs' alone; on the GPU, only
        its share while the measured program rests, and sway with it, is theirs."""
        self.done.set()
        self.sampler.join()
        elapsed = time.monotonic() - self.started
        # What each process used meanwhile, a new one all of its time; one that ended meanwhile is
        # not counted.
        used = sum(
            max(0, ticks - self.ticks.get(pid, 0)) for pid, ticks in others_ticks().items()
        )
        cores = used / os.sysconf("SC_CLK_TCK") / elapsed
        theirs = [share for share, resting in self.shares if resting or not gpu]
        share = statistics.fmean(theirs) if theirs else None
        disturbed = cores > DISTURBING_CORES or (share is not None and share > DISTURBING_GPU)
        return {"cores": cores, "gpu": share, "disturbed": disturbed}


class Sway:
    """A headless sway of its own, its output at 240 Hz, in a runtime directory of its own."""

    def __init__(self, work, gpu):
        self.runtime = Path(tempfile.mkdtemp(prefix="host-cost-"))
        self.runtime.chmod(0o700)
        self.process = None
        self.log = open(work / "sway.log", "w", encoding="utf-8")
        config = work / "sway.config"
        # No Xwayland, which would write into the system's /tmp/.X11-unix.
        config.write_text(
            "output HEADLESS-1 mode --custom 1920x1080@240Hz\n"
            "default_border none\nxwayland disable\n",
            encoding="utf-8",
        )
        environment = {
            key: value
            for key, value in os.environ.items()
            if key not in ("WAYLAND_DISPLAY", "DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "SWAYSOCK")
        }
        environment.update(
            XDG_RUNTIME_DIR=str(self.runtime),
            WLR_BACKENDS="headless",
            WLR_LIBINPUT_NO_DEVICES="1",
            WLR_RENDERER="gles2" if gpu else "pixman",
        )
        # Its own group, which is stopped whole: nixpkgs' sway is a wrapper.
        self.process = subprocess.Popen(
            ["sway", "-c", str(config)],
            env=environment,
            stdout=self.log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            self.ready(work)
        except BaseException:
            self.stop()
            raise

    def ready(self, work):
        """Waits for its sockets and its output at 240 Hz."""
        self.display = self.socket("wayland-*")
        self.ipc = self.runtime / self.socket("sway-ipc.*.sock")
        deadline = time.monotonic() + 5
        refresh = None
        while time.monotonic() < deadline:
            try:
                outputs = json.loads(self.message("-t", "get_outputs", "-r"))
                refresh = outputs[0]["current_mode"]["refresh"]
            except (subprocess.CalledProcessError, json.JSONDecodeError, IndexError, KeyError):
                pass
            if refresh == 240000:
                break
            time.sleep(0.1)
        if refresh != 240000:
            fail(f"sway's output refreshes at {refresh} mHz, not 240 Hz: see {work}/sway.log")

    def socket(self, pattern):
        """The name of the first socket in the runtime directory that `pattern` matches: its lock
        file comes first, and is no socket."""
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                fail(f"sway ended at its start: see {self.log.name}")
            found = sorted(path for path in self.runtime.glob(pattern) if path.is_socket())
            if found:
                return found[0].name
            time.sleep(0.05)
        fail(f"sway made no {pattern} within 10 s")
        return None

    def environment(self):
        """What a program and the input devices run with inside this compositor."""
        return dict(XDG_RUNTIME_DIR=str(self.runtime), WAYLAND_DISPLAY=self.display)

    def message(self, *words):
        return subprocess.run(
            ["swaymsg", "-s", str(self.ipc), *words],
            check=True,
            capture_output=True,
            text=True,
        ).stdout

    def stop(self):
        if self.process is not None:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait()
        self.log.close()
        shutil.rmtree(self.runtime, ignore_errors=True)


def program_environment(sway, work, name, gpu):
    """The environment of a program's run: the compositor's, and directories of its own."""
    environment = {
        key: value
        for key, value in os.environ.items()
        if key
        not in (
            "WAYLAND_DISPLAY",
            "DISPLAY",
            "DBUS_SESSION_BUS_ADDRESS",
            "SLIME_LOG",
            "VK_LOADER_DRIVERS_SELECT",
            "VK_LOADER_DRIVERS_DISABLE",
            "UWUI_TRACE",
        )
    }
    environment.update(sway.environment())
    own = work / "directories" / name.replace(" ", "-").replace(",", "").replace("'", "")
    for kind in ("state", "data", "config", "cache"):
        (own / kind).mkdir(parents=True, exist_ok=True)
    environment.update(
        XDG_STATE_HOME=str(own / "state"),
        XDG_DATA_HOME=str(own / "data"),
        XDG_CONFIG_HOME=str(own / "config"),
        XDG_CACHE_HOME=str(own / "cache"),
    )
    if gpu:
        environment.pop("VK_DRIVER_FILES", None)
    else:
        environment["VK_DRIVER_FILES"] = os.environ["SLIME_LAVAPIPE_ICD"]
    return environment


def events(path):
    """The complete events of a trace, which is written without its closing bracket; a line that
    the program is still writing is left out."""
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return []
    complete = []
    for line in text.split("\n")[1:]:
        try:
            complete.append(json.loads(line.rstrip(",")))
        except json.JSONDecodeError:
            pass
    return [event for event in complete if event.get("ph") in ("X", "i")]


def frames(trace):
    return sorted(
        (event for event in trace if event["name"] == "frame"), key=lambda event: event["ts"]
    )


class Run:
    """A program running in the compositor, traced."""

    def __init__(self, binary, environment, trace, log):
        self.name = binary.name
        self.trace = trace
        trace.unlink(missing_ok=True)
        environment = dict(environment, UWUI_TRACE=str(trace))
        self.log = open(log, "w", encoding="utf-8")
        self.launched = time.time_ns()
        self.process = subprocess.Popen(
            [str(binary)], env=environment, stdout=self.log, stderr=subprocess.STDOUT
        )

    def first_image(self):
        """When its first image's present ended, in nanoseconds since the Unix epoch."""
        deadline = time.monotonic() + SETTLE
        while time.monotonic() < deadline:
            shown = frames(events(self.trace))
            if shown:
                return round((shown[0]["ts"] + shown[0]["dur"]) * 1000)
            if self.process.poll() is not None:
                fail(f"{self.name} ended before its first image: see {self.log.name}")
            time.sleep(0.002)
        fail(f"{self.name} showed no image within {SETTLE:.0f} s: see {self.log.name}")
        return None

    def threads(self):
        """Each thread's switches off its core and its time on the processor, in nanoseconds."""
        counted = {}
        for task in glob.glob(f"/proc/{self.process.pid}/task/*"):
            try:
                status = Path(task, "status").read_text(encoding="ascii")
                on_core = int(Path(task, "schedstat").read_text(encoding="ascii").split()[0])
            except (OSError, ValueError, IndexError):
                # A thread that ends as it is read.
                continue
            switches = sum(
                int(line.split()[1])
                for line in status.splitlines()
                if line.startswith(("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"))
            )
            counted[Path(task).name] = (switches, on_core)
        if not counted:
            fail(f"{self.name}'s threads cannot be read: see {self.log.name}")
        return counted

    def settle(self):
        """Waits until no thread wakes for `STILL`, for at most `SETTLE`."""
        deadline = time.monotonic() + SETTLE
        before = self.threads()
        while time.monotonic() < deadline:
            time.sleep(STILL)
            now = self.threads()
            if now == before:
                return
            before = now
        fail(f"{self.name} did not come to rest within {SETTLE:.0f} s")

    def memory(self):
        """Its proportional and resident sizes and its peak, in MB, and its video memory where
        the driver counts it, each client of the driver once."""
        rollup = Path(f"/proc/{self.process.pid}/smaps_rollup").read_text(encoding="ascii")
        status = Path(f"/proc/{self.process.pid}/status").read_text(encoding="ascii")

        def kilobytes(text, key):
            for line in text.splitlines():
                if line.startswith(key + ":"):
                    return int(line.split()[1])
            return 0

        video = {}
        for fdinfo in glob.glob(f"/proc/{self.process.pid}/fdinfo/*"):
            try:
                text = Path(fdinfo).read_text(encoding="ascii")
            except OSError:
                continue
            client = None
            amount = None
            for line in text.splitlines():
                if line.startswith("drm-client-id:"):
                    client = line.split()[1]
                elif line.startswith(("drm-memory-vram:", "drm-total-vram:")):
                    value, unit = line.split()[1:3]
                    amount = int(value) * {"KiB": 1, "MiB": 1024, "GiB": 1024 * 1024}.get(unit, 0)
            if client is not None and amount is not None:
                video[client] = amount
        return {
            "pss": kilobytes(rollup, "Pss") / 1024,
            "rss": kilobytes(rollup, "Rss") / 1024,
            "peak": kilobytes(status, "VmHWM") / 1024,
            "video": sum(video.values()) / 1024 if video else None,
        }

    def rest(self):
        """Over `REST` without input: the frames drawn, every thread's wake-ups a second, and its
        share of a core."""
        before, frames_before = self.threads(), len(frames(events(self.trace)))
        started = time.monotonic()
        time.sleep(REST)
        after, elapsed = self.threads(), time.monotonic() - started
        frames_after = len(frames(events(self.trace)))
        woke = sum(
            switches - before.get(thread, (0, 0))[0] for thread, (switches, _) in after.items()
        )
        on_core = sum(ran - before.get(thread, (0, 0))[1] for thread, (_, ran) in after.items())
        return {
            "frames": frames_after - frames_before,
            "wakes": woke / elapsed,
            "core": on_core / (elapsed * 1e9) * 100,
        }

    def stop(self):
        """Ends it by SIGTERM; winit's program, which takes none, ends by it."""
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.log.close()


def wheel(devices, notches, every):
    """Turns the wheel `notches` times, `every` seconds apart, down and up in turn: when each notch
    was sent, in nanoseconds since the Unix epoch."""
    sent = []
    due = time.monotonic()
    for notch in range(notches):
        sent.append(time.time_ns())
        devices.scroll(1 if notch % 2 == 0 else -1)
        due += every
        time.sleep(max(0.0, due - time.monotonic()))
    # The last notch's image comes within a frame or two.
    time.sleep(0.2)
    return sent


def shown_times(trace):
    """When each image showed, by its number, in nanoseconds since the Unix epoch."""
    return {
        event["args"]["image"]: round(event["ts"] * 1000)
        for event in trace
        if event["name"] == "shown"
    }


def inputs_to_present(trace, sent, name):
    """For each image that took notches of the wheel, the interval in milliseconds from its oldest
    notch's sending to the end of its present, and to when the first image at or after it showed,
    none where the trace tells no image shown, with that notch's index in `sent`: the notches are
    taken in the order they were sent, which each frame's count follows."""
    intervals = []
    shown = shown_times(trace)
    # For each frame, when the first image at or after its own showed: an image never shown is
    # replaced by the next that is.
    images = [frame.get("args", {}).get("image") for frame in frames(trace)]
    first_shown = [None] * len(images)
    later = None
    for index in range(len(images) - 1, -1, -1):
        later = shown.get(images[index], later)
        first_shown[index] = later
    taken = 0
    # Notches that one image took together, which the notches apart must never be.
    together = []
    for index, frame in enumerate(frames(trace)):
        count = frame.get("args", {}).get("wheel", 0)
        if count == 0:
            continue
        if taken + count > len(sent):
            fail(f"{name}'s trace takes {taken + count} notches of the wheel, more than the "
                 f"{len(sent)} sent")
        end = round((frame["ts"] + frame["dur"]) * 1000)
        to_shown = None if first_shown[index] is None else (first_shown[index] - sent[taken]) / 1e6
        intervals.append((taken, (end - sent[taken]) / 1e6, to_shown))
        if count > 1:
            together.append(taken)
        taken += count
    if taken != len(sent):
        fail(f"{name}'s trace took {taken} of the {len(sent)} notches of the wheel sent")
    return intervals, together


def measure(name, binary, sway, devices, work, given):
    """Every figure of one program."""
    environment = program_environment(sway, work, name, given.gpu)
    slug = binary.name
    load = Load()
    starts = []
    # The first warms the system's caches.
    for start in range(given.starts + 1):
        run = Run(binary, environment, work / f"{slug}-start-{start}.json",
                  work / f"{slug}-start-{start}.log")
        try:
            shown = run.first_image()
        finally:
            run.stop()
        if start > 0:
            starts.append((shown - run.launched) / 1e6)
    run = Run(binary, environment, work / f"{slug}.json", work / f"{slug}.log")
    try:
        run.first_image()
        run.settle()
        memory = run.memory()
        load.resting = True
        rest = run.rest()
        load.resting = False
        devices.move_to(*OVER_THE_LIST)
        run.settle()
        apart = wheel(devices, 100, 0.1)
        run.settle()
        in_a_row = wheel(devices, 1000, 1 / IN_A_ROW)
        run.settle()
        in_motion = wheel(devices, 1000, 1 / IN_MOTION)
        if run.process.poll() is not None:
            fail(f"{name} ended before its inputs did: see {run.log.name}")
    finally:
        run.stop()
    trace = events(run.trace)
    intervals, together = inputs_to_present(trace, apart + in_a_row + in_motion, name)
    # Each notch apart has an image of its own: else one went unanswered, the wheel turning where
    # the program shows nothing that it moves.
    if any(index < len(apart) for index in together):
        fail(f"{name} took notches apart, 100 ms from each other, in one image: one went "
             f"unanswered; see {run.trace}")
    bounds = {
        "apart": (0, len(apart)),
        "in a row": (len(apart), len(apart) + len(in_a_row)),
        "in motion": (len(apart) + len(in_a_row), len(apart) + len(in_a_row) + len(in_motion)),
    }

    def scenario(which, to_shown):
        low, high = bounds[which]
        found = [
            (shown if to_shown else present)
            for index, present, shown in intervals
            if low <= index < high
        ]
        if not to_shown:
            return found
        # A program whose platform tells no image shown has no such figure; an image that the
        # platform did not tell, as one discarded with no later image, is left out.
        told = [interval for interval in found if interval is not None]
        return told or None

    size = binary.stat().st_size
    stripped = work / f"{slug}.stripped"
    subprocess.run(["strip", "-o", str(stripped), str(binary)], check=True)
    return {
        "start": starts,
        "memory": memory,
        "rest": rest,
        **{which: scenario(which, False) for which in bounds},
        **{f"{which} shown": scenario(which, True) for which in bounds},
        "binary": (size / 1e6, stripped.stat().st_size / 1e6),
        "frame work": [event["dur"] / 1000 for event in frames(trace)],
        "missed": sum(1 for event in trace if event["name"] == "missed"),
        "load": load.end(given.gpu),
    }


def table(figures):
    """The figures as a table of Markdown, a column per program and the host's own cost last."""
    names = [name for name, _ in PROGRAMS]
    blank = figures[names[0]]
    application = figures[names[2]]

    def p99(values):
        return percentile(sorted(values), 0.99)

    def latency(values):
        if values is None:
            return "not told"
        return (
            f"{statistics.median(values):.2f} / {p99(values):.2f} / {max(values):.2f} ms "
            f"({len(values)} images)"
        )

    def own(value_of, unit, digits=1):
        try:
            first, second = value_of(application), value_of(blank)
        except TypeError:
            # A figure that one of the two lacks.
            return "—"
        if first is None or second is None:
            return "—"
        return f"{first - second:+.{digits}f} {unit}"

    def load(f):
        gpu = "" if f["load"]["gpu"] is None else f", the GPU {f['load']['gpu']:.0f} % busy"
        mark = "**disturbed**: " if f["load"]["disturbed"] else ""
        return f"{mark}{f['load']['cores']:.2f} cores{gpu}"

    rows = [
        (
            "Start, median (min–max)",
            lambda f: f"{statistics.median(f['start']):.1f} ms "
            f"({min(f['start']):.1f}–{max(f['start']):.1f})",
            own(lambda f: statistics.median(f["start"]), "ms"),
        ),
        (
            "Memory, Pss / Rss / peak",
            lambda f: f"{f['memory']['pss']:.1f} / {f['memory']['rss']:.1f} / "
            f"{f['memory']['peak']:.1f} MB",
            own(lambda f: f["memory"]["pss"], "MB Pss"),
        ),
        (
            "Video memory",
            lambda f: "not counted"
            if f["memory"]["video"] is None
            else f"{f['memory']['video']:.1f} MB",
            own(lambda f: f["memory"]["video"], "MB"),
        ),
        (
            "Binary, built / stripped",
            lambda f: f"{f['binary'][0]:.1f} / {f['binary'][1]:.1f} MB",
            own(lambda f: f["binary"][1], "MB stripped"),
        ),
        (
            f"At rest over {REST:.0f} s: frames, wake-ups a second, share of a core",
            lambda f: f"{f['rest']['frames']}, {f['rest']['wakes']:.1f}, "
            f"{f['rest']['core']:.2f} %",
            own(lambda f: f["rest"]["wakes"], "wake-ups a second"),
        ),
        (
            "Input to present, apart: median / 99th / longest",
            lambda f: latency(f["apart"]),
            own(lambda f: p99(f["apart"]), "ms at the 99th", 2),
        ),
        (
            f"Input to present, in a row ({IN_A_ROW} a second): median / 99th / longest",
            lambda f: latency(f["in a row"]),
            own(lambda f: p99(f["in a row"]), "ms at the 99th", 2),
        ),
        (
            f"Input to present, in motion ({IN_MOTION} a second): median / 99th / longest",
            lambda f: latency(f["in motion"]),
            own(lambda f: p99(f["in motion"]), "ms at the 99th", 2),
        ),
        *(
            (
                f"Input to shown, {label}: median / 99th / longest",
                lambda f, which=which: latency(f[f"{which} shown"]),
                own(lambda f, which=which: p99(f[f"{which} shown"]), "ms at the 99th", 2),
            )
            for which, label in (
                ("apart", "apart"),
                ("in a row", f"in a row ({IN_A_ROW} a second)"),
                ("in motion", f"in motion ({IN_MOTION} a second)"),
            )
        ),
        (
            "Frames that missed their predicted refresh",
            lambda f: str(f["missed"]),
            "",
        ),
        (
            "A frame's work, start to present: median / 99th",
            lambda f: f"{statistics.median(f['frame work']):.2f} / "
            f"{p99(f['frame work']):.2f} ms",
            own(lambda f: statistics.median(f["frame work"]), "ms", 2),
        ),
        ("Other programs' load meanwhile", load, ""),
    ]
    lines = [
        "| Measure | " + " | ".join(names) + " | The host's own cost |",
        "|---|" + "---|" * (len(names) + 1),
    ]
    for label, cell, difference in rows:
        lines.append(
            f"| {label} | " + " | ".join(cell(figures[name]) for name in names) + f" | {difference} |"
        )
    return "\n".join(lines)


def main():
    given = arguments()
    signal.signal(signal.SIGTERM, on_signal)
    # A run under `nohup` keeps ignoring the hangup.
    if signal.getsignal(signal.SIGHUP) is not signal.SIG_IGN:
        signal.signal(signal.SIGHUP, on_signal)
    if not given.gpu and not os.environ.get("SLIME_LAVAPIPE_ICD"):
        fail("SLIME_LAVAPIPE_ICD is not set: run this in the development shell (nix develop)")
    for tool in ("sway", "swaymsg", "strip"):
        if shutil.which(tool) is None:
            fail(f"{tool} is not found: run this in the development shell (nix develop)")
    before = os.getloadavg()[0]
    build()
    work = given.keep or ROOT / "target" / "host-cost" / time.strftime("%Y%m%d-%H%M%S")
    work.mkdir(parents=True, exist_ok=True)
    figures = {}
    sway = None
    try:
        sway = Sway(work, given.gpu)
        os.environ.update(sway.environment())
        devices = script("compositor-input.py").Wlroots()
        for name, binary in PROGRAMS:
            print(f"measuring {name}…", file=sys.stderr)
            figures[name] = measure(name, binary, sway, devices, work, given)
    finally:
        if sway is not None:
            sway.stop()
    commit = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True
    ).stdout.strip()
    changed = subprocess.run(
        ["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True
    ).stdout.strip()
    drawn = "the machine's GPU, through sway's GL renderer" if given.gpu else "lavapipe"
    print(f"Commit {commit}{' with changes' if changed else ''}, drawn with {drawn}; "
          f"{given.starts} starts each; the machine's load {before:.2f} before the build.\n")
    print(table(figures))
    kept = work.relative_to(ROOT) if work.is_relative_to(ROOT) else work
    print(f"\nTraces and logs in {kept}.", file=sys.stderr)


if __name__ == "__main__":
    main()
