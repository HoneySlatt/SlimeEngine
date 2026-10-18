"""The models taken from Poly Haven, made lighter where they are too heavy for a game, and the props
made here: the lantern on its post, the campfire, the sealed door and the key."""

import math

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

import common
import ruins


class Models:
    """The meshes of Poly Haven's glTF files, imported once, each placed again as an object that
    shares its mesh, which the exporter writes once."""

    def __init__(self, sources):
        self.sources = sources
        self.meshes = {}
        # Where each mesh stood in its file: the parts of one model keep their places.
        self.offsets = {}

    def load(self, asset, resolution, triangles=None):
        """The meshes of `asset`, by object name; those of more than `triangles` triangles are
        decimated to about that many."""
        if asset in self.meshes:
            return self.meshes[asset]
        path = self.sources.path(asset, f"{asset}_{resolution}.gltf")
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=path)
        meshes = {}
        # By name: a set's order changes from one run to the next, and with it which mesh each
        # placement of `meshes[i % n]` takes.
        for obj in sorted(set(bpy.data.objects) - before, key=lambda o: o.name):
            if obj.type != "MESH":
                bpy.data.objects.remove(obj)
                continue
            count = sum(len(p.vertices) - 2 for p in obj.data.polygons)
            if triangles and count > triangles:
                modifier = obj.modifiers.new("lighter", "DECIMATE")
                modifier.ratio = triangles / count
                bpy.context.view_layer.objects.active = obj
                bpy.ops.object.modifier_apply(modifier=modifier.name)
            obj.data.use_fake_user = True
            obj.data.name = obj.name
            self.offsets[obj.data.name] = obj.matrix_world.copy()
            meshes[obj.name] = obj.data
            bpy.data.objects.remove(obj)
        self.meshes[asset] = meshes
        return meshes

    def grade(self, asset, saturation):
        """Grades toward the zone's palette (`common.grade`) the color images of the materials of
        `asset`'s meshes, loaded before."""
        def color_images(socket):
            for link in socket.links:
                node = link.from_node
                if node.type == "TEX_IMAGE" and node.image is not None:
                    yield node.image
                for upstream in node.inputs:
                    yield from color_images(upstream)

        for mesh in self.meshes[asset].values():
            for mat in mesh.materials:
                if mat is None or not mat.use_nodes:
                    continue
                for node in mat.node_tree.nodes:
                    if node.type == "BSDF_PRINCIPLED":
                        for image in color_images(node.inputs["Base Color"]):
                            common.grade(image, self.sources.scratch, saturation)

    def place(self, mesh, name, location, yaw=0.0, scale=1.0, tilt=(0.0, 0.0), as_part=False):
        """An object of `mesh` at `location`, turned by `yaw` about the vertical and `tilt` about x
        and y, scaled; `as_part` keeps it where it stood in its model, relative to `location`, as a
        door's leaves keep their places in its frame."""
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        offset = self.offsets.get(mesh.name, Matrix())
        if not as_part:
            offset = offset.to_3x3().to_4x4()
        placement = Matrix.LocRotScale(Vector(location), Euler((tilt[0], tilt[1], yaw)),
                                       Vector((scale, scale, scale)))
        obj.matrix_world = placement @ offset
        return obj


def footprint(mesh, scale=1.0):
    """The half sizes along x and y of `mesh`'s box, and its height, scaled."""
    xs = [v.co.x for v in mesh.vertices]
    ys = [v.co.y for v in mesh.vertices]
    zs = [v.co.z for v in mesh.vertices]
    return ((max(xs) - min(xs)) / 2 * scale, (max(ys) - min(ys)) / 2 * scale,
            (max(zs) - min(zs)) * scale)


def lantern_glass(models):
    """The lanterns' glass, drawn opaque, glowing warm: the renderer draws no transparency."""
    for mesh in models.load("wooden_lantern_01", "1k", 3000).values():
        for mat in mesh.materials:
            if mat and "glass" in mat.name and not mat.get("glowing"):
                bsdf = mat.node_tree.nodes["Principled BSDF"]
                for link in list(bsdf.inputs["Alpha"].links):
                    mat.node_tree.links.remove(link)
                bsdf.inputs["Alpha"].default_value = 1.0
                bsdf.inputs["Emission Color"].default_value = (1.0, 0.62, 0.3, 1.0)
                bsdf.inputs["Emission Strength"].default_value = 250.0
                mat["glowing"] = True


def lantern(models, wood):
    """A lantern on a post, the zone's prefab, which its scene file places six times: the post at
    the origin, its arm reaching 0.8 m north, along +y; the lantern hanging from the arm, its glass
    a node of its own, `glass`, so that a placement can remove it; its collider; and its light at
    night, `light`, a spot light aimed down, warm, which casts shadows. Every node is named for
    what it is, as the scene file names its parts: `lantern 5/glass`."""
    meshes = models.load("wooden_lantern_01", "1k", 3000)
    lantern_glass(models)
    parts = [ruins.box("post", (-0.09, -0.09, 0.0), (0.09, 0.09, 2.6), wood),
             ruins.box("arm", (-0.05, -0.05, 2.4), (0.05, 0.85, 2.5), wood)]
    post = common.join(parts, "post")
    common.world_box_uvs(post, 1.0)
    hang = Vector((0.0, 0.8, 2.45 - 0.62))
    glass = []
    for name, mesh in meshes.items():
        # wooden_lantern_01, wooden_lantern_01_door, wooden_lantern_01_handle.
        part = name.removeprefix("wooden_lantern_01").lstrip("_") or "body"
        obj = models.place(mesh, part, hang, scale=1.5, as_part=True)
        if any(m and "glass" in m.name for m in obj.data.materials):
            glass.append(split_material(obj, "glass", f"{part} glass"))
    if len(glass) > 1:
        common.join(glass, "glass")
    else:
        glass[0].name = "glass"
    common.collider_cylinder("post", (0.0, 0.0), 0.25, 2.6)
    common.light("light", "SPOT", "night", hang + Vector((0, 0, 0.3)), (0.0, 0.0, -1.0),
                 (1.0, 0.66, 0.36), 140.0, cone=(math.radians(62), math.radians(35)))


def split_material(obj, word, name):
    """Moves the faces of `obj` whose material's name holds `word` into an object of their own,
    named `name`, at the same place: `obj` keeps the others. Each keeps only the materials that its
    faces use, so that the exporter writes no empty primitive."""
    obj.data = obj.data.copy()
    apart = bpy.data.objects.new(name, obj.data.copy())
    bpy.context.scene.collection.objects.link(apart)
    apart.matrix_world = obj.matrix_world
    for target, keep in ((obj, False), (apart, True)):
        mesh = target.data
        bm = bmesh.new()
        bm.from_mesh(mesh)
        chosen = {i for i, m in enumerate(mesh.materials) if m and word in m.name}
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if (f.material_index in chosen) != keep],
                         context="FACES")
        bm.to_mesh(mesh)
        bm.free()
        bpy.ops.object.select_all(action="DESELECT")
        bpy.context.view_layer.objects.active = target
        target.select_set(True)
        bpy.ops.object.material_slot_remove_unused()
        target.select_set(False)
    return apart


def campfire(place, models):
    """The stone fire pit, its wood, two stumps to sit on, and its light at night: a point light,
    which casts no shadow."""
    x, y = place["campfire"]["at"]
    pit = models.load("stone_fire_pit", "1k")
    wood = models.load("dry_branches_medium_01", "1k", 2500)
    stump = models.load("tree_stump_01", "1k", 3000)
    objects = [models.place(m, "fire pit", (x, y, 0.0), scale=1.2) for m in pit.values()]
    for k, mesh in enumerate(wood.values()):
        a = k * 2 * math.pi / 3
        objects.append(models.place(mesh, f"firewood {k + 1}",
                                    (x + 0.15 * math.cos(a), y + 0.15 * math.sin(a), 0.12),
                                    yaw=a, scale=0.6, tilt=(0.35, 0.0)))
    seats = ((2.4, -1.4, 0.6), (-1.9, 2.1, 2.4))
    for k, (dx, dy, yaw) in enumerate(seats):
        for mesh in stump.values():
            objects.append(models.place(mesh, f"stump seat {k + 1}", (x + dx, y + dy, -0.05), yaw=yaw,
                                        scale=0.7))
    # The fire: a bed of embers and three crossed tongues of flame, glowing by day and night.
    embers = common.material("embers", (0.05, 0.03, 0.02), roughness=0.9)
    flame = common.material("flames", (0.0, 0.0, 0.0), roughness=1.0, double_sided=True)
    for mat, color, nits in ((embers, (1.0, 0.42, 0.12), 900.0), (flame, (1.0, 0.56, 0.2), 2600.0)):
        bsdf = mat.node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Emission Color"].default_value = (*color, 1.0)
        bsdf.inputs["Emission Strength"].default_value = nits
    objects.append(ruins.prism("embers", (x, y), 0.42, 0.04, 0.13, embers, sides=10))
    tongues = []
    for k in range(3):
        a = k * math.pi / 3
        d = Vector((math.cos(a), math.sin(a), 0.0)) * 0.26
        tongues.append(common.mesh_object("flame", [Vector((x, y, 0.12)) - d, Vector((x, y, 0.12)) + d,
                                                    Vector((x, y, 0.85))], [(0, 1, 2)], material=flame))
    objects.append(common.join(tongues, "flames"))
    colliders = [common.collider_cylinder("fire pit", (x, y), 1.0, 0.5)]
    colliders += [common.collider_cylinder(f"stump seat {k + 1}", (x + dx, y + dy), 0.55, 0.4)
                  for k, (dx, dy, _) in enumerate(seats)]
    marker = common.marker("campfire", "campfire", (x, y, 0.0))
    # 120 candelas: a small fire, a little below a lantern, which lights all around it, and which
    # flickers around that intensity.
    light = common.light("campfire light", "POINT", "night", (x, y, 0.6), (0.0, 0.0, -1.0),
                         (1.0, 0.55, 0.25), 120.0)
    common.component(light, "hunnawood::Flickers", f"{{ intensity = {common.number(120.0)} }}")
    return objects, colliders, marker, light


def door(place, models):
    """The sealed door in the chapel's north wall: its frame, and its two leaves under the `door`
    marker with the collider that closes the opening, which the game removes once the door
    opens."""
    meshes = models.load("large_castle_door", "1k")
    y = place["chapel"]["y"][1] + ruins.WALL / 2
    marker = common.marker("door", "door", (0.0, y, 0.0))
    objects = []
    for name, mesh in meshes.items():
        obj = models.place(mesh, name, (0.0, y, 0.0), scale=1.03, as_part=True)
        if "frame" not in name:
            common.reparent(obj, marker)
        objects.append(obj)
    common.collider_box("door", (0.0, y), (2.2, 0.6), 3.0, parent=marker)
    return objects, marker


def key(place):
    """The item that opens the door: a large old key of brass, 0.7 m long, on a fallen block in
    the nave, under the `item` marker, which the game hides once the hero picks it up. Polished, it
    catches the day's light; at night it glints faintly, enough to be found by."""
    x, y = place["item"]["at"]
    brass = common.material("brass", (0.85, 0.66, 0.32), roughness=0.3, metallic=1.0)
    bsdf = brass.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Emission Color"].default_value = (1.0, 0.75, 0.35, 1.0)
    bsdf.inputs["Emission Strength"].default_value = 6.0
    marker = common.marker("item", "item", (x, y, 0.0))
    stone_block = ruins.box("item block", (x - 0.35, y - 0.3, 0.0), (x + 0.35, y + 0.3, 0.5), None)
    # Made about its middle at its true size, then scaled up and laid on the block: the bow, a
    # ring of eight segments; the shaft; the bit, two teeth.
    scale = 2.2
    parts = []
    for k in range(8):
        a, b = 2 * math.pi * k / 8, 2 * math.pi * (k + 1) / 8
        p = Vector((-0.12 + 0.05 * math.cos(a), 0.05 * math.sin(a), 0.0))
        q = Vector((-0.12 + 0.05 * math.cos(b), 0.05 * math.sin(b), 0.0))
        mid = (p + q) / 2
        parts.append(ruins.box("bow", mid - Vector((0.012, 0.012, 0.012)),
                               mid + Vector((0.012, 0.012, 0.012)), brass))
    parts.append(ruins.box("shaft", (-0.07, -0.008, -0.008), (0.16, 0.008, 0.008), brass))
    parts.append(ruins.box("tooth", (0.10, -0.04, -0.006), (0.12, 0.0, 0.006), brass))
    parts.append(ruins.box("tooth", (0.14, -0.03, -0.006), (0.16, 0.0, 0.006), brass))
    obj = common.join(parts, "key")
    obj.location = (x, y, 0.5 + 0.03 * scale)
    obj.rotation_euler = (0.0, 0.0, 0.4)
    obj.scale = (scale, scale, scale)
    common.reparent(obj, marker)
    return stone_block, marker
