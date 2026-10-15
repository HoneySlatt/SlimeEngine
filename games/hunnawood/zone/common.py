"""What the parts of the zone's script share: the source files, materials, meshes built from
lists of vertices, and the nodes that the game reads (colliders, markers and lights).

Blender's axes: x east, y north, z up, in metres. The glTF exporter turns them into glTF's, y up
and z south, which are the game's.
"""

import math
import os
import random
import zipfile

import bmesh
import bpy
from mathutils import Vector

# Light in physical units, as KHR_lights_punctual and the engine read them (ADR-0037). Blender's
# exporter turns watts into these units in its standard lighting mode, "SPEC": a sun's strength,
# in W/m^2, times 683 lm/W gives lux; a point or spot light's power, in W, times 683 / (4 pi)
# gives candelas.
LUMENS_PER_WATT = 683


class Sources:
    """The fetched source assets, under `games/hunnawood/assets/sources/` (sources.tsv lists
    them); `scratch` is a directory for what is unpacked from their archives."""

    def __init__(self, root, scratch):
        self.root = root
        self.scratch = scratch

    def path(self, *parts):
        path = os.path.join(self.root, *parts)
        if not os.path.exists(path):
            raise SystemExit(f"{path} is missing: run scripts/fetch-assets.sh first")
        return path

    def from_zip(self, archive, member):
        """The file `member` of the zip file `archive`, unpacked into the scratch directory."""
        target = os.path.join(self.scratch, member)
        if not os.path.exists(target):
            with zipfile.ZipFile(self.path(archive)) as z:
                z.extract(member, self.scratch)
        return target


def load_image(path, data=False):
    """The image at `path`, once; `data` for images that hold values rather than colors."""
    name = os.path.basename(path)
    image = bpy.data.images.get(name) or bpy.data.images.load(path)
    if data:
        image.colorspace_settings.name = "Non-Color"
    return image


def material(name, color=(0.5, 0.5, 0.5), roughness=0.8, metallic=0.0, double_sided=False):
    """A Principled BSDF of one `color`; single-sided unless `double_sided`, which the exporter
    reads from backface culling."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.use_backface_culling = not double_sided
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    return mat


def image_node(mat, image):
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = image
    return node


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


def textured(name, diffuse, normal=None, arm=None, double_sided=False, alpha=None, tint=None):
    """A material of Poly Haven's layout: a color image, a normal image in OpenGL's convention
    (glTF's), and an image whose red, green and blue hold the occlusion, roughness and metallic
    (glTF's layout, which the exporter writes unchanged as both textures). `alpha`, an image
    whose red holds the coverage, cuts the surface out where it is below one half: the exporter
    writes the `MASK` mode and merges it into the color image. `tint` multiplies the colors, which
    the exporter writes as the base color factor: a photographed texture brought to the zone's
    palette."""
    mat = material(name, double_sided=double_sided)
    tree = mat.node_tree
    bsdf = tree.nodes["Principled BSDF"]
    color = image_node(mat, load_image(diffuse)).outputs["Color"]
    if tint is not None:
        mix = tree.nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        socket = {s.identifier: s for s in mix.inputs}
        socket["B_Color"].default_value = (*tint, 1.0)
        tree.links.new(color, socket["A_Color"])
        color = next(s for s in mix.outputs if s.identifier == "Result_Color")
    tree.links.new(color, bsdf.inputs["Base Color"])
    if normal is not None:
        node = tree.nodes.new("ShaderNodeNormalMap")
        tree.links.new(image_node(mat, load_image(normal, data=True)).outputs["Color"],
                       node.inputs["Color"])
        tree.links.new(node.outputs["Normal"], bsdf.inputs["Normal"])
    if arm is not None:
        separate = tree.nodes.new("ShaderNodeSeparateColor")
        tree.links.new(image_node(mat, load_image(arm, data=True)).outputs["Color"],
                       separate.inputs["Color"])
        tree.links.new(separate.outputs["Green"], bsdf.inputs["Roughness"])
        tree.links.new(separate.outputs["Blue"], bsdf.inputs["Metallic"])
        occlusion = tree.nodes.new("ShaderNodeGroup")
        occlusion.node_tree = gltf_material_output()
        tree.links.new(separate.outputs["Red"], occlusion.inputs["Occlusion"])
    if alpha is not None:
        cut(mat, image_node(mat, load_image(alpha, data=True)).outputs["Color"])
    return mat


def cut(mat, coverage):
    """Cuts `mat` out where `coverage` is below one half: rounded, as the exporter needs to write
    the `MASK` mode."""
    tree = mat.node_tree
    rounded = tree.nodes.new("ShaderNodeMath")
    rounded.operation = "ROUND"
    tree.links.new(coverage, rounded.inputs[0])
    tree.links.new(rounded.outputs[0], tree.nodes["Principled BSDF"].inputs["Alpha"])
    mat.use_backface_culling = False


def poly_haven_set(sources, name, resolution, double_sided=False, tint=None):
    """The material of the Poly Haven texture set `name` at `resolution` ("1k" or "2k"), its
    colors multiplied by `tint`."""
    folder = sources.path(name)
    names = sorted(os.listdir(folder))

    def find(key):
        found = [n for n in names if f"_{key}_{resolution}." in n]
        if len(found) != 1:
            raise SystemExit(f"{folder}: no single {key} image at {resolution}")
        return os.path.join(folder, found[0])

    diffuse = [n for n in names if f"_diff_{resolution}." in n or f"_diffuse_{resolution}." in n]
    return textured(name, os.path.join(folder, diffuse[0]), find("nor_gl"), find("arm"),
                    double_sided=double_sided, tint=tint)


def mesh_object(name, vertices, faces, uvs=None, material=None, smooth=False, collection=None):
    """An object of a new mesh: `faces` index `vertices`; `uvs`, when given, holds one pair per
    corner of each face, in the order of `faces`."""
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([tuple(v) for v in vertices], [], [tuple(f) for f in faces])
    if uvs is not None:
        layer = mesh.uv_layers.new(name="UVMap")
        for loop, uv in zip(layer.data, uvs):
            loop.uv = uv
    if material is not None:
        mesh.materials.append(material)
    mesh.shade_smooth() if smooth else mesh.shade_flat()
    mesh.validate()
    obj = bpy.data.objects.new(name, mesh)
    (collection or bpy.context.scene.collection).objects.link(obj)
    return obj


def join(objects, name):
    """`objects` joined into the first, renamed `name`: one mesh, one primitive per material."""
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    objects[0].name = name
    objects[0].data.name = name
    return objects[0]


def triangulate(obj):
    """Splits faces of more than four corners into triangles, without which the exporter writes
    no tangents."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.triangulate(bm, faces=[f for f in bm.faces if len(f.verts) > 4])
    bm.to_mesh(obj.data)
    bm.free()


def world_box_uvs(obj, scale):
    """Projects each face's texture coordinates from its position, along the axis its normal is
    nearest, one repeat of the texture every `scale` metres: stone and earth that line up across
    the faces of a model."""
    mesh = obj.data
    layer = mesh.uv_layers.get("UVMap") or mesh.uv_layers.new(name="UVMap")
    matrix = obj.matrix_world
    for poly in mesh.polygons:
        n = (matrix.to_3x3() @ poly.normal).normalized()
        axis = max(range(3), key=lambda i: abs(n[i]))
        for index in poly.loop_indices:
            p = matrix @ mesh.vertices[mesh.loops[index].vertex_index].co
            if axis == 2:
                u, v = p.x, p.y
            elif axis == 1:
                u, v = p.x * (1 if n.y < 0 else -1), p.z
            else:
                u, v = p.y * (1 if n.x > 0 else -1), p.z
            layer.data[index].uv = (u / scale, v / scale)


def collider_box(name, center, size, height, yaw=0.0, parent=None):
    """A box that stops the hero, standing on the ground: `center` (x, y), `size` (along its own
    x and y), turned by `yaw` radians about the vertical, `height` metres high. An empty whose
    custom properties the exporter writes as the node's `extras`; its scale holds half the size."""
    bpy.ops.object.empty_add(type="CUBE", location=(center[0], center[1], height / 2))
    obj = bpy.context.active_object
    obj.name = name
    obj.rotation_euler = (0.0, 0.0, yaw)
    obj.scale = (size[0] / 2, size[1] / 2, height / 2)
    obj["collider"] = "box"
    obj["height"] = round(height, 3)
    if parent is not None:
        reparent(obj, parent)
    return obj


def collider_cylinder(name, center, radius, height, parent=None):
    """A cylinder that stops the hero: an empty whose x scale is the radius."""
    bpy.ops.object.empty_add(type="CIRCLE", location=(center[0], center[1], 0.0))
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = (radius, radius, radius)
    obj["collider"] = "cylinder"
    obj["height"] = round(height, 3)
    if parent is not None:
        reparent(obj, parent)
    return obj


def marker(name, kind, location, facing=0.0):
    """A point the game reads by its `marker` property: where the hero starts, the item, the door,
    the campfire. It faces along its local +y, north turned by `facing` radians towards the west,
    which glTF writes as the node's -z."""
    bpy.ops.object.empty_add(type="ARROWS", location=location)
    obj = bpy.context.active_object
    obj.name = name
    obj.rotation_euler = (0.0, 0.0, facing)
    obj["marker"] = kind
    return obj


def reparent(child, parent):
    """Makes `child` a child of `parent`, keeping where it stands."""
    # The world matrices follow a change of location, rotation or scale only once updated.
    bpy.context.view_layer.update()
    world = child.matrix_world.copy()
    child.parent = parent
    child.matrix_world = world


def light(name, kind, time, location, direction, color, value, cone=None):
    """A light of `time` ("day" or "night"), which the game reads from the node's `time`
    property: `kind` "SUN" of `value` lux, or "SPOT" or "POINT" of `value` candelas, shining
    along `direction`; `cone` is a spot light's outer and inner angles from its axis, in
    radians. The exporter writes no range: the light fades with the square of the distance, without
    end."""
    data = bpy.data.lights.new(name, kind)
    data.color = color
    if kind == "SUN":
        data.energy = value / LUMENS_PER_WATT
    else:
        data.energy = value * 4 * math.pi / LUMENS_PER_WATT
    if cone is not None:
        outer, inner = cone
        data.spot_size = 2 * outer
        data.spot_blend = 1 - inner / outer
    obj = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = location
    obj.rotation_euler = Vector(direction).to_track_quat("-Z", "Y").to_euler()
    obj["time"] = time
    return obj


def rng(seed):
    return random.Random(seed)



def apply_transform(obj):
    """Bakes `obj`'s location, rotation and scale into its mesh. The operator works on every
    selected object, and new empties are left selected: everything else is deselected first, or
    the colliders made before would lose their places."""
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.select_set(False)
