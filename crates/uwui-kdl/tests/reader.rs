//! What a document read holds, how its strings and numbers read, where its errors lie, and the
//! limits that bound it.

use std::borrow::Cow;

use uwui_kdl::{Limits, Value, read, read_within};

/// The text of `value`, which must be a string.
fn string<'t>(value: &Value<'_, 't>) -> Cow<'t, str> {
    match value {
        Value::String(string) => string.text(),
        other => panic!("{other:?} is no string"),
    }
}

#[test]
fn a_node_holds_its_name_entries_and_children_in_their_order() {
    let text =
        "(kind)button back \"two\" 3 text=Back on=#true none=#null {\n    icon; label\n}\nnext";
    let document = read(text).expect("read");
    let nodes: Vec<_> = document.nodes().collect();
    assert_eq!(nodes.len(), 2);
    let button = nodes[0];
    assert_eq!(button.name().text(), "button");
    assert_eq!(button.annotation().expect("annotated").text(), "kind");
    let arguments: Vec<_> = button
        .arguments()
        .map(|entry| entry.value().to_string())
        .collect();
    assert_eq!(arguments, ["back", "two", "3"]);
    assert_eq!(string(&button.property("text").expect("text")), "Back");
    assert!(matches!(button.property("on"), Some(Value::Bool(true))));
    assert!(matches!(button.property("none"), Some(Value::Null)));
    assert!(button.property("off").is_none());
    let children: Vec<_> = button.children().map(|child| child.name().text()).collect();
    assert_eq!(children, ["icon", "label"]);
    assert_eq!(nodes[1].children().count(), 0);
    assert_eq!(
        &text[button.span()],
        &text[..text.find("}").expect("a block") + 1]
    );
}

#[test]
fn a_property_given_twice_is_kept_twice_and_the_last_is_its_value() {
    let document = read("node a=1 b=2 a=3").expect("read");
    let node = document.nodes().next().expect("a node");
    let names: Vec<_> = node
        .entries()
        .map(|entry| entry.name().expect("a property").text())
        .collect();
    assert_eq!(names, ["a", "b", "a"]);
    assert_eq!(node.property("a").expect("a").to_string(), "3");
    assert_eq!(document.normal_form(), "node a=3 b=2\n");
}

#[test]
fn a_string_without_escapes_is_borrowed_and_one_with_them_decoded() {
    let text = concat!(
        "node bare \"plain\" #\"raw\\n\"# \"tab\\there\" \"\\u{E06C}\" ",
        "\"\"\"\n    first\n      second\n    \"\"\""
    );
    let document = read(text).expect("read");
    let node = document.nodes().next().expect("a node");
    let values: Vec<_> = node
        .arguments()
        .map(|entry| string(&entry.value()))
        .collect();
    assert!(matches!(values[0], Cow::Borrowed("bare")));
    assert!(matches!(values[1], Cow::Borrowed("plain")));
    assert!(matches!(values[2], Cow::Borrowed("raw\\n")));
    assert_eq!(values[3], "tab\there");
    assert_eq!(values[4], "\u{E06C}");
    assert_eq!(values[5], "first\n  second");
    let quoted = node.arguments().nth(1).expect("quoted");
    let Value::String(quoted) = quoted.value() else {
        panic!("a string");
    };
    assert_eq!(&text[quoted.span()], "\"plain\"");
}

#[test]
fn numbers_read_in_every_base_and_as_floats() {
    let document = read(
        "n 0xff -0b101 0o17 1_000 1.5e3 -0.5 #inf #-inf #nan 0xffffffffffffffffff 9223372036854775807",
    )
    .expect("read");
    let numbers: Vec<_> = document
        .nodes()
        .next()
        .expect("a node")
        .arguments()
        .map(|entry| match entry.value() {
            Value::Number(number) => number,
            other => panic!("{other:?} is no number"),
        })
        .collect();
    let whole: Vec<_> = numbers.iter().map(uwui_kdl::Number::to_i64).collect();
    assert_eq!(
        whole,
        [
            Some(255),
            Some(-5),
            Some(15),
            Some(1000),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(i64::MAX)
        ]
    );
    assert_eq!(numbers[4].to_f64(), 1500.0);
    assert_eq!(numbers[5].to_f64(), -0.5);
    assert_eq!(numbers[6].to_f64(), f64::INFINITY);
    assert_eq!(numbers[7].to_f64(), f64::NEG_INFINITY);
    assert!(numbers[8].to_f64().is_nan());
    assert_eq!(numbers[0].text(), "0xff");
}

#[test]
fn an_error_names_its_line_and_its_column_in_characters() {
    // A block closed that was never opened, after a character of two bytes.
    let error = read("first\né1 }").expect_err("refused");
    assert_eq!((error.line, error.column, error.offset), (2, 4, 10));
    assert!(
        error.to_string().starts_with("line 2, column 4: "),
        "{error}"
    );
    // A carriage return and a line feed end one line.
    let error = read("first\r\nsecond }").expect_err("refused");
    assert_eq!((error.line, error.column), (2, 8));
    // A text that ends inside a block is refused at its end.
    let error = read("node {").expect_err("refused");
    assert_eq!((error.line, error.column, error.offset), (1, 7, 6));
    // A text that ends after a string's `\\` leaves the string open.
    let error = read("n \"a\\").expect_err("refused");
    assert!(error.message.contains("not closed"), "{error}");
}

#[test]
fn a_document_deeper_than_its_limit_is_refused_at_the_node_too_deep() {
    let nested = |depth: usize| {
        let mut text = String::new();
        for level in 0..=depth {
            text.push_str(&format!("n{level} {{\n"));
        }
        text.push_str(&"}\n".repeat(depth + 1));
        text
    };
    assert!(read(&nested(64)).is_ok(), "64 levels below the top");
    let error = read(&nested(65)).expect_err("65 levels");
    assert_eq!(error.line, 66, "{error}");
}

#[test]
fn a_document_larger_or_holding_more_items_than_its_limits_is_refused() {
    let limits = Limits {
        size: 10,
        ..Limits::DEFAULT
    };
    assert!(read_within("node 1 2 3", &limits).is_ok());
    assert!(read_within("node 1 2 34", &limits).is_err());
    // One node and three entries are four items.
    let four = Limits {
        items: 4,
        ..Limits::DEFAULT
    };
    let three = Limits {
        items: 3,
        ..Limits::DEFAULT
    };
    assert!(read_within("node 1 2 3", &four).is_ok());
    // The fourth item, the argument `3`, is the one refused.
    let error = read_within("node 1 2 3", &three).expect_err("four items");
    assert_eq!((error.offset, error.column), (9, 10), "{error}");
    assert!(read_within("a; b; c; d", &three).is_err());
}

#[test]
fn reading_never_recurses_however_deep_the_document() {
    // A million levels, read on a thread whose stack holds no recursion of that depth.
    let depth = 1_000_000;
    let text = format!("{}{}", "a{".repeat(depth), "}".repeat(depth));
    let unbounded = Limits {
        depth: usize::MAX,
        ..Limits::DEFAULT
    };
    let read = std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || read_within(&text, &unbounded).map(|document| document.nodes().count()))
        .expect("a thread")
        .join()
        .expect("no overflow of the stack");
    assert_eq!(read, Ok(1));
}

#[test]
fn an_error_placed_by_a_reader_above_lies_where_the_reader_would_place_it() {
    let text = "first\r\nsecond é=1\nthird";
    let at = text.find("é").expect("é");
    let error = uwui_kdl::Error::at(text, at, "a fault");
    assert_eq!((error.line, error.column, error.offset), (2, 8, at));
    assert_eq!(error.to_string(), "line 2, column 8: a fault");
    // Inside a character: at its first byte; past the text: at its end.
    assert_eq!(uwui_kdl::Error::at(text, at + 1, "x").offset, at);
    let end = uwui_kdl::Error::at(text, text.len() + 9, "x");
    assert_eq!((end.line, end.column, end.offset), (3, 6, text.len()));
}
