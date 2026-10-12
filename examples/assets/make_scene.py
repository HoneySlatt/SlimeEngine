"""Builds the sample scene of SlimeEngine's `scene` example in Blender and exports it as a GLB file.

Run it inside Blender (5.2 was used), from the repository root:

    blender --background --python examples/assets/make_scene.py -- examples/assets/scene.glb

The scene is small on purpose: a tiled floor, two wooden crates (the small one a child of the big
one), a sphere and a pillar under a turned empty, a brick wall and a ring, so that the loader
meets textured and plain materials, smooth and flat shading, and nested transforms. Its textures
are drawn here with numpy and embedded in the file as PNG images. Everything the script produces
is released under CC0 1.0 (public domain); see README.md in this directory.
"""

import math
import sys

import bpy
import bmesh
import numpy as np
from mathutils import Vector

SIZE = 128


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


def image(name, pixels):
    """A texture from `pixels`, an array of shape (SIZE, SIZE, 3) of sRGB bytes, rows from top."""
    img = bpy.data.images.new(name, SIZE, SIZE, alpha=False)
    rgba = np.ones((SIZE, SIZE, 4), dtype=np.float32)
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
    """Horizontal wooden planks with a darker frame: the sides of a crate."""
    rng = np.random.default_rng(3)
    pixels = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    plank = 32
    for row in range(SIZE // plank):
        color = np.array([168, 118, 64]) + rng.integers(-14, 14)
        pixels[row * plank:(row + 1) * plank, :] = color
        pixels[row * plank:row * plank + 2, :] = [84, 56, 30]
    grain = rng.integers(-10, 10, size=(SIZE, 1, 1)) * np.ones((1, SIZE, 1), dtype=int)
    pixels = np.clip(pixels.astype(int) + grain, 0, 255).astype(np.uint8)
    frame = np.array([96, 64, 34], dtype=np.uint8)
    for edge in range(6):
        pixels[edge, :] = frame
        pixels[SIZE - 1 - edge, :] = frame
        pixels[:, edge] = frame
        pixels[:, SIZE - 1 - edge] = frame
    return pixels


def bricks():
    """Rows of bricks, each row shifted by half a brick, with mortar between them."""
    rng = np.random.default_rng(11)
    mortar = np.array([200, 194, 184], dtype=np.uint8)
    pixels = np.tile(mortar, (SIZE, SIZE, 1))
    height, width = 16, 32
    for row in range(SIZE // height):
        shift = (width // 2) if row % 2 else 0
        for column in range(-1, SIZE // width + 1):
            x0 = column * width + shift + 2
            x1 = x0 + width - 2
            y0 = row * height + 2
            y1 = y0 + height - 2
            color = np.array([150, 72, 56]) + rng.integers(-16, 16)
            pixels[y0:y1, max(x0, 0):min(x1, SIZE)] = color
    return pixels.astype(np.uint8)


def material(name, color, texture=None):
    """A Principled BSDF of `color`, multiplied by `texture` when there is one."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.8
    if texture is not None:
        node = nodes.new("ShaderNodeTexImage")
        node.image = texture
        mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


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
    tile_texture, plank_texture, brick_texture = image("tiles", tiles()), image("planks", planks()), image("bricks", bricks())
    floor_material = material("tiles", (1.0, 1.0, 1.0), tile_texture)
    crate_material = material("planks", (1.0, 1.0, 1.0), plank_texture)
    wall_material = material("bricks", (1.0, 1.0, 1.0), brick_texture)
    red = material("red", (0.80, 0.15, 0.10))
    grey = material("grey", (0.72, 0.72, 0.70))
    teal = material("teal", (0.10, 0.60, 0.60))

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
    add("pillar", grey, (-1.6, -1.4, 1.0), parent=props, smooth=True)

    bpy.ops.mesh.primitive_cube_add(size=1.0)
    wall = add("wall", wall_material, (0.0, -2.2, 0.5))
    wall.scale = (3.0, 0.3, 1.0)
    box_uvs(wall, repeat_u=3.0, repeat_v=1.0)

    bpy.ops.mesh.primitive_torus_add(major_radius=0.4, minor_radius=0.12, major_segments=48,
                                     minor_segments=12)
    add("ring", teal, (1.8, -0.6, 0.12), smooth=True)

    # A camera and a sun, to look at the scene in Blender; neither is exported.
    bpy.ops.object.camera_add(location=(5.5, -5.5, 3.6),
                              rotation=(math.radians(66), 0.0, math.radians(45)))
    bpy.context.scene.camera = bpy.context.active_object
    bpy.ops.object.light_add(type="SUN", location=(2.0, -2.0, 6.0),
                             rotation=(math.radians(35), math.radians(20), 0.0))


def export(path):
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_apply=True,
        export_yup=True,
        export_normals=True,
        export_texcoords=True,
        export_materials="EXPORT",
        export_image_format="AUTO",
        export_animations=False,
        export_skins=False,
        export_morph=False,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
        use_selection=False,
    )


if __name__ == "__main__":
    build()
    export(output_path())
