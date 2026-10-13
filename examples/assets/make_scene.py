"""Builds the sample scene of SlimeEngine's `scene` example in Blender and exports it as a GLB file.

Run it inside Blender (5.2 was used), from the repository root:

    blender --background --python examples/assets/make_scene.py -- examples/assets/scene.glb

The scene is small on purpose: a tiled floor, two wooden crates (the small one a child of the big
one), a sphere and a pillar under a turned empty, a brick wall and a ring, lit by a sun, a point
light in the ring and a spot light, so that the loader meets textured and plain materials, smooth
and flat shading, nested transforms, glTF's physically based properties (metallic-roughness,
normal, occlusion and emissive textures, an emissive strength above 1, metals and dielectrics of
several roughnesses, single- and double-sided materials, tangents) and lights in physical units
(KHR_lights_punctual). Its textures are drawn here with numpy and embedded in the file as PNG
images. Everything the script produces is released under CC0 1.0 (public domain); see README.md in
this directory.
"""

import math
import sys

import bpy
import bmesh
import numpy as np
from mathutils import Vector

SIZE = 128
BRICK_HEIGHT, BRICK_WIDTH = 16, 32
# The ring's major and minor radii, in metres.
RING_RADII = (0.4, 0.12)

# Light in physical units, as KHR_lights_punctual and the engine read it (ADR-0037): lux for the
# sun, candelas for the point and spot lights, nits (candelas per square metre) for the ring's
# emission. The engine's sample draws it at an exposure of pi / SUN_LUX
# (examples/common/sample_scene.rs).
SUN_LUX = 100_000  # direct sunlight
# Four times what a white matte surface facing the sun reflects: 4 x 100 000 / pi nits.
RING_NITS = 4 * SUN_LUX / math.pi
# A point light at the ring's center stands for its glow. The glowing segments, 10 of every 16
# columns of their texture (ring_glow), cover 0.625 of the torus, whose area is 4 pi^2 R r. A
# surface that glows evenly at L nits over A square metres emits pi L A lumens, as much as a point
# light of L A / 4 candelas: about 37 700 candelas, in the color of the glowing texels, (120, 255,
# 235) in sRGB, (0.1878, 1, 0.8308) in linear light.
RING_LIGHT_CANDELAS = RING_NITS * 0.625 * 4 * math.pi ** 2 * RING_RADII[0] * RING_RADII[1] / 4
RING_LIGHT_COLOR = (0.1878, 1.0, 0.8308)
SPOT_CANDELAS = 80_000  # a stage spotlight, bright enough to show beside the sun
# Blender's exporter turns watts into these units in its standard lighting mode, "SPEC"
# (io_scene_gltf2/blender/exp/lights.py): a sun's strength, in W/m^2, times 683 lm/W gives lux; a
# point or spot light's power, in W, times 683 / (4 pi) gives candelas. Emission strengths are
# written as they are.
LUMENS_PER_WATT = 683


def output_path():
    """The GLB file to write: the argument after `--`, or `OUTPUT` when run from another script."""
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1]
    return globals()["OUTPUT"]


def clear():
    """Removes the default objects and data of a new file."""
    for collection in (bpy.data.objects, bpy.data.meshes, bpy.data.materials, bpy.data.lights,
                       bpy.data.cameras):
        for block in list(collection):
            collection.remove(block)
    for image in list(bpy.data.images):
        if image.users == 0 and image.name not in ("Render Result", "Viewer Node"):
            bpy.data.images.remove(image)


def image(name, pixels, data=False):
    """A texture from `pixels`, an array of shape (height, width, 3) of bytes, rows from the top:
    sRGB-encoded colors, or, when `data` is set, values that shaders read as they are."""
    height, width = pixels.shape[:2]
    img = bpy.data.images.new(name, width, height, alpha=False)
    if data:
        img.colorspace_settings.name = "Non-Color"
    rgba = np.ones((height, width, 4), dtype=np.float32)
    rgba[:, :, :3] = pixels[::-1].astype(np.float32) / 255.0  # Blender stores rows from bottom.
    img.pixels.foreach_set(rgba.ravel())
    img.pack()
    return img


def tiles():
    """Square tiles with grout between them, each tile a little lighter or darker."""
    rng = np.random.default_rng(7)
    pixels = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    tile = 32
    for row in range(SIZE // tile):
        for column in range(SIZE // tile):
            shade = rng.integers(-18, 18)
            color = np.array([190, 182, 168]) + shade
            pixels[row * tile:(row + 1) * tile, column * tile:(column + 1) * tile] = color
    grout = np.array([70, 64, 58])
    for line in range(0, SIZE, tile):
        pixels[line:line + 2, :] = grout
        pixels[:, line:line + 2] = grout
    noise = rng.integers(-6, 6, size=(SIZE, SIZE, 1))
    return np.clip(pixels.astype(int) + noise, 0, 255).astype(np.uint8)


def planks():
    """Horizontal wooden planks in an iron frame: the sides of a crate."""
    rng = np.random.default_rng(3)
    pixels = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    plank = 32
    for row in range(SIZE // plank):
        color = np.array([168, 118, 64]) + rng.integers(-14, 14)
        pixels[row * plank:(row + 1) * plank, :] = color
        pixels[row * plank:row * plank + 2, :] = [84, 56, 30]
    grain = rng.integers(-10, 10, size=(SIZE, 1, 1)) * np.ones((1, SIZE, 1), dtype=int)
    pixels = np.clip(pixels.astype(int) + grain, 0, 255).astype(np.uint8)
    frame = np.array([150, 150, 155], dtype=np.uint8)
    for edge in range(6):
        pixels[edge, :] = frame
        pixels[SIZE - 1 - edge, :] = frame
        pixels[:, edge] = frame
        pixels[:, SIZE - 1 - edge] = frame
    return pixels


def planks_orm():
    """Occlusion, roughness and metallic of the planks in red, green and blue (glTF's layout): the
    iron frame smooth and metallic, the wood rough, the gaps between planks occluded."""
    orm = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    orm[:, :, 0] = 255
    orm[:, :, 1] = round(0.7 * 255)
    for row in range(SIZE // 32):
        orm[row * 32:row * 32 + 2, :, 0] = round(0.5 * 255)
    frame = np.zeros((SIZE, SIZE), dtype=bool)
    frame[:6, :] = frame[-6:, :] = frame[:, :6] = frame[:, -6:] = True
    orm[frame] = (255, round(0.4 * 255), 255)
    return orm


def tiles_occlusion():
    """How much ambient light reaches the floor: all of it on the tiles, 0.6 of it in the grout."""
    occlusion = np.full((SIZE, SIZE, 3), 255, dtype=np.uint8)
    for line in range(0, SIZE, 32):
        occlusion[line:line + 2, :] = round(0.6 * 255)
        occlusion[:, line:line + 2] = round(0.6 * 255)
    return occlusion


def brick_rectangles():
    """The bricks, (y0, y1, x0, x1) in pixels, clipped to the image: rows shifted by half a brick,
    2 pixels of mortar between them. Empty ones are kept, so that `bricks` draws its random
    shades in the same order."""
    for row in range(SIZE // BRICK_HEIGHT):
        shift = (BRICK_WIDTH // 2) if row % 2 else 0
        for column in range(-1, SIZE // BRICK_WIDTH + 1):
            x0 = column * BRICK_WIDTH + shift + 2
            y0 = row * BRICK_HEIGHT + 2
            yield y0, y0 + BRICK_HEIGHT - 2, max(x0, 0), min(x0 + BRICK_WIDTH - 2, SIZE)


def bricks():
    """Rows of bricks, each row shifted by half a brick, with mortar between them."""
    rng = np.random.default_rng(11)
    pixels = np.tile(np.array([200, 194, 184], dtype=np.uint8), (SIZE, SIZE, 1))
    for y0, y1, x0, x1 in brick_rectangles():
        color = np.array([150, 72, 56]) + rng.integers(-16, 16)
        pixels[y0:y1, x0:x1] = color
    return pixels.astype(np.uint8)


def normal_map(height, strength):
    """The tangent-space normal map of `height` (rows from the top, wrapping around, as the texture
    repeats), in glTF's convention: red for +X (to the right), green for +Y (up the image), blue for
    +Z (out of the surface), each from -1 to 1 stored as 0 to 255."""
    right = (np.roll(height, -1, axis=1) - np.roll(height, 1, axis=1)) / 2
    up = (np.roll(height, 1, axis=0) - np.roll(height, -1, axis=0)) / 2
    normal = np.dstack((-strength * right, -strength * up, np.ones_like(height)))
    normal /= np.linalg.norm(normal, axis=2, keepdims=True)
    return np.round((normal + 1) * 127.5).astype(np.uint8)


def bricks_normal():
    """The bricks raised above their mortar, with rounded edges."""
    height = np.zeros((SIZE, SIZE))
    for y0, y1, x0, x1 in brick_rectangles():
        height[y0:y1, x0:x1] = 1.0
    for _ in range(2):  # a 3 x 3 box blur, twice
        height = sum(np.roll(np.roll(height, dy, 0), dx, 1) for dy in (-1, 0, 1) for dx in (-1, 0, 1)) / 9
    return normal_map(height, strength=2.0)


def ring_glow():
    """Eight glowing teal segments on black, around the ring."""
    pixels = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    for start in range(0, SIZE, 16):
        pixels[:, start:start + 10] = (120, 255, 235)
    return pixels


def material(name, color, texture=None, roughness=0.8, metallic=0.0, double_sided=False):
    """A Principled BSDF of `color`, multiplied by `texture` when there is one; closed objects'
    materials cull their back faces, which glTF exports as single-sided."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.use_backface_culling = not double_sided
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if texture is not None:
        mat.node_tree.links.new(image_node(mat, texture).outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def image_node(mat, img):
    """A node of `mat`'s node tree that reads `img`."""
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = img
    return node


def channels(mat, img):
    """The red, green and blue outputs of `img` in `mat`'s node tree."""
    separate = mat.node_tree.nodes.new("ShaderNodeSeparateColor")
    mat.node_tree.links.new(image_node(mat, img).outputs["Color"], separate.inputs["Color"])
    return separate.outputs


def gltf_material_output():
    """The node group whose "Occlusion" input Blender's glTF exporter reads as the occlusion
    texture, built as the exporter's own helper builds it."""
    group = bpy.data.node_groups.get("glTF Material Output")
    if group is None:
        group = bpy.data.node_groups.new("glTF Material Output", "ShaderNodeTree")
        group.interface.new_socket("Occlusion", in_out="INPUT", socket_type="NodeSocketFloat")
        group.nodes.new("NodeGroupInput")
        group.nodes.new("NodeGroupOutput")
    return group


def add_occlusion(mat, red):
    """Makes `red`, the red output of an image's `channels`, the occlusion texture of `mat`: glTF
    reads occlusion from the red channel, and Blender's shaders have no occlusion input."""
    node = mat.node_tree.nodes.new("ShaderNodeGroup")
    node.node_tree = gltf_material_output()
    mat.node_tree.links.new(red, node.inputs["Occlusion"])


def add_metal_roughness(mat, outputs):
    """Makes the green and blue `outputs` of an image's `channels` the roughness and metallic of
    `mat`: the exporter then writes that image, unchanged, as the metallic-roughness texture, and
    as the occlusion texture too when its red output is `add_occlusion`'s."""
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    mat.node_tree.links.new(outputs["Green"], bsdf.inputs["Roughness"])
    mat.node_tree.links.new(outputs["Blue"], bsdf.inputs["Metallic"])


def add_normal_map(mat, img):
    """Gives `mat` the tangent-space normal texture `img`, at glTF's default scale of 1."""
    normal = mat.node_tree.nodes.new("ShaderNodeNormalMap")  # tangent space, strength 1
    mat.node_tree.links.new(image_node(mat, img).outputs["Color"], normal.inputs["Color"])
    mat.node_tree.links.new(normal.outputs["Normal"], mat.node_tree.nodes["Principled BSDF"].inputs["Normal"])


def add_light(name, kind, energy, location, target=None, color=(1.0, 1.0, 1.0)):
    """Adds a light of `kind` ("SUN", "POINT" or "SPOT") of `energy` (Blender's strength or power)
    at `location`, pointing at `target` when given: Blender's lights, like glTF's, shine down their
    local -Z axis, which the exporter keeps when it turns Z up into glTF's Y up."""
    bpy.ops.object.light_add(type=kind, location=location)
    obj = bpy.context.active_object
    obj.name = name
    obj.data.name = name
    obj.data.energy = energy
    obj.data.color = color
    if target is not None:
        obj.rotation_euler = (Vector(target) - Vector(location)).to_track_quat("-Z", "Y").to_euler()
    return obj


def add_emission(mat, img, strength):
    """Makes `img` times `strength` the light `mat` emits: the exporter writes an emissive factor
    of 1 and, for a strength above 1, the strength as `KHR_materials_emissive_strength`."""
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    mat.node_tree.links.new(image_node(mat, img).outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = strength


def triangulate_ngons(obj):
    """Splits faces of more than four corners into triangles, which Blender's tangents need: the
    exporter writes no tangents for a mesh with such faces. It would split them into as many
    triangles itself, so the triangle count does not change."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.triangulate(bm, faces=[face for face in bm.faces if len(face.verts) > 4])
    bm.to_mesh(obj.data)
    bm.free()


def box_uvs(obj, repeat_u=1.0, repeat_v=1.0):
    """Gives each face of a box the whole texture, repeated `repeat_u` × `repeat_v` times."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    layer = bm.loops.layers.uv.verify()
    corners = [(0.0, 0.0), (repeat_u, 0.0), (repeat_u, repeat_v), (0.0, repeat_v)]
    for face in bm.faces:
        for loop, uv in zip(face.loops, corners):
            loop[layer].uv = uv
    bm.to_mesh(obj.data)
    bm.free()


def add(name, material, location, rotation_z=0.0, parent=None, smooth=False):
    """Names, places and colors the object just added by a primitive operator."""
    obj = bpy.context.active_object
    obj.name = name
    obj.data.name = name
    obj.data.materials.append(material)
    obj.location = location
    obj.rotation_euler = (0.0, 0.0, math.radians(rotation_z))
    if parent is not None:
        obj.parent = parent
    if smooth:
        bpy.ops.object.shade_auto_smooth()
    return obj


def build():
    clear()
    floor_material = material("tiles", (1.0, 1.0, 1.0), image("tiles", tiles()), double_sided=True)
    add_occlusion(floor_material, channels(floor_material, image("tiles occlusion", tiles_occlusion(), data=True))["Red"])
    crate_material = material("planks", (1.0, 1.0, 1.0), image("planks", planks()))
    crate_orm = channels(crate_material, image("planks ORM", planks_orm(), data=True))
    add_occlusion(crate_material, crate_orm["Red"])
    add_metal_roughness(crate_material, crate_orm)
    wall_material = material("bricks", (1.0, 1.0, 1.0), image("bricks", bricks()))
    add_normal_map(wall_material, image("bricks normal", bricks_normal(), data=True))
    red = material("red", (0.80, 0.15, 0.10), roughness=0.35)
    grey = material("grey", (0.72, 0.72, 0.70), roughness=0.4, metallic=1.0)
    teal = material("teal", (0.10, 0.60, 0.60))
    add_emission(teal, image("ring glow", ring_glow()), strength=RING_NITS)

    bpy.ops.mesh.primitive_plane_add(size=6.0)
    floor = add("floor", floor_material, (0.0, 0.0, 0.0))
    box_uvs(floor, repeat_u=3.0, repeat_v=3.0)

    bpy.ops.mesh.primitive_cube_add(size=1.0)
    crate = add("crate", crate_material, (1.0, 0.6, 0.5), rotation_z=20.0)
    box_uvs(crate)
    bpy.ops.mesh.primitive_cube_add(size=0.6)
    small_crate = add("small crate", crate_material, (0.0, 0.0, 0.8), rotation_z=35.0, parent=crate)
    box_uvs(small_crate)

    bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0.3, 0.0, 0.0))
    props = bpy.context.active_object
    props.name = "props"
    props.rotation_euler = (0.0, 0.0, math.radians(10.0))
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.45, segments=32, ring_count=16)
    add("sphere", red, (-1.2, 0.8, 0.45), parent=props, smooth=True)
    bpy.ops.mesh.primitive_cylinder_add(radius=0.25, depth=2.0, vertices=32)
    pillar = add("pillar", grey, (-1.6, -1.4, 1.0), parent=props, smooth=True)
    triangulate_ngons(pillar)

    bpy.ops.mesh.primitive_cube_add(size=1.0)
    wall = add("wall", wall_material, (0.0, -2.2, 0.5))
    wall.scale = (3.0, 0.3, 1.0)
    box_uvs(wall, repeat_u=3.0, repeat_v=1.0)

    bpy.ops.mesh.primitive_torus_add(major_radius=RING_RADII[0], minor_radius=RING_RADII[1],
                                     major_segments=48, minor_segments=12)
    ring = add("ring", teal, (1.8, -0.6, 0.12), smooth=True)

    # A camera, to look at the scene in Blender; it is not exported.
    bpy.ops.object.camera_add(location=(5.5, -5.5, 3.6),
                              rotation=(math.radians(66), 0.0, math.radians(45)))
    bpy.context.scene.camera = bpy.context.active_object

    # A slightly warm sun from above, travelling along (-0.5, 0.6, -1.0): (-0.5, -1.0, -0.6) in
    # glTF's axes. Where a sun is placed does not matter.
    add_light("sun", "SUN", SUN_LUX / LUMENS_PER_WATT, (2.0, -2.0, 6.0), (1.5, -1.4, 5.0),
              color=(1.0, 0.98, 0.9))
    # The ring's glow (see RING_LIGHT_CANDELAS), at its center and moving with it, up to 4 m, where
    # it would give a surface facing it about 2 400 lux.
    glow = add_light("ring light", "POINT", RING_LIGHT_CANDELAS * 4 * math.pi / LUMENS_PER_WATT,
                     (0.0, 0.0, 0.0), color=RING_LIGHT_COLOR)
    glow.parent = ring
    glow.data.use_custom_distance = True
    glow.data.cutoff_distance = 4.0
    # A warm spot light on the floor, in front of the wall, aimed up at the side that the sun leaves
    # in the shade. Only the sun casts shadows: aimed up, the light that the wall would stop goes on
    # into the sky instead of onto the floor behind the wall.
    spot = add_light("spot", "SPOT", SPOT_CANDELAS * 4 * math.pi / LUMENS_PER_WATT,
                     (0.6, -0.9, 0.05), (0.6, -2.05, 0.5), color=(1.0, 0.8, 0.55))
    # Full within 18 degrees of its axis, nothing beyond 30: the exporter writes the outer angle as
    # spot_size / 2 and the inner one as spot_size / 2 x (1 - spot_blend).
    spot.data.spot_size = math.radians(60.0)
    spot.data.spot_blend = 0.4


def export(path):
    # Under any other render engine, the exporter reads a light's strength from its node tree, where
    # a sun's is 1 whatever its `energy`: it would write the sun as 683 lux.
    bpy.context.scene.render.engine = "BLENDER_EEVEE"
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
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
        # Physical units: lux for the sun, candelas for the point and spot lights (LUMENS_PER_WATT).
        export_import_convert_lighting_mode="SPEC",
        export_extras=False,
        use_selection=False,
    )


if __name__ == "__main__":
    build()
    export(output_path())
