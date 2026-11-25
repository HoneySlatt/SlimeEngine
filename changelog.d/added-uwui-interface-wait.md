- An agent waits for a change of a program's interface: `interface/wait`, the interface's family
  4, waits for elements to match a role, a name and states held or lacking, or for none to, and
  gives `after`, the elements seen, which the next wait of the same condition takes to see each
  change once. uwui answers it from a host's `agents::Waits` (`agents::wait`, `agents::met`); a
  wait, or a check, while its target holds a change not yet framed reads nothing until the next
  frame. uwui's MCP library gives it as `wait_for_elements`, up to 50 s, and bounds every tool that
  waits through `uwui_mcp::waited`, which `editor_journal` now takes (ADR-0314).
