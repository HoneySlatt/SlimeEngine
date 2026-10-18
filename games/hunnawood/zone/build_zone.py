"""Builds Hunna Wood's zone in Blender from its source assets and exports it as glTF.

Run it with Blender 5.2, from the repository root, once `scripts/fetch-assets.sh` has fetched the
sources:

    blender --background --factory-startup --python games/hunnawood/zone/build_zone.py
    blender --background --factory-startup --python games/hunnawood/zone/build_zone.py -- --preview DIR

It reads `games/hunnawood/assets/sources/` and `placements.toml`, beside this file, and writes
three glTF files into `games/hunnawood/assets/zone/`, each with its buffer, and their images:
`land.gltf`, the zone but its lanterns; `lantern.gltf`, a lantern on its post, which the zone's
scene file, `zone.scene`, written by hand, places six times beside the land; and `hero.gltf`.
With `--preview DIR`, it also renders views of the zone into DIR, with Blender's EEVEE, the
lanterns where the scene file places them. Everything the script makes is released under CC0 1.0,
as the sources it reads are.

The zone is a forest path: the hero starts in the south, crosses a gully on a stone bridge, passes
a campfire in a clearing, and reaches a ruined chapel whose north wall holds a sealed door; behind
it, a walled terrace on the edge of a cliff. Besides meshes, the export holds what the game reads,
as the game's components, typed properties of the nodes: colliders (`hunnawood::Collider`, a box or
a cylinder with its height; an empty whose scale is half the box's size, or the cylinder's
radius), markers (`hunnawood::Marker`: the start, the item, the door, the campfire, the terrace,
the bounds; the item's and the door's meshes and collider are the marker's children), the time of
day of the lights (`hunnawood::TimeOfDay`), and the campfire's flicker (`hunnawood::Flickers`).
Every node has a name of its own, and the script fails when Blender numbers one. The game's
README describes them.
"""

import math
import os
import sys
import tomllib

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
# The script's modules, imported from here, leave no compiled files beside them.
sys.dont_write_bytecode = True

import backdrop  # noqa: E402
import common  # noqa: E402
import hero  # noqa: E402
import land  # noqa: E402
import props  # noqa: E402
import ruins  # noqa: E402
import vegetation  # noqa: E402

ASSETS = os.path.normpath(os.path.join(HERE, "..", "assets"))
LAND = os.path.join(ASSETS, "zone", "land.gltf")
LANTERN = os.path.join(ASSETS, "zone", "lantern.gltf")
HERO = os.path.join(ASSETS, "zone", "hero.gltf")
# The zone's scene file, written by hand, which places the land and the lanterns.
SCENE = os.path.join(ASSETS, "zone", "zone.scene")
# The day is overcast: a sun of 12 000 lux behind the clouds, from the south-west, 48 degrees
# high. The night is lit by the moon, the lanterns and the campfire.
SUN = (12_000.0, (0.5, 0.5, -0.8), (1.0, 0.97, 0.92))
MOON = (0.8, (-0.4, 0.3, -0.85), (0.62, 0.72, 1.0))


def arguments():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    preview = None
    if args[:1] == ["--preview"] and len(args) == 2:
        preview = args[1]
    elif args:
        raise SystemExit("usage: build_zone.py [-- --preview DIR]")
    return preview


def build(place, sources):
    seed = place["zone"]["seed"]
    materials = {
        # The ground and the stone, photographed warmer than the zone's palette, graded toward it.
        "forest floor": common.poly_haven_set(sources, "forest_leaves_02", "2k", tint=(0.62, 0.7, 0.8),
                                              saturation=0.55),
        "path": common.poly_haven_set(sources, "stony_dirt_path", "2k", saturation=0.45),
        "stone": common.poly_haven_set(sources, "medieval_blocks_02", "2k", tint=(0.62, 0.66, 0.72),
                                       saturation=0.4),
        "oak bark": common.poly_haven_set(sources, "jolcham_oak_bark_01", "1k"),
        "pine bark": common.poly_haven_set(sources, "pine_bark", "1k"),
    }
    stone = materials["stone"]
    land.build(place, materials)
    ruins.bridge(place, stone)
    ruins.chapel(place, stone)
    models = props.Models(sources)
    props.campfire(place, models)
    props.door(place, models)
    block, _ = props.key(place)
    block.data.materials.append(stone)
    common.world_box_uvs(block, ruins.STONE_REPEAT)
    x, y = place["item"]["at"]
    common.collider_box("item block", (x, y), (0.7, 0.6), 0.5)
    trees = forest(place, sources, models, materials, seed)
    backdrop.build(sources, place, sources.scratch, trees)
    markers(place)
    common.light("sun", "SUN", "day", (0.0, 0.0, 30.0), SUN[1], SUN[2], SUN[0])
    common.light("moon", "SUN", "night", (0.0, 0.0, 30.0), MOON[1], MOON[2], MOON[0])
    return models, materials


def forest(place, sources, models, materials, seed):
    random = common.rng(seed)
    path = land.smooth(place["path"]["points"], 1.0) + land.smooth(place["path"]["branch"], 1.0)
    keepout = vegetation.Keepout(place, path)
    scratch = sources.scratch
    twigs = vegetation.twig_atlas(sources, os.path.join(scratch, "oak_leaves.png"))
    leaves = vegetation.cards_material("oak leaves", twigs)
    needle_color = sources.from_zip("LeafSet019/LeafSet019_2K-JPG.zip", "LeafSet019_2K-JPG_Color.jpg")
    needle_alpha = sources.from_zip("LeafSet019/LeafSet019_2K-JPG.zip", "LeafSet019_2K-JPG_Opacity.jpg")
    needles = vegetation.cards_material("conifer needles", needle_color, needle_alpha)
    for image in (common.load_image(needle_color), common.load_image(needle_alpha, data=True)):
        image.scale(1024, 1024)

    def variants(make, count, bark, cards, first_seed):
        out = []
        for k in range(count):
            wood, crown, radius = make(f"{make.__name__} {k + 1}", first_seed + k, bark, cards)
            out.append((wood.data, crown.data, radius))
            for obj in (wood, crown):
                obj.data.use_fake_user = True
                bpy.data.objects.remove(obj)
        return out

    oaks = variants(vegetation.oak, 4, materials["oak bark"], leaves, 100)
    conifers = variants(vegetation.conifer, 2, materials["pine bark"], needles, 200)
    taken = []
    scatter = place["scatter"]
    for kind, protos, count, spacing in (("oak", oaks, scatter["oaks"], 5.0),
                                          ("conifer", conifers, scatter["conifers"], 4.0)):
        for i, (x, y, _) in enumerate(vegetation.scatter_points(place, keepout, count, 1.8, spacing,
                                                                random, taken)):
            wood, crown, radius = protos[i % len(protos)]
            scale = random.uniform(0.85, 1.2)
            yaw = random.uniform(0, 2 * math.pi)
            crown_part = "leaves" if kind == "oak" else "needles"
            for mesh, part in ((wood, "wood"), (crown, crown_part)):
                models.place(mesh, f"{kind} {i + 1} {part}", (x, y, 0.0), yaw, scale)
            if vegetation.inside(place, x, y, 1.0):
                common.collider_cylinder(f"{kind} {i + 1}", (x, y), radius * scale + 0.05, 8.0)
            taken.append((x, y, spacing))
    rocks(place, models, keepout, random, taken)
    ferns(place, models, keepout, random, taken)
    return oaks + conifers


def rocks(place, models, keepout, random, taken):
    """Mossy rocks in the forest and in the gully, logs and stumps, and rock faces along the
    gully, the cliff and the edges of the ground."""
    rock_meshes = list(models.load("rock_moss_set_01", "1k", 2500).values())
    rock_meshes += list(models.load("rock_moss_set_02", "1k", 2500).values())
    for asset in ("rock_moss_set_01", "rock_moss_set_02"):
        models.grade(asset, 0.45)
    for i, (x, y, _) in enumerate(vegetation.scatter_points(place, keepout, place["scatter"]["rocks"],
                                                            1.2, 2.5, random, taken)):
        mesh = rock_meshes[i % len(rock_meshes)]
        scale = random.uniform(0.6, 1.3)
        hx, hy, h = props.footprint(mesh, scale)
        models.place(mesh, f"rock {i + 1}", (x, y, -0.12 * h), random.uniform(0, 2 * math.pi), scale)
        if vegetation.inside(place, x, y, 1.0):
            common.collider_cylinder(f"rock {i + 1}", (x, y), 0.8 * max(hx, hy), h)
    ravine = place["ravine"]
    for i in range(10):
        x = random.uniform(-40, 40)
        if -2 < x < 6:
            continue
        mesh = rock_meshes[random.randrange(len(rock_meshes))]
        y = random.uniform(ravine["south"] + 2.0, ravine["north"] - 2.0)
        models.place(mesh, f"gully rock {i + 1}", (x, y, -ravine["depth"] - 0.1),
                     random.uniform(0, 2 * math.pi), random.uniform(0.7, 1.4))
    faces = list(models.load("rock_face_01", "2k", 6000).values())
    faces += list(models.load("rock_face_02", "2k", 6000).values())
    for asset in ("rock_face_01", "rock_face_02"):
        models.grade(asset, 0.45)
    cliff = place["chapel"]["cliff"]
    hx_zone, hy_zone = place["zone"]["half_size"]
    spots = [
        # In the gully's walls, facing into it.
        (-22.0, ravine["south"] + 0.6, -1.6, 0.0, 1.6), (-9.0, ravine["north"] - 0.6, -1.6, math.pi, 1.5),
        (13.0, ravine["south"] + 0.6, -1.6, 0.0, 1.7), (26.0, ravine["north"] - 0.6, -1.6, math.pi, 1.5),
        # On the edge of the cliff, past the terrace.
        (-26.0, cliff + 0.8, -2.5, math.pi, 2.6), (-14.0, cliff + 0.8, -2.0, math.pi, 2.2),
        (14.0, cliff + 0.8, -2.0, math.pi, 2.2), (26.0, cliff + 0.8, -2.5, math.pi, 2.6),
        # Outcrops beyond the edges of the ground.
        (-hx_zone - 3.0, -35.0, 0.0, -math.pi / 2, 2.0), (-hx_zone - 3.0, 18.0, 0.0, -math.pi / 2, 2.2),
        (hx_zone + 3.0, -28.0, 0.0, math.pi / 2, 2.0), (hx_zone + 3.0, 30.0, 0.0, math.pi / 2, 2.4),
        (-12.0, -hy_zone - 4.0, 0.0, 0.0, 2.0), (14.0, -hy_zone - 4.0, 0.0, 0.0, 2.2),
    ]
    for i, (x, y, z, yaw, scale) in enumerate(spots):
        models.place(faces[i % len(faces)], f"rock face {i + 1}", (x, y, z), yaw, scale)
    logs = list(models.load("dead_tree_trunk", "1k", 5000).values())
    stumps = list(models.load("tree_stump_01", "1k", 3000).values())
    for i, (x, y, _) in enumerate(vegetation.scatter_points(place, keepout, 3, 2.0, 6.0, random, taken)):
        yaw = random.uniform(0, 2 * math.pi)
        models.place(logs[0], f"log {i + 1}", (x, y, 0.15), yaw, 1.8)
        if vegetation.inside(place, x, y, 1.0):
            common.collider_box(f"log {i + 1}", (x, y), (5.6, 0.6), 0.6, yaw)
    for i, (x, y, _) in enumerate(vegetation.scatter_points(place, keepout, 4, 1.5, 4.0, random, taken)):
        models.place(stumps[0], f"stump {i + 1}", (x, y, -0.05), random.uniform(0, 2 * math.pi), 1.0)
        if vegetation.inside(place, x, y, 1.0):
            common.collider_cylinder(f"stump {i + 1}", (x, y), 0.6, 0.6)


def ferns(place, models, keepout, random, taken):
    """Ferns under the trees, on the edges of the path: the alpha of their leaves, which Poly
    Haven's glTF file leaves out, is put back from its own image."""
    meshes = list(models.load("fern_02", "1k").values())
    alpha = models.sources.path("fern_02", "textures", "fern_02_alpha_1k.png")
    for mat in {m for mesh in meshes for m in mesh.materials if m}:
        common.cut(mat, common.image_node(mat, common.load_image(alpha, data=True)).outputs["Color"])
    for i, (x, y, _) in enumerate(vegetation.scatter_points(place, keepout, place["scatter"]["ferns"],
                                                            0.3, 1.1, random, [], near_path=12.0)):
        models.place(meshes[i % len(meshes)], f"fern {i + 1}", (x, y, 0.0),
                     random.uniform(0, 2 * math.pi), random.uniform(1.3, 2.2))


def markers(place):
    x, y = place["start"]["at"]
    common.marker("start", "start", (x, y, 0.0), math.radians(place["start"]["facing"]))
    chapel = place["chapel"]
    terrace_y = (chapel["y"][1] + ruins.WALL + chapel["terrace_north"]) / 2
    common.marker("terrace", "terrace", (0.0, terrace_y, 0.0))
    hx, hy = place["zone"]["half_size"]
    bounds = common.marker("bounds", "bounds", (0.0, 0.0, 0.0))
    bounds.empty_display_type = "CUBE"
    bounds.scale = (hx, hy, 1.0)


def export(path, extras=True):
    """Writes the scene to `path` as glTF, its buffer and images beside it; with `extras`, the
    custom properties that the game reads."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    # Under any other render engine, the exporter reads a light's strength from its node tree.
    bpy.context.scene.render.engine = "BLENDER_EEVEE"
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLTF_SEPARATE",
        export_texture_dir="textures",
        export_apply=True,
        export_yup=True,
        export_normals=True,
        export_tangents=True,
        export_texcoords=True,
        export_materials="EXPORT",
        export_image_format="AUTO",
        export_animations=False,
        export_skins=False,
        export_morph=False,
        export_cameras=False,
        export_lights=True,
        export_import_convert_lighting_mode="SPEC",
        export_extras=extras,
        use_selection=False,
    )


def report(what):
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    triangles = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in meshes)
    primitives = sum(max(1, len(o.data.materials)) for o in meshes)
    print(f"{what}: {len(meshes)} mesh objects, about {primitives} instances drawn, "
          f"{triangles} triangles, {len(bpy.data.images)} images")


def preview_lanterns(models, wood):
    """The lanterns, for the views of `--preview` only, which the export of the land holds none of:
    the prefab built once, in a collection of its own, then shown where the zone's scene file
    places each lantern, read from its `Transform` (glTF's axes: x, y up, z south; a turn about y)."""
    with open(SCENE, "rb") as f:
        scene = tomllib.load(f)
    collection = bpy.data.collections.new("lantern")
    before = set(bpy.context.scene.objects)
    props.lantern(models, wood)
    for obj in set(bpy.context.scene.objects) - before:
        bpy.context.scene.collection.objects.unlink(obj)
        collection.objects.link(obj)
    for name, changed in scene.get("changed", {}).items():
        transform = changed.get("Transform")
        if not name.startswith("lantern") or transform is None:
            continue
        at, turn = transform.get("translation", {}), transform.get("rotation", {})
        shown = bpy.data.objects.new(f"{name} shown", None)
        shown.instance_type = "COLLECTION"
        shown.instance_collection = collection
        shown.location = (at.get("x", 0.0), -at.get("z", 0.0), at.get("y", 0.0))
        shown.rotation_euler = (0.0, 0.0, 2 * math.atan2(turn.get("y", 0.0), turn.get("w", 1.0)))
        bpy.context.scene.collection.objects.link(shown)


def main():
    preview = arguments()
    with open(os.path.join(HERE, "placements.toml"), "rb") as f:
        place = tomllib.load(f)
    scratch = os.path.join(ASSETS, "build")
    os.makedirs(scratch, exist_ok=True)
    sources = common.Sources(os.path.join(ASSETS, "sources"), scratch)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    models, materials = build(place, sources)
    report("land")
    common.check_names()
    export(LAND)
    if preview:
        import preview as views
        preview_lanterns(models, materials["pine bark"])
        views.render(preview)
    # The lantern, the zone's prefab, in a file of its own: the zone's scene file places it.
    bpy.ops.wm.read_factory_settings(use_empty=True)
    props.lantern(props.Models(sources), common.poly_haven_set(sources, "pine_bark", "1k"))
    report("lantern")
    common.check_names()
    export(LANTERN)
    # The hero, in a file of its own: the game draws it where the hero stands.
    bpy.ops.wm.read_factory_settings(use_empty=True)
    hero.build(sources)
    common.check_names()
    # Without extras: the body keeps properties of the bundle's add-ons, which the game does not read.
    export(HERO, extras=False)


main()
