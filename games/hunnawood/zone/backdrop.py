"""What stands beyond the zone, painted on cards: the region's capital on its cliff, north of the
terrace, past a misty valley crossed by aqueducts; and a line of trees around the other sides, so
that the forest seems to go on. Each is rendered once with Cycles into an image whose alpha cuts
out the sky, then shown on quads far away, which the game's sky and light reach like any other
surface."""

import math
import os

import bpy
import numpy as np
from mathutils import Vector

import common
import ruins
import vegetation

# The capital's picture: rendered from the terrace, looking north, onto a card this far away.
VISTA_SIZE = (4096, 1536)
VISTA_FOV = math.radians(110)
VISTA_DISTANCE = 520.0
TREELINE_SIZE = (4096, 512)


def scene(name):
    scene = bpy.data.scenes.new(name)
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 48
    scene.cycles.use_denoising = True
    scene.render.film_transparent = True
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    world = bpy.data.worlds.new(name)
    world.use_nodes = True
    background = world.node_tree.nodes["Background"]
    background.inputs["Color"].default_value = (0.62, 0.66, 0.72, 1.0)
    background.inputs["Strength"].default_value = 0.8
    scene.world = world
    return scene


def link(scene, obj):
    for collection in obj.users_collection:
        collection.objects.unlink(obj)
    scene.collection.objects.link(obj)


def render(scene, path, camera, size):
    scene.camera = camera
    scene.render.resolution_x, scene.render.resolution_y = size
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True, scene=scene.name)


def emissive(name, color, strength):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (0.02, 0.02, 0.02, 1.0)
    bsdf.inputs["Emission Color"].default_value = (*color, 1.0)
    bsdf.inputs["Emission Strength"].default_value = strength
    return mat


def hazed(view, cam, path, size, haze, start, depth):
    """Renders `view` into `path`, then fades what is far into `haze`: the valley's air, which the
    engine does not draw. A second render gives each pixel its distance, from `start` to `start` +
    `depth` metres, as a factor."""
    render(view, path, cam, size)
    mist = bpy.data.materials.new("mist")
    mist.use_nodes = True
    tree = mist.node_tree
    tree.nodes.clear()
    out = tree.nodes.new("ShaderNodeOutputMaterial")
    data = tree.nodes.new("ShaderNodeCameraData")
    ramp = tree.nodes.new("ShaderNodeMapRange")
    ramp.inputs["From Min"].default_value = start
    ramp.inputs["From Max"].default_value = start + depth
    emission = tree.nodes.new("ShaderNodeEmission")
    tree.links.new(data.outputs["View Distance"], ramp.inputs["Value"])
    tree.links.new(ramp.outputs["Result"], emission.inputs["Strength"])
    tree.links.new(emission.outputs["Emission"], out.inputs["Surface"])
    view.view_layers[0].material_override = mist
    distance_path = path + ".distance.png"
    render(view, distance_path, cam, size)
    view.view_layers[0].material_override = None
    color = bpy.data.images.load(path)
    distance = bpy.data.images.load(distance_path)
    pixels = np.empty(len(color.pixels), dtype=np.float32)
    color.pixels.foreach_get(pixels)
    factor = np.empty(len(distance.pixels), dtype=np.float32)
    distance.pixels.foreach_get(factor)
    pixels = pixels.reshape(-1, 4)
    f = factor.reshape(-1, 4)[:, :1]
    pixels[:, :3] = pixels[:, :3] * (1.0 - f) + np.array(haze, dtype=np.float32) * f
    color.pixels.foreach_set(pixels.ravel())
    color.save()
    bpy.data.images.remove(color)
    bpy.data.images.remove(distance)
    os.remove(distance_path)


def capital(stone, rock, windows, origin, random):
    """The capital: a cliff rising from the valley, houses stacked on its ledges, the cathedral
    fortress on its top, and aqueducts crossing the valley in front of it. Returns its objects."""
    objects = []
    base_y = origin.y + 1400.0
    # The cliff: a rough wall of rock, its face toward the terrace, with ledges.
    columns, rows = 60, 24
    v, faces, uvs = [], [], []
    for i in range(columns + 1):
        x = -700.0 + 1400.0 * i / columns
        crest = 140.0 + 40.0 * math.sin(i * 0.37) + random.uniform(-12, 12)
        if abs(x) < 120:
            crest += 30.0 * (1 - abs(x) / 120)
        for r in range(rows + 1):
            t = r / rows
            z = -160.0 + (crest + 160.0) * t
            y = base_y + random.uniform(-14, 14) - 40.0 * (1 - t) ** 2
            v.append((x, y, z))
            uvs.append((x / 25.0, z / 25.0))
    for i in range(columns):
        for r in range(rows):
            a = i * (rows + 1) + r
            faces.append((a, a + rows + 1, a + rows + 2, a + 1))
    corner_uvs = [uvs[k] for f in faces for k in f]
    objects.append(common.mesh_object("capital cliff", v, faces, corner_uvs, rock))
    # Houses on four ledges: dark stone, pitched roofs, warm windows.
    for k in range(170):
        x = random.uniform(-200, 200) * (0.6 + 0.4 * random.random())
        t = random.choice((0.45, 0.6, 0.74, 0.86)) + random.uniform(-0.03, 0.03)
        z = -160.0 + 300.0 * t
        y = base_y - 40.0 * (1 - t) ** 2 - random.uniform(6, 30)
        w, d, h = random.uniform(10, 22), random.uniform(8, 14), random.uniform(10, 26)
        objects.append(house(x, y, z, w, d, h, stone, windows, random))
    # The cathedral fortress on the crest, on a plinth of masonry rising from the rock: a nave,
    # towers and spires.
    top = 175.0
    objects.append(ruins.box("cathedral plinth", (-60, base_y - 6, 60), (60, base_y + 46, top), stone))
    objects.append(ruins.box("cathedral nave", (-40, base_y + 10, top), (40, base_y + 40, top + 45), stone))
    objects.append(roof(-40, 40, base_y + 10, base_y + 40, top + 45, 22, stone))
    for x in (-52, -28, 28, 52):
        h = 85 if abs(x) < 40 else 65
        objects.append(ruins.box("tower", (x - 7, base_y + 8, top), (x + 7, base_y + 22, top + h), stone))
        objects.append(spire(x, base_y + 15, top + h, 9, 45 if abs(x) < 40 else 30, stone))
    for x in range(-35, 40, 10):
        objects.append(spire(x, base_y + 10, top + 45, 2.5, 14, stone))
    # The aqueduct: two tiers of pointed arches crossing the valley from the west to the city.
    y = origin.y + 900.0
    for (bottom, top_z, width) in ((-220.0, -30.0, 30.0), (-30.0, 12.0, 15.0)):
        wall = ruins.box("aqueduct", (-640.0, y - 5, bottom), (-80.0, y + 5, top_z), stone)
        holes = []
        x = -640.0 + width
        while x < -80.0 - width:
            spring = top_z - width * 0.87 - 6.0
            holes.append(ruins.arch_prism("aqueduct arch", False, (x, y), width * 0.8, bottom - 1,
                                          spring, 14.0))
            x += width
        objects.append(ruins.cut_out(wall, holes))
    return objects


def house(x, y, z, w, d, h, stone, windows, random):
    parts = [ruins.box("house", (x - w / 2, y - d / 2, z), (x + w / 2, y + d / 2, z + h), stone),
             roof(x - w / 2, x + w / 2, y - d / 2, y + d / 2, z + h, random.uniform(3, 6), stone)]
    for _ in range(random.randint(0, 3)):
        wx = x + random.uniform(-w / 2 + 1, w / 2 - 1)
        wz = z + random.uniform(1.5, h - 1.5)
        parts.append(ruins.box("window", (wx - 0.5, y - d / 2 - 0.1, wz - 0.8), (wx + 0.5, y - d / 2 + 0.05, wz + 0.8), windows))
    return common.join(parts, "house")


def roof(x0, x1, y0, y1, z, height, mat):
    v = [(x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z), (x0, (y0 + y1) / 2, z + height), (x1, (y0 + y1) / 2, z + height)]
    faces = [(0, 1, 5, 4), (2, 3, 4, 5), (0, 4, 3), (1, 2, 5)]
    return common.mesh_object("roof", v, faces, material=mat)


def spire(x, y, z, radius, height, mat):
    v = [(x - radius, y - radius, z), (x + radius, y - radius, z), (x + radius, y + radius, z),
         (x - radius, y + radius, z), (x, y, z + height)]
    faces = [(0, 3, 2, 1), (0, 1, 4), (1, 2, 4), (2, 3, 4), (3, 0, 4)]
    return common.mesh_object("spire", v, faces, material=mat)


def vista(sources, place, scratch):
    """Renders the capital as seen from the terrace into a color image and a night image of its
    lit windows alone; returns their paths."""
    color = os.path.join(scratch, "capital.png")
    lights = os.path.join(scratch, "capital_windows.png")
    if os.path.exists(color) and os.path.exists(lights):
        return color, lights
    view = scene("capital")
    random = common.rng(41)
    chapel = place["chapel"]
    origin = Vector((0.0, (chapel["y"][1] + chapel["terrace_north"]) / 2, 1.8))
    stone = common.poly_haven_set(sources, "medieval_blocks_02", "2k", tint=(0.42, 0.45, 0.52))
    rock = common.textured("capital rock", sources.path("rock_face_01", "textures", "rock_face_01_diff_2k.jpg"),
                           tint=(0.5, 0.53, 0.6))
    windows = emissive("capital windows", (1.0, 0.62, 0.3), 30.0)
    objects = capital(stone, rock, windows, origin, random)
    for obj in objects:
        link(view, obj)
    sun = bpy.data.lights.new("capital sun", "SUN")
    sun.energy = 2.0
    sun_obj = bpy.data.objects.new("capital sun", sun)
    sun_obj.rotation_euler = (math.radians(50), 0.0, math.radians(-140))
    view.collection.objects.link(sun_obj)
    camera = bpy.data.cameras.new("capital camera")
    camera.angle = VISTA_FOV
    camera.sensor_fit = "HORIZONTAL"
    camera.clip_end = 5000.0
    cam = bpy.data.objects.new("capital camera", camera)
    cam.location = origin
    cam.rotation_euler = (math.pi / 2, 0.0, 0.0)
    view.collection.objects.link(cam)
    hazed(view, cam, color, VISTA_SIZE, (0.6, 0.63, 0.68), 300.0, 2600.0)
    # The windows alone, for the night: everything else black.
    black = common.material("black", (0.0, 0.0, 0.0), roughness=1.0)
    for obj in objects:
        for slot in obj.material_slots:
            if slot.material is not windows:
                slot.material = black
    view.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.0
    sun.energy = 0.0
    render(view, lights, cam, (VISTA_SIZE[0] // 2, VISTA_SIZE[1] // 2))
    bpy.data.scenes.remove(view)
    return color, lights


def treeline(trees, scratch):
    """Renders a line of the zone's trees, seen from the side, into an image: the forest that goes
    on beyond the zone."""
    path = os.path.join(scratch, "treeline.png")
    if os.path.exists(path):
        return path
    view = scene("treeline")
    random = common.rng(43)
    width = 128.0
    x = -width / 2
    while x < width / 2:
        wood, crown, _ = trees[random.randrange(len(trees))]
        for mesh in (wood, crown):
            obj = bpy.data.objects.new("treeline tree", mesh)
            obj.location = (x, random.uniform(-6, 6), 0.0)
            obj.rotation_euler = (0.0, 0.0, random.uniform(0, 2 * math.pi))
            s = random.uniform(0.9, 1.3)
            obj.scale = (s, s, s)
            view.collection.objects.link(obj)
        x += random.uniform(2.5, 5.0)
    sun = bpy.data.lights.new("treeline sun", "SUN")
    sun.energy = 2.0
    sun_obj = bpy.data.objects.new("treeline sun", sun)
    sun_obj.rotation_euler = (math.radians(55), 0.0, math.radians(20))
    view.collection.objects.link(sun_obj)
    camera = bpy.data.cameras.new("treeline camera")
    camera.type = "ORTHO"
    camera.ortho_scale = width
    cam = bpy.data.objects.new("treeline camera", camera)
    cam.location = (0.0, -60.0, width * TREELINE_SIZE[1] / TREELINE_SIZE[0] / 2)
    cam.rotation_euler = (math.pi / 2, 0.0, 0.0)
    view.collection.objects.link(cam)
    hazed(view, cam, path, TREELINE_SIZE, (0.55, 0.58, 0.62), 40.0, 120.0)
    bpy.data.scenes.remove(view)
    return path


def card(name, center, right, up, width, height, mat):
    """A quad of `width` by `height` around `center`, spanned by the unit vectors `right` and
    `up`, showing the whole of `mat`'s image."""
    c, r, u = Vector(center), Vector(right) * width / 2, Vector(up) * height / 2
    corners = [c - r - u, c + r - u, c + r + u, c - r + u]
    return common.mesh_object(name, corners, [(0, 1, 2, 3)], [(0, 0), (1, 0), (1, 1), (0, 1)], mat)


def build(sources, place, scratch, trees):
    """Renders the pictures, then places their cards in the zone."""
    color, lights = vista(sources, place, scratch)
    mat = vegetation.cards_material("capital", color)
    mat.use_backface_culling = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    glow = common.image_node(mat, common.load_image(lights))
    mat.node_tree.links.new(glow.outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = 1.0
    chapel = place["chapel"]
    origin = Vector((0.0, (chapel["y"][1] + chapel["terrace_north"]) / 2, 1.8))
    width = 2 * VISTA_DISTANCE * math.tan(VISTA_FOV / 2)
    height = width * VISTA_SIZE[1] / VISTA_SIZE[0]
    card("capital", origin + Vector((0.0, VISTA_DISTANCE, 0.0)), (1, 0, 0), (0, 0, 1), width, height, mat)
    line = treeline(trees, scratch)
    trees_mat = vegetation.cards_material("forest beyond", line)
    trees_mat.use_backface_culling = True
    span = 128.0
    tall = span * TREELINE_SIZE[1] / TREELINE_SIZE[0]
    ex, ey = place["scatter"]["extent"]
    ring = [
        # Facing the zone from the west, the east and the south; the cliff closes the north.
        ((-ex - 22.0, y), (0, -1, 0)) for y in (-110.0, 0.0, 110.0)
    ] + [((ex + 22.0, y), (0, 1, 0)) for y in (-110.0, 0.0, 110.0)] + [
        ((x, -ey - 22.0), (1, 0, 0)) for x in (-110.0, 0.0, 110.0)]
    for k, ((x, y), right) in enumerate(ring):
        card(f"forest beyond {k + 1}", (x, y, tall / 2 - 1.0), right, (0, 0, 1), span, tall, trees_mat)
