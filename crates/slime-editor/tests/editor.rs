//! The editor's core on a level of its own: steps made, undone and redone, the level written back
//! with its comments and opened again, a journal played again (`docs/adr/0134-the-editors-core.md`).

use std::fs;
use std::path::{Path, PathBuf};

use slime_assets::AssetId;
use slime_editor::{EditError, Editor};
use slime_reflect::Reflect;
use slime_world::{Command, Component, Entity, Name, PersistentId, SceneLibrary, World};

/// What an entity looks at.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "test::Aim")]
struct Aim {
    at: Entity,
}
impl Component for Aim {}

const LAMP: AssetId = AssetId::from_u128(2);

/// A lamp on a post.
const LAMP_FILE: &str = r#"
[entities.post.components]
Name = "post"

[entities.light]
parent = "post"

[entities.light.components]
Name = "light"
"#;

/// The yard, as people wrote it.
const YARD_FILE: &str = r#"# The yard: two lamps, a well, and a sign that points at it.

[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[entities]
# By the gate.
gate = { prefab = "lamp" }
porch = { prefab = "lamp" } # to light the door
well = { components = { Name = "well" } }
sign = { components = { Name = "sign", "test::Aim" = { at = "well" } } }

# The gate's lamp, a step east.
[changed.gate.Transform]
translation = { x = 4.0 }
"#;

/// A directory of the test's own, which holds the yard and its metadata file.
struct Yard {
    directory: PathBuf,
}

impl Yard {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory of the test's own");
        fs::write(directory.join("yard.scene"), YARD_FILE).expect("the yard written");
        fs::write(
            directory.join("yard.scene.meta"),
            "id = \"00000000-0000-0000-0000-000000000001\"\n",
        )
        .expect("its metadata written");
        Self { directory }
    }

    fn file(&self) -> PathBuf {
        self.directory.join("yard.scene")
    }

    fn world() -> World {
        let mut world = World::new();
        world.register::<Aim>();
        world
    }

    fn scenes() -> SceneLibrary {
        let mut scenes = SceneLibrary::new();
        scenes.insert_file(LAMP, "lamp", LAMP_FILE);
        scenes
    }

    /// The yard opened into a new world.
    fn open(&self) -> (World, Editor) {
        let mut world = Self::world();
        let editor =
            Editor::open(&mut world, Self::scenes(), &self.file(), "yard").expect("opened");
        (world, editor)
    }

    /// An editor recovered from `journal` into a new world.
    fn recover(&self, journal: &str) -> Result<(World, Editor), EditError> {
        let mut world = Self::world();
        let file = self.file();
        let editor = Editor::recover(&mut world, Self::scenes(), &file, "yard", journal, None)?;
        Ok((world, editor))
    }
}

impl Drop for Yard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn id(text: &str) -> PersistentId {
    text.parse().expect("an identity")
}

/// The scene file that the world writes of the level, without writing it.
fn level(world: &mut World, editor: &Editor) -> String {
    let root = editor.root(world).expect("the level's root");
    world.write_scene(root).expect("written").text
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("readable")
}

/// A torch created in the yard, its name set, and the porch deleted.
fn light_the_well(world: &mut World, editor: &mut Editor) -> PersistentId {
    let torch = editor.new_id(world, "torch");
    editor
        .apply(
            world,
            "Light the well",
            vec![
                Command::Create {
                    entity: torch.clone(),
                    parent: Some(id("yard")),
                    rank: Some(2),
                    components: vec![("Name".to_owned(), Name("torch".to_owned()).to_value())],
                },
                Command::Delete {
                    entity: id("yard/porch"),
                },
            ],
            Some(vec![torch.clone()]),
        )
        .expect("made");
    torch
}

/// What the sign aims at.
fn aimed(world: &World) -> Entity {
    let sign = world.find(&id("yard/sign")).expect("the sign");
    world.get::<Aim>(sign).expect("its aim").at
}

#[test]
fn a_level_changed_written_and_opened_again_holds_the_changes_and_its_comments() {
    let yard = Yard::new("written");
    let (mut world, mut editor) = yard.open();
    let torch = light_the_well(&mut world, &mut editor);
    let lamp = editor
        .scenes()
        .resolve(world.registry(), LAMP)
        .expect("the lamp");
    let placed = editor.new_id(&world, "lamp");
    editor
        .apply(
            &mut world,
            "Place a lamp",
            vec![Command::Place {
                entity: placed.clone(),
                parent: Some(id("yard")),
                rank: None,
                prefab: lamp,
            }],
            None,
        )
        .expect("made");
    let left_out = editor.write(&mut world).expect("written");
    assert!(left_out.is_empty(), "{left_out:?}");
    let written = read(&yard.file());
    let (own, placed_own) = (&torch.parts()[1], &placed.parts()[1]);
    assert!(written.contains(&format!("[entities.{own}]")), "{written}");
    assert!(
        written.contains(&format!("[entities.{placed_own}]\nprefab = \"lamp\"\n")),
        "{written}"
    );
    assert!(
        !written.contains("porch") && !written.contains("[removed"),
        "{written}"
    );
    for comment in [
        "# The yard: two lamps, a well, and a sign that points at it.",
        "# By the gate.",
        "# The gate's lamp, a step east.",
    ] {
        assert!(written.contains(comment), "{comment}\n{written}");
    }
    assert!(
        !written.contains("# to light the door"),
        "the porch's comment goes with it"
    );
    // Opened again, the level is the one written, and what the editor made has its identities.
    let (mut again, editor_again) = yard.open();
    let found = again.find(&torch).expect("the torch, under its identity");
    assert_eq!(again.get::<Name>(found), Some(&Name("torch".to_owned())));
    let found = again.find(&placed).expect("the lamp, under its identity");
    assert_eq!(again.get::<Name>(found), Some(&Name(placed_own.clone())));
    assert_eq!(level(&mut again, &editor_again), level(&mut world, &editor));
}

#[test]
fn steps_undo_and_redo_whole_and_select_again_what_they_selected() {
    let yard = Yard::new("undo");
    let (mut world, mut editor) = yard.open();
    let before = level(&mut world, &editor);
    editor.select(vec![id("yard/well")]);
    let torch = light_the_well(&mut world, &mut editor);
    let after = level(&mut world, &editor);
    assert_eq!(editor.selection(), std::slice::from_ref(&torch));
    assert_eq!(editor.done().collect::<Vec<_>>(), ["Light the well"]);

    assert!(editor.undo(&mut world).expect("undone"));
    assert_eq!(level(&mut world, &editor), before);
    assert_eq!(editor.selection(), [id("yard/well")]);
    assert_eq!(editor.undone().collect::<Vec<_>>(), ["Light the well"]);
    assert!(!editor.undo(&mut world).expect("nothing to undo"));

    assert!(editor.redo(&mut world).expect("redone"));
    assert_eq!(level(&mut world, &editor), after);
    assert_eq!(editor.selection(), [torch]);
    assert!(!editor.redo(&mut world).expect("nothing to redo"));

    // A new step forgets the steps undone.
    assert!(editor.undo(&mut world).expect("undone"));
    editor
        .apply_text(
            &mut world,
            "Rename the well",
            &["[SetProperty]\nentity = \"yard/well\"\ncomponent = \"Name\"\npath = \"0\"\nvalue = \"old well\"\n"],
            None,
        )
        .expect("made");
    assert_eq!(editor.undone().count(), 0);
    assert_eq!(editor.done().collect::<Vec<_>>(), ["Rename the well"]);
}

#[test]
fn a_step_refused_half_way_leaves_the_level_as_it_was() {
    let yard = Yard::new("refused");
    let (mut world, mut editor) = yard.open();
    let before = level(&mut world, &editor);
    // The well deleted, then restored as the step is refused: the sign aims at it again.
    let refused = editor.apply_text(
        &mut world,
        "Half",
        &[
            "[Delete]\nentity = \"yard/well\"\n",
            "[Reparent]\nentity = \"yard/sign\"\nparent = \"yard/gate\"\n",
            "[Delete]\nentity = \"yard/porch/nowhere\"\n",
        ],
        Some(vec![id("yard/gate")]),
    );
    assert!(
        matches!(refused, Err(EditError::Refused { index: 2, .. })),
        "{refused:?}"
    );
    assert_eq!(level(&mut world, &editor), before);
    assert_eq!(Some(aimed(&world)), world.find(&id("yard/well")));
    assert_eq!(editor.done().count(), 0);
    assert!(editor.selection().is_empty());
    let unreadable = editor.apply_text(
        &mut world,
        "Typo",
        &["[Delet]\nentity = \"yard/porch\"\n"],
        None,
    );
    assert!(
        matches!(unreadable, Err(EditError::Text { index: 0, .. })),
        "{unreadable:?}"
    );
    // An entity that the world does not have is a refusal, whether the command's text needs it.
    let missing = editor.apply_text(
        &mut world,
        "Missing",
        &["[SetProperty]\nentity = \"yard/nowhere\"\ncomponent = \"Name\"\nvalue = \"x\"\n"],
        None,
    );
    assert!(
        matches!(missing, Err(EditError::Refused { index: 0, .. })),
        "{missing:?}"
    );
    assert!(matches!(
        editor.apply(
            &mut world,
            "Nothing",
            Vec::new(),
            Some(vec![id("yard/well")])
        ),
        Err(EditError::Empty)
    ));
    assert_eq!((editor.done().count(), editor.selection().len()), (0, 0));
}

#[test]
fn a_command_that_would_change_what_is_not_under_the_level_s_root_is_refused() {
    let yard = Yard::new("outside");
    let (mut world, mut editor) = yard.open();
    let before = level(&mut world, &editor);
    let refused = [
        (
            id("yard/torch-0rdx"),
            Command::Create {
                entity: id("yard/torch-0rdx"),
                parent: None,
                rank: None,
                components: Vec::new(),
            },
        ),
        (
            id("cellar/torch-0rdx"),
            Command::Create {
                entity: id("cellar/torch-0rdx"),
                parent: Some(id("yard")),
                rank: None,
                components: Vec::new(),
            },
        ),
        (
            id("yard/well"),
            Command::Reparent {
                entity: id("yard/well"),
                parent: None,
                rank: None,
            },
        ),
        (id("yard"), Command::Delete { entity: id("yard") }),
        (
            id("yard"),
            Command::Reparent {
                entity: id("yard"),
                parent: Some(id("yard/well")),
                rank: None,
            },
        ),
    ];
    for (entity, command) in refused {
        let result = editor.apply(&mut world, "Out", vec![command], None);
        assert!(
            matches!(&result, Err(EditError::OutOfLevel { index: 0, entity: out }) if *out == entity),
            "{result:?}"
        );
    }
    assert_eq!(level(&mut world, &editor), before);
    assert_eq!(editor.done().count(), 0);
}

#[test]
fn a_recovered_editor_holds_the_level_the_history_and_the_selection_of_the_one_that_crashed() {
    let yard = Yard::new("journal");
    let journal = yard.directory.join("journal");
    let (mut world, mut editor) = yard.open();
    editor.keep_journal(&world, &journal).expect("kept");
    editor.select(vec![id("yard/gate")]);
    let torch = light_the_well(&mut world, &mut editor);
    // A step whose second command names what its first created, by the identity it was given.
    let sign = editor.new_id(&world, "sign");
    editor
        .apply_text(
            &mut world,
            "Put up a sign",
            &[
                &format!("[Create]\nentity = \"{sign}\"\nparent = \"yard\"\n"),
                &format!(
                    "[Insert]\nentity = \"{sign}\"\ncomponent = \"test::Aim\"\nvalue = {{ at = \"{torch}\" }}\n"
                ),
            ],
            None,
        )
        .expect("made");
    // The level written, then the steps made before the writing undone and done again, and one
    // made after.
    editor.write(&mut world).expect("written");
    let written = read(&yard.file());
    assert!(editor.undo(&mut world).expect("undone"));
    assert!(editor.undo(&mut world).expect("undone"));
    assert!(editor.redo(&mut world).expect("redone"));
    assert!(editor.redo(&mut world).expect("redone"));
    editor.select(vec![id("yard/well")]);
    editor
        .apply(
            &mut world,
            "Move the well",
            vec![Command::Reparent {
                entity: id("yard/well"),
                parent: Some(id("yard/gate")),
                rank: Some(0),
            }],
            None,
        )
        .expect("made");
    assert!(editor.undo(&mut world).expect("undone"));
    let expected = level(&mut world, &editor);
    let text = read(&journal);
    assert!(text.starts_with("# level\n"), "{text}");

    // The editor crashed: the file holds what it last wrote, the journal what it did since.
    let (mut again, mut recovered) = yard.recover(&text).expect("recovered");
    assert_eq!(level(&mut again, &recovered), expected);
    assert_eq!(
        recovered.done().collect::<Vec<_>>(),
        ["Light the well", "Put up a sign"]
    );
    assert_eq!(recovered.undone().collect::<Vec<_>>(), ["Move the well"]);
    assert_eq!(recovered.selection(), editor.selection());
    // Both go on alike.
    for (world, editor) in [(&mut world, &mut editor), (&mut again, &mut recovered)] {
        assert!(editor.redo(world).expect("redone"));
        assert!(editor.undo(world).expect("undone"));
        assert!(editor.undo(world).expect("undone"));
        assert!(editor.redo(world).expect("redone"));
    }
    assert_eq!(level(&mut again, &recovered), level(&mut world, &editor));
    assert_eq!(recovered.selection(), editor.selection());
    let (sign_entity, torch_entity) = (again.find(&sign), again.find(&torch));
    assert_eq!(
        again
            .get::<Aim>(sign_entity.expect("the sign"))
            .map(|aim| aim.at),
        torch_entity
    );

    // A journal started after a step is refused, and so is a journal whose level's file changed
    // apart from the editor, or that undoes a step that it did not make.
    assert!(matches!(
        editor.keep_journal(&world, &yard.directory.join("late")),
        Err(EditError::Journal(_))
    ));
    fs::write(yard.file(), YARD_FILE).expect("the file changed by hand");
    assert!(matches!(yard.recover(&text), Err(EditError::Journal(_))));
    fs::write(yard.file(), &written).expect("written back");
    let wrong = format!("{text}# undo\n# undo\n# undo\n");
    assert!(
        matches!(
            yard.recover(&wrong),
            Err(EditError::Replay { error, .. }) if matches!(*error, EditError::Journal(_))
        ),
        "an undo of no step"
    );
}

#[test]
fn the_history_forgets_the_steps_undone_then_the_oldest_done_past_its_budget_but_the_last() {
    let yard = Yard::new("budget");
    let (mut world, mut editor) = yard.open();
    for step in 0..5 {
        let value = Name(format!("well {step}")).to_value();
        editor
            .apply(
                &mut world,
                &format!("Step {step}"),
                vec![Command::Insert {
                    entity: id("yard/well"),
                    component: "Name".to_owned(),
                    value,
                }],
                None,
            )
            .expect("made");
    }
    assert!(editor.undo(&mut world).expect("undone"));
    assert!(editor.undo(&mut world).expect("undone"));
    assert_eq!((editor.done().count(), editor.undone().count()), (3, 2));
    editor.set_history_budget(1);
    assert_eq!(editor.undone().count(), 0, "the steps undone first");
    assert_eq!(editor.done().collect::<Vec<_>>(), ["Step 2"]);
    assert!(editor.undo(&mut world).expect("undone"));
    let well = world.find(&id("yard/well")).expect("the well");
    assert_eq!(world.get::<Name>(well), Some(&Name("well 1".to_owned())));
}
