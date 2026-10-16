//! Scanning directories of source files, in temporary directories: the IDs that new files get,
//! keep when moved and lose when copied, and what the scan reports about metadata files.

use std::fs;
use std::path::{Path, PathBuf};

use slime_cook::metadata;
use slime_cook::scan::metadata_path;
use slime_cook::{Mode, Problem, ScanError, scan};

/// An empty directory of its own for the test `name`, under Cargo's directory for temporary files
/// of integration tests.
fn directory(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-cook-scan")
        .join(name);
    if path.exists() {
        fs::remove_dir_all(&path).expect("the test's old directory can be removed");
    }
    fs::create_dir_all(&path).expect("the test's directory can be created");
    path
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the directory can be created");
    }
    fs::write(path, contents).expect("the file can be written");
}

#[test]
fn a_new_source_file_gets_an_id_in_a_metadata_file_beside_it() {
    let root = directory("new");
    write(&root.join("rock.png"), "an image");
    write(&root.join("models/Ruin.GLB"), "a scene");
    write(&root.join("README.md"), "not a source");
    let result = scan(&root, Mode::GiveIds).expect("the scan succeeds");
    assert!(result.problems.is_empty(), "{:?}", result.problems);
    let paths: Vec<&Path> = result.sources.iter().map(|s| s.path.as_path()).collect();
    assert_eq!(paths, [root.join("models/Ruin.GLB"), root.join("rock.png")]);
    assert_eq!(
        result.created,
        [
            root.join("models/Ruin.GLB.meta"),
            root.join("rock.png.meta")
        ]
    );
    for source in &result.sources {
        let text = fs::read_to_string(metadata_path(&source.path)).expect("written");
        assert_eq!(metadata::read(&text).expect("a metadata file"), source.id);
    }
    assert_ne!(result.sources[0].id, result.sources[1].id);
    assert!(!root.join("README.md.meta").exists());
}

#[test]
fn a_second_scan_writes_nothing_and_finds_the_same_ids() {
    let root = directory("second");
    write(&root.join("a.png"), "a");
    write(&root.join("b/c.jpeg"), "c");
    let first = scan(&root, Mode::GiveIds).expect("the first scan succeeds");
    let second = scan(&root, Mode::GiveIds).expect("the second scan succeeds");
    assert!(second.created.is_empty());
    assert!(second.problems.is_empty());
    assert_eq!(second.sources, first.sources);
}

#[test]
fn a_file_moved_or_renamed_with_its_metadata_keeps_its_id() {
    let root = directory("moved");
    write(&root.join("a/rock.png"), "a rock");
    let before = scan(&root, Mode::GiveIds)
        .expect("the scan succeeds")
        .sources[0]
        .id;
    fs::create_dir_all(root.join("b")).expect("created");
    fs::rename(root.join("a/rock.png"), root.join("b/stone.png")).expect("moved");
    fs::rename(root.join("a/rock.png.meta"), root.join("b/stone.png.meta")).expect("moved");
    let after = scan(&root, Mode::Check).expect("the scan succeeds");
    assert!(after.problems.is_empty(), "{:?}", after.problems);
    assert_eq!(after.sources[0].path, root.join("b/stone.png"));
    assert_eq!(after.sources[0].id, before);
}

#[test]
fn a_file_moved_without_its_metadata_leaves_it_behind_and_gets_a_new_id() {
    let root = directory("moved-alone");
    write(&root.join("rock.png"), "a rock");
    let before = scan(&root, Mode::GiveIds)
        .expect("the scan succeeds")
        .sources[0]
        .id;
    fs::rename(root.join("rock.png"), root.join("stone.png")).expect("renamed");
    let check = scan(&root, Mode::Check).expect("the scan succeeds");
    assert_eq!(
        check.problems,
        [
            Problem::NoSource {
                metadata: root.join("rock.png.meta")
            },
            Problem::NoMetadata {
                source: root.join("stone.png")
            },
        ]
    );
    assert!(check.sources.is_empty());
    let after = scan(&root, Mode::GiveIds).expect("the scan succeeds");
    assert_ne!(after.sources[0].id, before);
}

#[test]
fn a_file_copied_with_its_metadata_is_reported() {
    let root = directory("copied");
    write(&root.join("rock.png"), "a rock");
    let id = scan(&root, Mode::GiveIds)
        .expect("the scan succeeds")
        .sources[0]
        .id;
    fs::copy(root.join("rock.png"), root.join("rock 2.png")).expect("copied");
    fs::copy(root.join("rock.png.meta"), root.join("rock 2.png.meta")).expect("copied");
    let result = scan(&root, Mode::GiveIds).expect("the scan succeeds");
    assert_eq!(
        result.problems,
        [Problem::SameId {
            id,
            first: root.join("rock 2.png.meta"),
            second: root.join("rock.png.meta"),
        }]
    );
    // Once the copy's metadata file is deleted, the copy gets an ID of its own.
    fs::remove_file(root.join("rock 2.png.meta")).expect("deleted");
    let result = scan(&root, Mode::GiveIds).expect("the scan succeeds");
    assert!(result.problems.is_empty());
    assert_eq!(result.created, [root.join("rock 2.png.meta")]);
    assert_ne!(result.sources[0].id, result.sources[1].id);
}

#[test]
fn a_check_writes_nothing() {
    let root = directory("check");
    write(&root.join("rock.png"), "a rock");
    let result = scan(&root, Mode::Check).expect("the scan succeeds");
    assert_eq!(
        result.problems,
        [Problem::NoMetadata {
            source: root.join("rock.png")
        }]
    );
    assert!(result.created.is_empty());
    assert!(!root.join("rock.png.meta").exists());
}

#[test]
fn a_metadata_file_out_of_its_format_fails_the_scan_with_its_path_and_line() {
    let root = directory("malformed");
    write(&root.join("rock.png"), "a rock");
    write(&root.join("rock.png.meta"), "# fine\nname = \"rock\"\n");
    let error = scan(&root, Mode::GiveIds).expect_err("the scan fails");
    assert!(matches!(error, ScanError::Metadata { .. }), "{error:?}");
    let message = error.to_string();
    assert!(
        message.contains("rock.png.meta") && message.contains("line 2"),
        "{message}"
    );
}

#[test]
fn a_directory_that_does_not_exist_fails_the_scan() {
    let root = directory("missing").join("nowhere");
    let error = scan(&root, Mode::Check).expect_err("the scan fails");
    assert!(matches!(error, ScanError::Io { .. }), "{error:?}");
}
