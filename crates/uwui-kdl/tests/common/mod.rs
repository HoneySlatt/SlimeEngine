//! What the tests compare of two documents: their trees, as lines of text.

use uwui_kdl::{Document, Entry, Node, Value};

/// `document`'s tree, a line for each node, indented by its depth: its annotation and name, its
/// arguments in their order, then the last value of each property's name, by name. A number is
/// compared by its value, as a whole number where it is written as one, else by the bits of its
/// float: `1_000` and `1000` are the same.
pub fn tree(document: &Document<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    // Nodes still to write, last first, with their depths: a walk without recursion.
    let mut stack: Vec<(Node<'_, '_>, usize)> = document.nodes().map(|node| (node, 0)).collect();
    stack.reverse();
    while let Some((node, depth)) = stack.pop() {
        let mut line = "    ".repeat(depth);
        if let Some(annotation) = node.annotation() {
            line.push_str(&format!("({})", annotation.text()));
        }
        line.push_str(&node.name().text());
        for argument in node.arguments() {
            line.push(' ');
            line.push_str(&entry(&argument));
        }
        let mut properties: Vec<(String, String)> = Vec::new();
        for property in node.entries().filter(|entry| entry.name().is_some()) {
            let name = property
                .name()
                .map(|name| name.text().into_owned())
                .unwrap_or_default();
            properties.retain(|(given, _)| *given != name);
            properties.push((name, entry(&property)));
        }
        properties.sort();
        for (name, value) in properties {
            line.push_str(&format!(" {name}={value}"));
        }
        lines.push(line);
        let children: Vec<_> = node.children().collect();
        stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }
    lines
}

/// An entry's annotation and value, as `tree` writes them.
fn entry(entry: &Entry<'_, '_>) -> String {
    let annotation = entry
        .annotation()
        .map(|annotation| format!("({})", annotation.text()))
        .unwrap_or_default();
    let value = match entry.value() {
        Value::String(string) => format!("{:?}", string.text()),
        Value::Number(number) => match number.to_i64() {
            Some(whole) => format!("{whole}"),
            None => format!("float {:#x}", number.to_f64().to_bits()),
        },
        Value::Bool(value) => format!("#{value}"),
        Value::Null => "#null".to_owned(),
    };
    format!("{annotation}{value}")
}
