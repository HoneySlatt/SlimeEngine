//! The counters that CI checks (`docs/adr/0186-counters-and-ratios-in-ci.md`): figures that a
//! scenario gives the same on every machine and at every run, draws, items, elements, bytes and
//! allocations, recorded in [`FILE`], which `--counters` compares a run with, and
//! `--write-counters` writes.

use std::collections::BTreeMap;

/// The file that records the counters, from the workspace's directory.
pub const FILE: &str = "benches/counters.txt";

/// What the file says above its counters.
const HEADER: &str = "\
# The counters of slime-benches, which `scripts/check.sh` checks: each line a benchmark, what it
# counts and its value, parted by tabs (docs/adr/0186-counters-and-ratios-in-ci.md). A change
# that changes a counter on purpose writes this file again in the same commit:
#   cargo run -p slime-benches -- --write-counters [<name>...]
";

/// One counter that a run gave: its benchmark, what it counts, and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counter {
    pub benchmark: String,
    pub what: String,
    pub value: u64,
}

/// The counters of `text`, the file's, by benchmark and by what they count.
///
/// # Errors
///
/// A line that is neither a comment nor a counter: which, as a sentence.
fn read(text: &str) -> Result<BTreeMap<(String, String), u64>, String> {
    let mut counters = BTreeMap::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        let [benchmark, what, value] = parts[..] else {
            return Err(format!(
                "{FILE}, line {}: not a benchmark, what it counts and a value, parted by tabs",
                number + 1
            ));
        };
        let value = value
            .parse()
            .map_err(|_| format!("{FILE}, line {}: {value:?} is not a count", number + 1))?;
        if counters
            .insert((benchmark.to_owned(), what.to_owned()), value)
            .is_some()
        {
            return Err(format!(
                "{FILE}, line {}: `{benchmark}` counts {what:?} twice",
                number + 1
            ));
        }
    }
    Ok(counters)
}

/// How `measured`, the counters of the benchmarks named `run`, differ from those that `recorded`,
/// the file's text, holds for them: one sentence for each counter changed, missing from the run or
/// from the file, and for each benchmark of the file that `known` does not name. Empty when they
/// agree.
///
/// # Errors
///
/// When `recorded` cannot be read: why, as a sentence.
pub fn differences(
    recorded: &str,
    measured: &[Counter],
    run: &[&str],
    known: &[&str],
) -> Result<Vec<String>, String> {
    let recorded = read(recorded)?;
    let mut differences = Vec::new();
    for ((benchmark, what), value) in &recorded {
        if !known.contains(&benchmark.as_str()) {
            differences.push(format!(
                "`{benchmark}` is in {FILE}, but no benchmark has that name"
            ));
        } else if run.contains(&benchmark.as_str())
            && !measured
                .iter()
                .any(|counter| &counter.benchmark == benchmark && &counter.what == what)
        {
            differences.push(format!(
                "`{benchmark}` no longer counts {what:?}, which {FILE} holds at {value}"
            ));
        }
    }
    for counter in measured {
        let key = (counter.benchmark.clone(), counter.what.clone());
        match recorded.get(&key) {
            Some(&value) if value == counter.value => {}
            Some(&value) => differences.push(format!(
                "`{}` counts {} for {:?}, where {FILE} holds {value}",
                counter.benchmark, counter.value, counter.what
            )),
            None => differences.push(format!(
                "`{}` counts {} for {:?}, which {FILE} does not hold",
                counter.benchmark, counter.value, counter.what
            )),
        }
    }
    Ok(differences)
}

/// The file's text with the counters of the benchmarks named `run` replaced by `measured`, those
/// of the others kept, and those of benchmarks that `known` does not name dropped.
///
/// # Errors
///
/// When `recorded` cannot be read: why, as a sentence.
pub fn written(
    recorded: &str,
    measured: &[Counter],
    run: &[&str],
    known: &[&str],
) -> Result<String, String> {
    let mut counters = read(recorded)?;
    counters.retain(|(benchmark, _), _| {
        known.contains(&benchmark.as_str()) && !run.contains(&benchmark.as_str())
    });
    for counter in measured {
        counters.insert(
            (counter.benchmark.clone(), counter.what.clone()),
            counter.value,
        );
    }
    let mut text = String::from(HEADER);
    for ((benchmark, what), value) in counters {
        text.push_str(&format!("{benchmark}\t{what}\t{value}\n"));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(benchmark: &str, what: &str, value: u64) -> Counter {
        Counter {
            benchmark: benchmark.to_owned(),
            what: what.to_owned(),
            value,
        }
    }

    const KNOWN: &[&str] = &["ui.tree.editor", "editor.window.zone"];
    const RECORDED: &str = "# a comment\nui.tree.editor\telements\t12487\n\
                            editor.window.zone\tallocations, standing still\t0\n";

    #[test]
    fn a_counter_doubled_or_halved_differs_and_one_unchanged_does_not() {
        let unchanged = [counter("ui.tree.editor", "elements", 12_487)];
        let run = ["ui.tree.editor"];
        assert_eq!(
            differences(RECORDED, &unchanged, &run, KNOWN),
            Ok(Vec::new())
        );
        for value in [24_974, 6_243] {
            let changed = [counter("ui.tree.editor", "elements", value)];
            let found = differences(RECORDED, &changed, &run, KNOWN).expect("read");
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].contains(&value.to_string()), "{found:?}");
        }
    }

    #[test]
    fn allocations_where_none_were_differ() {
        let found = differences(
            RECORDED,
            &[counter(
                "editor.window.zone",
                "allocations, standing still",
                1,
            )],
            &["editor.window.zone"],
            KNOWN,
        )
        .expect("read");
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn a_counter_new_lost_or_of_an_unknown_benchmark_differs() {
        let new = [
            counter("ui.tree.editor", "elements", 12_487),
            counter("ui.tree.editor", "visited", 3),
        ];
        assert_eq!(
            differences(RECORDED, &new, &["ui.tree.editor"], KNOWN)
                .expect("read")
                .len(),
            1
        );
        assert_eq!(
            differences(RECORDED, &[], &["ui.tree.editor"], KNOWN)
                .expect("read")
                .len(),
            1
        );
        assert_eq!(
            differences(RECORDED, &[], &[], &["ui.tree.editor"])
                .expect("read")
                .len(),
            1,
            "a benchmark gone"
        );
    }

    #[test]
    fn the_benchmarks_not_run_are_not_compared() {
        assert_eq!(
            differences(RECORDED, &[], &[], KNOWN),
            Ok(Vec::new()),
            "neither ran"
        );
    }

    #[test]
    fn a_damaged_file_is_told() {
        assert!(differences("ui.tree.editor elements 3\n", &[], &[], KNOWN).is_err());
        assert!(differences("ui.tree.editor\telements\tmany\n", &[], &[], KNOWN).is_err());
        assert!(
            differences(
                "ui.tree.editor\telements\t1\nui.tree.editor\telements\t2\n",
                &[],
                &[],
                KNOWN
            )
            .is_err()
        );
    }

    #[test]
    fn writing_replaces_the_benchmarks_run_and_keeps_the_others() {
        let text = written(
            RECORDED,
            &[counter("ui.tree.editor", "visited", 3)],
            &["ui.tree.editor"],
            KNOWN,
        )
        .expect("read");
        assert!(text.starts_with(HEADER));
        let counters = read(&text).expect("read again");
        assert_eq!(counters.len(), 2);
        assert_eq!(
            counters.get(&("ui.tree.editor".to_owned(), "visited".to_owned())),
            Some(&3)
        );
        assert_eq!(
            counters.get(&(
                "editor.window.zone".to_owned(),
                "allocations, standing still".to_owned()
            )),
            Some(&0)
        );
        let gone = written(RECORDED, &[], &[], &["ui.tree.editor"]).expect("read");
        assert_eq!(read(&gone).expect("read again").len(), 1);
    }
}
