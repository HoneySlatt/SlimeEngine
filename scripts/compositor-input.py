#!/usr/bin/env python3
"""Gives a person's input to a compositor of the series, through the tool of its family
(docs/adr/0195-a-series-of-headless-compositors.md,
docs/adr/0238-the-series-input-its-table-and-its-jobs.md):
wlroots' virtual pointers and keyboards for sway, libei for KWin and Mutter, XTest for X11
(`xdotool`). Headless Weston takes none.

    scripts/compositor-input.py serve sway|kwin|mutter|x11 <socket>
    scripts/compositor-input.py move-to <x> <y>   # the pointer, in the output's logical pixels
    scripts/compositor-input.py click             # the left button, pressed and let go
    scripts/compositor-input.py key <keys>        # keys pressed together, `ctrl+a`

`serve` holds a virtual keyboard and a virtual pointer for as long as it runs, and takes the other
commands on a Unix socket, whose path `SLIME_COMPOSITOR_INPUT` gives them. A device made for each
command and gone after it would come and go before the window bound it: the compositor tells its
clients that the seat has a keyboard, then none, and the keys between reach no window.
scripts/compositors.sh serves; slime-mcp's `compositor` test sends.

sway takes wlroots' protocols, spoken here on the Wayland socket itself; KWin and Mutter give a
socket of libei on the session bus, as the desktop's portal would, without its dialog: KWin through
`org.kde.KWin.EIS.RemoteDesktop.connectToEIS`, Mutter through a session of
`org.gnome.Mutter.RemoteDesktop`, which ends with this program. libei's C library, `libei.so.1`,
is called through ctypes: the signatures below are those of libei 1.6's `libei.h`.
"""

import array
import ctypes
import os
import select
import socket
import struct
import subprocess
import sys
import time

# The left button, and the keys that the series types, as Linux's input codes name them
# (linux/input-event-codes.h), which libei and wlroots' virtual keyboard take.
BUTTON_LEFT = 0x110
KEYS = {
    "ctrl": 29,
    "shift": 42,
    "alt": 56,
    "return": 28,
    "escape": 1,
    "tab": 15,
    **{letter: code for letter, code in zip("qwertyuiop", range(16, 26))},
    **{letter: code for letter, code in zip("asdfghjkl", range(30, 39))},
    **{letter: code for letter, code in zip("zxcvbnm", range(44, 51))},
}
# The modifiers' bits in the keymap below, which wlroots' virtual keyboard must be told.
MODIFIERS = {"shift": 1, "ctrl": 4, "alt": 8}

# The logical size of the series' output, which wlroots' absolute motion is given in.
OUTPUT = (1920, 1080)

# How long a compositor may take to answer, to give a seat or its devices.
SETUP_SECONDS = 10


def keys_of(text):
    """The keys of `ctrl+a`, each known."""
    keys = text.lower().split("+")
    if unknown := [key for key in keys if key not in KEYS]:
        raise ValueError(f"no key {unknown}: {sorted(KEYS)}")
    return keys


class Wlroots:
    """A virtual keyboard and a virtual pointer of wlroots, on the compositor's Wayland socket,
    spoken without a library: each message is the object's id, its size and opcode, then its
    arguments, each 32 bits (wayland.freedesktop.org/docs/html/ch04.html, "Wire Format")."""

    def __init__(self):
        path = os.path.join(os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"])
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(path)
        self.next_id = 2
        self.received = b""
        registry = self.new_id()
        self.send(1, 1, struct.pack("<I", registry))  # wl_display.get_registry
        globals_ = {}

        def on_global(sender, opcode, body):
            if sender == registry and opcode == 0:
                globals_.update([self.global_of(body)])

        self.roundtrip(on_global)
        wanted = ["wl_seat", "zwp_virtual_keyboard_manager_v1", "zwlr_virtual_pointer_manager_v1"]
        if missing := [name for name in wanted if name not in globals_]:
            sys.exit(f"compositor-input: the compositor gives no {missing}")
        seat, keyboards, pointers = (self.bind(registry, globals_[name], name) for name in wanted)
        self.keyboard, self.pointer = self.new_id(), self.new_id()
        # zwp_virtual_keyboard_manager_v1.create_virtual_keyboard, then its keymap; then
        # zwlr_virtual_pointer_manager_v1.create_virtual_pointer.
        self.send(keyboards, 0, struct.pack("<II", seat, self.keyboard))
        keymap = KEYMAP.encode() + b"\0"
        fd = os.memfd_create("slime-keymap")
        os.write(fd, keymap)
        self.send(self.keyboard, 0, struct.pack("<II", 1, len(keymap)), [fd])
        os.close(fd)
        self.send(pointers, 0, struct.pack("<II", seat, self.pointer))
        self.roundtrip(lambda *_: None)
        self.held = 0

    def new_id(self):
        self.next_id += 1
        return self.next_id - 1

    @staticmethod
    def string(text):
        data = text.encode() + b"\0"
        return struct.pack("<I", len(data)) + data + b"\0" * (-len(data) % 4)

    @staticmethod
    def global_of(body):
        """The name and interface of a `wl_registry.global` event."""
        name, length = struct.unpack_from("<II", body)
        return body[8 : 8 + length - 1].decode(), name

    def bind(self, registry, name, interface):
        """Binds the global `name` at version 1, and returns its new object."""
        bound = self.new_id()
        self.send(
            registry,
            0,
            struct.pack("<I", name) + self.string(interface) + struct.pack("<II", 1, bound),
        )
        return bound

    def send(self, sender, opcode, arguments, fds=()):
        message = struct.pack("<II", sender, (8 + len(arguments)) << 16 | opcode) + arguments
        ancillary = [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", fds))] if fds else []
        self.socket.sendmsg([message], ancillary)

    def roundtrip(self, on_event):
        """Sends `wl_display.sync`, and gives each event to `on_event` until its callback is done;
        a protocol error ends the program."""
        callback = self.new_id()
        self.send(1, 0, struct.pack("<I", callback))
        deadline = time.monotonic() + SETUP_SECONDS
        while True:
            while len(self.received) >= 8:
                sender, header = struct.unpack_from("<II", self.received)
                size, opcode = header >> 16, header & 0xFFFF
                if len(self.received) < size:
                    break
                body, self.received = self.received[8:size], self.received[size:]
                if sender == 1 and opcode == 0:
                    sys.exit(f"compositor-input: the compositor refused a request: {body!r}")
                if sender == callback and opcode == 0:
                    return
                on_event(sender, opcode, body)
            if not select.select([self.socket], [], [], deadline - time.monotonic())[0]:
                sys.exit("compositor-input: the compositor did not answer")
            data = self.socket.recv(65536)
            if not data:
                sys.exit("compositor-input: the compositor closed the connection")
            self.received += data

    @staticmethod
    def now():
        return int(time.monotonic() * 1000) & 0xFFFFFFFF

    def move_to(self, x, y):
        # zwlr_virtual_pointer_v1.motion_absolute, then frame.
        self.send(self.pointer, 1, struct.pack("<IIIII", self.now(), round(x), round(y), *OUTPUT))
        self.send(self.pointer, 4, b"")
        self.roundtrip(lambda *_: None)

    def click(self):
        for state in (1, 0):
            self.send(self.pointer, 2, struct.pack("<III", self.now(), BUTTON_LEFT, state))
            self.send(self.pointer, 4, b"")
        self.roundtrip(lambda *_: None)

    def key(self, keys):
        # zwp_virtual_keyboard_v1.key, and its modifiers, which the client holds the state of.
        for key in keys:
            self.held |= MODIFIERS.get(key, 0)
            self.send(self.keyboard, 2, struct.pack("<IIII", self.held, 0, 0, 0))
            self.send(self.keyboard, 1, struct.pack("<III", self.now(), KEYS[key], 1))
        for key in reversed(keys):
            self.send(self.keyboard, 1, struct.pack("<III", self.now(), KEYS[key], 0))
            self.held &= ~MODIFIERS.get(key, 0)
            self.send(self.keyboard, 2, struct.pack("<IIII", self.held, 0, 0, 0))
        self.roundtrip(lambda *_: None)

    def dispatch(self):
        """Reads what the compositor sent meanwhile, so that its buffer never fills."""
        self.roundtrip(lambda *_: None)

    def fileno(self):
        return self.socket.fileno()


# A keymap of the US layout, which the compositor compiles from its own XKB files.
KEYMAP = """xkb_keymap {
    xkb_keycodes { include "evdev+aliases(qwerty)" };
    xkb_types { include "complete" };
    xkb_compat { include "complete" };
    xkb_symbols { include "pc+us+inet(evdev)" };
};"""


class Libei:
    """A sender of libei, on the socket that KWin or Mutter gives on the session bus."""

    # Capabilities of a device, `enum ei_device_capability`, and events, `enum ei_event_type`.
    CAP_POINTER_ABSOLUTE = 1 << 1
    CAP_KEYBOARD = 1 << 2
    CAP_BUTTON = 1 << 5
    EVENT_DISCONNECT = 2
    EVENT_SEAT_ADDED = 3
    EVENT_DEVICE_REMOVED = 6
    EVENT_DEVICE_PAUSED = 7
    EVENT_DEVICE_RESUMED = 8
    DEVICE_EVENTS = (EVENT_DEVICE_REMOVED, EVENT_DEVICE_PAUSED, EVENT_DEVICE_RESUMED)

    def __init__(self, compositor):
        from gi.repository import Gio, GLib

        self.lib = library()
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        call = self.call
        if compositor == "kwin":
            # The portal's flags: keyboard 1, pointer 2.
            (handle, _cookie), fds = call(
                "org.kde.KWin",
                "/org/kde/KWin/EIS/RemoteDesktop",
                "org.kde.KWin.EIS.RemoteDesktop",
                "connectToEIS",
                GLib.Variant("(i)", (3,)),
            )
        else:
            service = "org.gnome.Mutter.RemoteDesktop"
            (session,), _ = call(
                service, "/org/gnome/Mutter/RemoteDesktop", service, "CreateSession", None
            )
            call(service, session, f"{service}.Session", "Start", None)
            # Its device types: keyboard 1, pointer 2.
            options = {"device-types": GLib.Variant("u", 3)}
            (handle,), fds = call(
                service,
                session,
                f"{service}.Session",
                "ConnectToEIS",
                GLib.Variant("(a{sv})", (options,)),
            )
        lib = self.lib
        self.ei = lib.ei_new_sender(None)
        lib.ei_configure_name(self.ei, b"slime-compositor-input")
        if (error := lib.ei_setup_backend_fd(self.ei, fds.get(handle))) != 0:
            sys.exit(f"compositor-input: libei refused the compositor's socket: error {-error}")
        self.devices = {}
        self.wanted = [self.CAP_POINTER_ABSOLUTE, self.CAP_BUTTON, self.CAP_KEYBOARD]
        deadline = time.monotonic() + SETUP_SECONDS
        while any(cap not in self.devices for cap in self.wanted):
            if not select.select([self], [], [], max(0, deadline - time.monotonic()))[0]:
                missing = sorted(set(self.wanted) - set(self.devices))
                sys.exit(f"compositor-input: the compositor gave no device for {missing}")
            self.dispatch()

    def call(self, destination, path, interface, method, parameters):
        """Calls a method on the session bus, and returns its result and the file descriptors that
        came with it."""
        from gi.repository import Gio

        result, fds = self.bus.call_with_unix_fd_list_sync(
            destination,
            path,
            interface,
            method,
            parameters,
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
            None,
        )
        return result.unpack(), fds

    def fileno(self):
        return self.lib.ei_get_fd(self.ei)

    def dispatch(self):
        """Reads libei's events: binds the seat's capabilities, and keeps for each capability a
        device that has it, resumed and emulating."""
        lib = self.lib
        lib.ei_dispatch(self.ei)
        while event := lib.ei_get_event(self.ei):
            kind = lib.ei_event_get_type(event)
            if kind == self.EVENT_DISCONNECT:
                sys.exit("compositor-input: the compositor ended libei's connection")
            if kind == self.EVENT_SEAT_ADDED:
                caps = [ctypes.c_int(cap) for cap in self.wanted]
                seat = ctypes.c_void_p(lib.ei_event_get_seat(event))
                lib.ei_seat_bind_capabilities(seat, *caps, ctypes.c_void_p(None))
            device = lib.ei_event_get_device(event) if kind in self.DEVICE_EVENTS else None
            if kind == self.EVENT_DEVICE_RESUMED:
                for cap in self.wanted:
                    if cap not in self.devices and lib.ei_device_has_capability(device, cap):
                        self.devices[cap] = lib.ei_device_ref(device)
                        lib.ei_device_start_emulating(device, 1)
            if kind in (self.EVENT_DEVICE_REMOVED, self.EVENT_DEVICE_PAUSED):
                for cap, kept in list(self.devices.items()):
                    if kept == device:
                        lib.ei_device_unref(kept)
                        del self.devices[cap]
            lib.ei_event_unref(event)

    def device(self, cap):
        if cap not in self.devices:
            raise RuntimeError("the compositor took the device away")
        return self.devices[cap]

    def frame(self, device):
        self.lib.ei_device_frame(device, self.lib.ei_now(self.ei))
        self.lib.ei_dispatch(self.ei)

    def move_to(self, x, y):
        device = self.device(self.CAP_POINTER_ABSOLUTE)
        self.lib.ei_device_pointer_motion_absolute(device, x, y)
        self.frame(device)

    def click(self):
        device = self.device(self.CAP_BUTTON)
        for pressed in (True, False):
            self.lib.ei_device_button_button(device, BUTTON_LEFT, pressed)
            self.frame(device)

    def key(self, keys):
        device = self.device(self.CAP_KEYBOARD)
        for pressed, order in ((True, keys), (False, list(reversed(keys)))):
            for key in order:
                self.lib.ei_device_keyboard_key(device, KEYS[key], pressed)
                self.frame(device)


def library():
    """libei's C library, its functions given their signatures."""
    pointer = ctypes.c_void_p
    lib = ctypes.CDLL("libei.so.1")
    for name, result, arguments in [
        ("ei_new_sender", pointer, [pointer]),
        ("ei_configure_name", None, [pointer, ctypes.c_char_p]),
        ("ei_setup_backend_fd", ctypes.c_int, [pointer, ctypes.c_int]),
        ("ei_get_fd", ctypes.c_int, [pointer]),
        ("ei_dispatch", None, [pointer]),
        ("ei_get_event", pointer, [pointer]),
        ("ei_now", ctypes.c_uint64, [pointer]),
        ("ei_event_get_type", ctypes.c_int, [pointer]),
        ("ei_event_get_seat", pointer, [pointer]),
        ("ei_event_get_device", pointer, [pointer]),
        ("ei_event_unref", pointer, [pointer]),
        ("ei_device_ref", pointer, [pointer]),
        ("ei_device_unref", pointer, [pointer]),
        ("ei_device_has_capability", ctypes.c_bool, [pointer, ctypes.c_int]),
        ("ei_device_start_emulating", None, [pointer, ctypes.c_uint32]),
        ("ei_device_frame", None, [pointer, ctypes.c_uint64]),
        ("ei_device_pointer_motion_absolute", None, [pointer, ctypes.c_double, ctypes.c_double]),
        ("ei_device_button_button", None, [pointer, ctypes.c_uint32, ctypes.c_bool]),
        ("ei_device_keyboard_key", None, [pointer, ctypes.c_uint32, ctypes.c_bool]),
    ]:
        function = getattr(lib, name)
        function.restype = result
        function.argtypes = arguments
    # Variadic, its arguments typed at each call: the seat, its capabilities as C ints, then a null
    # pointer. An untyped Python int would go as a C int, a pointer cut to 32 bits.
    lib.ei_seat_bind_capabilities.restype = None
    return lib


class Xtest:
    """X11's input through XTest, which `xdotool` gives; its devices last as long as the server.
    Xvfb runs no window manager, and its keys go to no window until one is given the focus: the
    pointer's motion gives it to the window under it, as a manager whose focus follows the
    pointer would."""

    @staticmethod
    def run(*arguments):
        subprocess.run(["xdotool", *arguments], check=True)

    def move_to(self, x, y):
        self.run("mousemove", str(round(x)), str(round(y)))
        under = subprocess.run(
            ["xdotool", "getmouselocation", "--shell"], check=True, capture_output=True, text=True
        ).stdout
        window = dict(line.split("=", 1) for line in under.split())["WINDOW"]
        self.run("windowfocus", window)

    def click(self):
        self.run("click", "1")

    def key(self, keys):
        self.run("key", "+".join(keys))

    def dispatch(self):
        pass


def serve(compositor, path):
    """Holds the compositor's devices, and runs each command that a client sends, a line each,
    answering `done` or the error."""
    devices = {
        "sway": Wlroots,
        "kwin": lambda: Libei("kwin"),
        "mutter": lambda: Libei("mutter"),
        "x11": Xtest,
    }[compositor]()
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(path)
    listener.listen()
    # Ready: the script that started this waits for the socket.
    watched = [listener] + ([devices] if hasattr(devices, "fileno") else [])
    while True:
        ready = select.select(watched, [], [])[0]
        if devices in ready:
            devices.dispatch()
        if listener not in ready:
            continue
        client, _ = listener.accept()
        with client, client.makefile("rw") as stream:
            words = stream.readline().split()
            try:
                if words[:1] == ["move-to"] and len(words) == 3:
                    devices.move_to(float(words[1]), float(words[2]))
                elif words == ["click"]:
                    devices.click()
                elif words[:1] == ["key"] and len(words) == 2:
                    devices.key(keys_of(words[1]))
                else:
                    raise ValueError(f"no command {' '.join(words)!r}")
                answer = "done"
            except (ValueError, RuntimeError, subprocess.CalledProcessError) as error:
                answer = f"error: {error}"
            stream.write(answer + "\n")


def send(words):
    """Sends a command to the program that serves, and fails if it did not run it."""
    path = os.environ.get("SLIME_COMPOSITOR_INPUT")
    if not path:
        sys.exit(
            "compositor-input: SLIME_COMPOSITOR_INPUT names no socket: the compositor takes no input"
        )
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.connect(path)
        with client.makefile("rw") as stream:
            stream.write(" ".join(words) + "\n")
            stream.flush()
            answer = stream.readline().strip()
    if answer != "done":
        sys.exit(f"compositor-input: {answer or 'no answer'}")


def main():
    arguments = sys.argv[1:]
    if (
        arguments[:1] == ["serve"]
        and len(arguments) == 3
        and arguments[1] in ("sway", "kwin", "mutter", "x11")
    ):
        serve(arguments[1], arguments[2])
    elif (
        (arguments[:1] == ["move-to"] and len(arguments) == 3)
        or arguments == ["click"]
        or (arguments[:1] == ["key"] and len(arguments) == 2)
    ):
        send(arguments)
    else:
        sys.exit(__doc__.split("\n\n")[1])


if __name__ == "__main__":
    main()
