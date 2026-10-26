"""Reads and uses the `interface` example's window as a screen reader does, through AT-SPI, the
service that Orca reads on Linux (docs/adr/0146-screen-readers.md).

scripts/screen-reader-check.sh runs it, on a session bus of its own where the AT-SPI service runs,
while the example runs: it finds the example's tree, reads its bar, its buttons and its fields, a
field's text, caret and words and the field of numbers' value, gives the field of text another
text, which the example reads as committed, moves the caret, and presses the button that opens the
dialog. It fails with a message at the first thing that does not hold.
"""

import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def fail(message):
    print(f"screen reader check: {message}", file=sys.stderr)
    # What AT-SPI's registry holds, to tell a registry that never answered from an application
    # that never came.
    try:
        desktop = Atspi.get_desktop(0)
        names = [
            desktop.get_child_at_index(index).get_name()
            for index in range(desktop.get_child_count())
        ]
        print(f"screen reader check: the desktop holds {names}", file=sys.stderr)
    except gi.repository.GLib.Error as error:
        print(f"screen reader check: the desktop cannot be read: {error}", file=sys.stderr)
    sys.exit(1)


def until(what, find, seconds=30.0):
    """Asks `find` again, a tenth of a second apart, until it gives something, as the example's
    frames apply what was asked."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            found = find()
        except gi.repository.GLib.Error:
            # An element that a frame replaced while it was read.
            found = None
        if found:
            return found
        time.sleep(0.1)
    fail(f"{what}: not within {seconds:.0f} seconds")


def application(name):
    """The application of AT-SPI's desktop named `name`."""
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        child = desktop.get_child_at_index(index)
        if child is not None and child.get_name() == name:
            return child
    return None


def walk(node):
    """`node` and every node below it."""
    nodes = [node]
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child is not None:
            nodes.extend(walk(child))
    return nodes


def find(app, role, name=None):
    """The first node below `app` of the role `role`, named `name` when given."""
    for node in walk(app):
        if node.get_role_name() == role and (name is None or node.get_name() == name):
            return node
    return None


def text_of(node):
    return Atspi.Text.get_text(node, 0, Atspi.Text.get_character_count(node))


app = until("the example's tree", lambda: application("interface"), seconds=90.0)
frame = until("the window", lambda: find(app, "frame"))
if frame.get_name() != "SlimeEngine interface example":
    fail(f"the window is named {frame.get_name()!r}")
bar = until("the bar", lambda: find(app, "tool bar", "bar"))
buttons = [node for node in walk(bar) if node.get_role_name() == "button"]
if len(buttons) < 2:
    fail(f"the bar holds {len(buttons)} buttons")
if Atspi.Action.get_n_actions(buttons[0]) < 1:
    fail("a button takes no action")

# The field of text: its text, its caret at its end, and its words with the spaces after them.
entry = until("the field of text", lambda: find(app, "entry", "name"))
if text_of(entry) != "a name":
    fail(f"the field holds {text_of(entry)!r}")
if Atspi.Text.get_caret_offset(entry) != 6:
    fail(f"the caret is at {Atspi.Text.get_caret_offset(entry)}")
word = Atspi.Text.get_string_at_offset(entry, 1, Atspi.TextGranularity.WORD).content
if word != "a ":
    fail(f"the word at 1 is {word!r}")
number = until("the field of numbers", lambda: find(app, "spin button", "scale"))
if Atspi.Value.get_current_value(number) != 1.0:
    fail(f"the field of numbers holds {Atspi.Value.get_current_value(number)}")

# A text given, as a screen reader lets a person type it: the example reads it as committed.
def entry_holding(text):
    """The field of text, when it holds `text`."""
    node = find(app, "entry", "name")
    return node if node is not None and text_of(node) == text else None


if not Atspi.EditableText.set_text_contents(entry, "Orca reads"):
    fail("the field took no text")
until("the text committed", lambda: find(app, "label", "name Orca reads"))
entry = until("the field's new text", lambda: entry_holding("Orca reads"))
if not Atspi.Text.set_caret_offset(entry, 2):
    fail("the field took no caret")
until("the caret moved", lambda: Atspi.Text.get_caret_offset(entry) == 2)

# The first button pressed: it opens the dialog.
opener = until("the bar's first button", lambda: find(app, "button", buttons[0].get_name()))
if not Atspi.Action.do_action(opener, 0):
    fail("the button took no click")
until("the dialog open", lambda: find(app, "dialog"))
print("screen reader check ... ok")
