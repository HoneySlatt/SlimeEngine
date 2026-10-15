"""The forest: oaks and conifers made here, their foliage as leaf cards, quads whose texture's
alpha cuts the leaves out; and the scatter of trees, rocks, ferns, logs and rock faces around the
path, kept off the path, the gully, the chapel and the clearing."""

import math
import os

import bpy
from mathutils import Matrix, Vector

import common

TWIG_CELLS = 2  # the twig atlas holds 2 x 2 twigs
OAK_BARK = (1.0, 2.0)  # the size of one repeat of the oak's bark, around and along, in metres
PINE_BARK = (2.0, 2.0)


# Twigs: a picture of leaves on a stem, rendered once from the leaf atlas, which the oaks' cards
# show.

def twig_atlas(sources, path, size=2048):
    """Renders `TWIG_CELLS` x `TWIG_CELLS` twigs of LeafSet024's leaves into `path`, a PNG image
    whose alpha is the leaves' coverage, seen from above, without light: the colors of the
    leaves as photographed. A few leaves of each twig are yellowing, as in early autumn."""
    if os.path.exists(path):
        return path
    color = sources.from_zip("LeafSet024/LeafSet024_2K-JPG.zip", "LeafSet024_2K-JPG_Color.jpg")
    opacity = sources.from_zip("LeafSet024/LeafSet024_2K-JPG.zip", "LeafSet024_2K-JPG_Opacity.jpg")
    main = bpy.context.scene
    scene = bpy.data.scenes.new("twigs")
    if bpy.context.window:
        bpy.context.window.scene = scene
    random = common.rng(5)
    leaf = bpy.data.materials.new("twig leaf")
    leaf.use_nodes = True
    tree = leaf.node_tree
    tree.nodes.clear()
    out = tree.nodes.new("ShaderNodeOutputMaterial")
    tex = tree.nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(color)
    alpha = tree.nodes.new("ShaderNodeTexImage")
    alpha.image = bpy.data.images.load(opacity)
    alpha.image.colorspace_settings.name = "Non-Color"
    # A vertex color tints the yellowing leaves.
    tint = tree.nodes.new("ShaderNodeVertexColor")
    tint.layer_name = "Col"
    mix = tree.nodes.new("ShaderNodeVectorMath")
    mix.operation = "MULTIPLY"
    emission = tree.nodes.new("ShaderNodeEmission")
    transparent = tree.nodes.new("ShaderNodeBsdfTransparent")
    shader = tree.nodes.new("ShaderNodeMixShader")
    tree.links.new(tex.outputs["Color"], mix.inputs[0])
    tree.links.new(tint.outputs["Color"], mix.inputs[1])
    tree.links.new(mix.outputs["Vector"], emission.inputs["Color"])
    tree.links.new(alpha.outputs["Color"], shader.inputs["Fac"])
    tree.links.new(transparent.outputs["BSDF"], shader.inputs[1])
    tree.links.new(emission.outputs["Emission"], shader.inputs[2])
    tree.links.new(shader.outputs["Shader"], out.inputs["Surface"])
    stem = bpy.data.materials.new("twig stem")
    stem.use_nodes = True
    stem.node_tree.nodes.clear()
    stem_out = stem.node_tree.nodes.new("ShaderNodeOutputMaterial")
    stem_emit = stem.node_tree.nodes.new("ShaderNodeEmission")
    stem_emit.inputs["Color"].default_value = (0.09, 0.06, 0.04, 1.0)
    stem.node_tree.links.new(stem_emit.outputs["Emission"], stem_out.inputs["Surface"])

    vertices, faces, uvs, colors, mats = [], [], [], [], []

    def quad(corners, uv, rgb, mat):
        base = len(vertices)
        vertices.extend(corners)
        faces.append((base, base + 1, base + 2, base + 3))
        uvs.extend(uv)
        colors.extend([rgb] * 4)
        mats.append(mat)

    for cell in range(TWIG_CELLS * TWIG_CELLS):
        cx = (cell % TWIG_CELLS) + 0.5
        cy = (cell // TWIG_CELLS) + 0.5
        # The stem runs diagonally across the cell, from its lower left corner.
        start = Vector((cx - 0.42, cy - 0.42, 0.0))
        end = Vector((cx + 0.3, cy + 0.36, 0.0))
        along = (end - start).normalized()
        side = Vector((-along.y, along.x, 0.0))
        quad([start - side * 0.006, start + side * 0.006, end + side * 0.004, end - side * 0.004],
             [(0, 0)] * 4, (1, 1, 1, 1), 1)
        count = 17
        for k in range(count):
            t = 0.12 + 0.86 * k / (count - 1)
            at = start.lerp(end, t)
            sign = 1 if k % 2 == 0 else -1
            angle = math.radians(random.uniform(35, 65)) * sign
            direction = Matrix.Rotation(angle, 3, "Z") @ along
            length = random.uniform(0.17, 0.24) * (1.15 - 0.35 * t)
            tip = at + direction * length
            across = Vector((-direction.y, direction.x, 0.0)) * length * 0.5
            # A leaf of the atlas's 3 x 3: its stalk at the bottom of its cell.
            col, row = random.randrange(3), random.randrange(3)
            u0, v0 = col / 3 + 0.02, 1 - (row + 1) / 3 + 0.01
            u1, v1 = (col + 1) / 3 - 0.02, 1 - row / 3 - 0.01
            yellow = random.random() < 0.2
            rgb = (1.35, 1.15, 0.35, 1.0) if yellow else (random.uniform(0.85, 1.0),) * 3 + (1.0,)
            quad([at - across, at + across, tip + across, tip - across],
                 [(u1, v0), (u0, v0), (u0, v1), (u1, v1)], rgb, 0)
            # Leaves overlap in order along the stem, nearer the tip above.
            for v in vertices[-4:]:
                v.z = 0.001 * k
    mesh = bpy.data.meshes.new("twigs")
    mesh.from_pydata([tuple(v) for v in vertices], [], faces)
    layer = mesh.uv_layers.new(name="UVMap")
    for loop, uv in zip(layer.data, uvs):
        loop.uv = uv
    color = mesh.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
    for i, c in enumerate(colors):
        color.data[i].color = c
    mesh.materials.append(leaf)
    mesh.materials.append(stem)
    for poly, m in zip(mesh.polygons, mats):
        poly.material_index = m
    obj = bpy.data.objects.new("twigs", mesh)
    scene.collection.objects.link(obj)
    camera = bpy.data.cameras.new("twigs")
    camera.type = "ORTHO"
    camera.ortho_scale = TWIG_CELLS
    cam = bpy.data.objects.new("twigs camera", camera)
    cam.location = (TWIG_CELLS / 2, TWIG_CELLS / 2, 5.0)
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 16
    scene.cycles.use_denoising = False
    scene.render.film_transparent = True
    scene.render.resolution_x = size
    scene.render.resolution_y = size
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True, scene=scene.name)
    bpy.data.scenes.remove(scene)
    if bpy.context.window:
        bpy.context.window.scene = main
    return path


def cards_material(name, image_path, alpha_path=None):
    """A double-sided material of a color image cut out by its own alpha, or by `alpha_path`'s
    red, rough and not metallic."""
    mat = common.material(name, roughness=0.75, double_sided=True)
    tree = mat.node_tree
    bsdf = tree.nodes["Principled BSDF"]
    node = common.image_node(mat, common.load_image(image_path))
    tree.links.new(node.outputs["Color"], bsdf.inputs["Base Color"])
    if alpha_path is None:
        common.cut(mat, node.outputs["Alpha"])
    else:
        coverage = common.image_node(mat, common.load_image(alpha_path, data=True))
        common.cut(mat, coverage.outputs["Color"])
    return mat


# Trees.

class Builder:
    """Gathers the vertices, faces and texture coordinates of one mesh."""

    def __init__(self):
        self.vertices, self.faces, self.uvs = [], [], []

    def add(self, corners, uv):
        base = len(self.vertices)
        self.vertices.extend(corners)
        self.faces.append(tuple(range(base, base + len(corners))))
        self.uvs.extend(uv)

    def object(self, name, mat, smooth):
        return common.mesh_object(name, self.vertices, self.faces, self.uvs, mat, smooth=smooth)


def tube(builder, points, radii, sides, repeat):
    """A tube along `points` of `radii`, `sides` around, its bark repeating every `repeat`
    (around, along) metres; each ring turned with the path, without twisting."""
    frames = []
    normal = None
    for i, p in enumerate(points):
        ahead = (points[min(i + 1, len(points) - 1)] - points[max(i - 1, 0)]).normalized()
        if normal is None:
            normal = ahead.orthogonal().normalized()
        normal = (normal - ahead * normal.dot(ahead)).normalized()
        frames.append((p, ahead, normal, ahead.cross(normal)))
    along = 0.0
    rings = []
    for i, (p, _, n, b) in enumerate(frames):
        if i > 0:
            along += (p - frames[i - 1][0]).length
        ring = []
        for k in range(sides + 1):
            a = 2 * math.pi * k / sides
            ring.append((p + (n * math.cos(a) + b * math.sin(a)) * radii[i],
                         (k / sides * 2 * math.pi * radii[0] / repeat[0], along / repeat[1])))
        rings.append(ring)
    for i in range(len(rings) - 1):
        for k in range(sides):
            a, b = rings[i][k], rings[i][k + 1]
            c, d = rings[i + 1][k + 1], rings[i + 1][k]
            builder.add([a[0], b[0], c[0], d[0]], [a[1], b[1], c[1], d[1]])


def bent(start, direction, length, count, droop, random):
    """`count` + 1 points from `start` along `direction`, bending down by `droop` per step, a
    little crooked."""
    points = [start]
    d = direction.normalized()
    step = length / count
    for _ in range(count):
        d = (d + Vector((random.uniform(-0.15, 0.15), random.uniform(-0.15, 0.15), -droop))).normalized()
        points.append(points[-1] + d * step)
    return points


def card(builder, center, size, random, cell, up_bias=0.5):
    """A square leaf card of `size` at `center`, facing a random way leaning up, showing the
    twig of the atlas's `cell`."""
    normal = Vector((random.uniform(-1, 1), random.uniform(-1, 1), random.uniform(-1, 1) + up_bias)).normalized()
    u = normal.orthogonal().normalized()
    u = Matrix.Rotation(random.uniform(0, 2 * math.pi), 3, normal) @ u
    v = normal.cross(u)
    h = size / 2
    col, row = cell % TWIG_CELLS, cell // TWIG_CELLS
    u0, v0 = col / TWIG_CELLS, row / TWIG_CELLS
    u1, v1 = u0 + 1 / TWIG_CELLS, v0 + 1 / TWIG_CELLS
    builder.add([center - u * h - v * h, center + u * h - v * h, center + u * h + v * h, center - u * h + v * h],
                [(u0, v0), (u1, v0), (u1, v1), (u0, v1)])


def oak(name, seed, bark, leaves):
    """An old oak: a trunk with a flared foot, four to six main branches with their own, and a
    crown of leaf cards around the branches' outer parts. Returns its wood, its crown and its
    trunk's radius at the ground."""
    random = common.rng(seed)
    wood, crown = Builder(), Builder()
    height = random.uniform(5.0, 6.5)
    r = random.uniform(0.38, 0.55)
    trunk = bent(Vector((0, 0, -0.2)), Vector((random.uniform(-0.1, 0.1), random.uniform(-0.1, 0.1), 1)),
                 height + 0.2, 8, 0.0, random)
    radii = [r * (1.6 if i == 0 else 1.15 if i == 1 else 1.0 - 0.35 * i / 8) for i in range(9)]
    tube(wood, trunk, radii, 10, OAK_BARK)
    clusters = []
    count = random.randint(4, 6)
    for k in range(count):
        at = trunk[random.randint(5, 8)]
        azimuth = 2 * math.pi * (k + random.uniform(-0.25, 0.25)) / count
        rise = math.radians(random.uniform(25, 55))
        direction = Vector((math.cos(azimuth) * math.cos(rise), math.sin(azimuth) * math.cos(rise), math.sin(rise)))
        length = random.uniform(3.2, 4.8)
        branch = bent(at, direction, length, 5, 0.05, random)
        base = r * 0.55
        tube(wood, branch, [base * (1 - 0.8 * i / 5) for i in range(6)], 6, OAK_BARK)
        clusters += branch[3:]
        for j in range(2):
            at2 = branch[random.randint(2, 4)]
            side = direction.cross(Vector((0, 0, 1))).normalized() * (1 if j == 0 else -1)
            sub = bent(at2, (direction + side * 0.9 + Vector((0, 0, 0.3))), random.uniform(1.6, 2.6), 3, 0.08, random)
            tube(wood, sub, [base * 0.35 * (1 - 0.7 * i / 3) for i in range(4)], 5, OAK_BARK)
            clusters.append(sub[-1])
            clusters.append(sub[-2])
    # A top of leaves above the trunk.
    clusters.append(trunk[-1] + Vector((0, 0, 0.8)))
    for c in clusters:
        for _ in range(random.randint(9, 13)):
            offset = Vector((random.gauss(0, 0.7), random.gauss(0, 0.7), random.gauss(0.2, 0.5)))
            card(crown, c + offset, random.uniform(1.2, 1.7), random, random.randrange(TWIG_CELLS ** 2))
    return (wood.object(name + " wood", bark, smooth=True),
            crown.object(name + " leaves", leaves, smooth=False), r * 1.6)


def conifer(name, seed, bark, needles):
    """A spruce: a straight trunk and whorls of drooping cards of needles, shorter towards the
    top. Returns its wood, its needles and its trunk's radius at the ground."""
    random = common.rng(seed)
    wood, crown = Builder(), Builder()
    height = random.uniform(13.0, 17.0)
    r = random.uniform(0.28, 0.38)
    trunk = [Vector((0, 0, -0.2 + i * (height + 0.2) / 10)) for i in range(11)]
    tube(wood, trunk, [r * (1.3 if i == 0 else 1.0 - 0.9 * i / 10) for i in range(11)], 8, PINE_BARK)
    # The needle atlas holds three sprigs, their stems at the right: (u0, v0, u1, v1).
    sprigs = [(0.28, 0.73, 0.72, 0.95), (0.07, 0.40, 0.90, 0.70), (0.03, 0.02, 0.93, 0.36)]
    z = 2.2
    while z < height - 0.4:
        t = (z - 2.0) / (height - 2.0)
        length = 3.4 * (1 - t) + 0.5
        whorl = random.randint(5, 6)
        turn = random.uniform(0, 2 * math.pi)
        for k in range(whorl):
            a = turn + 2 * math.pi * k / whorl
            out = Vector((math.cos(a), math.sin(a), 0.0))
            droop = math.radians(random.uniform(10, 28))
            tip = Vector((0, 0, z)) + (out * math.cos(droop) - Vector((0, 0, math.sin(droop)))) * length
            across = Vector((-out.y, out.x, 0.0)) * (0.32 + 0.22 * length ** 0.5)
            u0, v0, u1, v1 = sprigs[random.randrange(3)]
            root = Vector((0, 0, z))
            crown.add([root - across * 0.4, tip - across, tip + across, root + across * 0.4],
                      [(u1, v0), (u0, v0), (u0, v1), (u1, v1)])
        z += random.uniform(0.5, 0.7)
    top = Vector((0, 0, height - 0.6))
    for a in (0.0, math.pi / 2):
        side = Vector((math.cos(a), math.sin(a), 0.0)) * 0.35
        crown.add([top - side, top + side, top + side + Vector((0, 0, 1.2)), top - side + Vector((0, 0, 1.2))],
                  [(0.93, 0.02), (0.93, 0.36), (0.03, 0.36), (0.03, 0.02)])
    return (wood.object(name + " wood", bark, smooth=True),
            crown.object(name + " needles", needles, smooth=False), r * 1.3)


# The scatter.

class Keepout:
    """Where nothing is scattered: the path and its branch, the gully, the chapel and its
    terrace, the clearing, the hero's start, and beyond the cliff."""

    def __init__(self, place, path_points):
        self.place = place
        self.path = [Vector((p.x, p.y)) for p in path_points]

    def path_distance(self, x, y):
        p = Vector((x, y))
        best = 1e9
        for a, b in zip(self.path, self.path[1:]):
            ab = b - a
            t = max(0.0, min(1.0, (p - a).dot(ab) / max(ab.length_squared, 1e-9)))
            best = min(best, (a + ab * t - p).length)
        return best

    def free(self, x, y, margin, in_gully=False):
        place = self.place
        ravine, chapel = place["ravine"], place["chapel"]
        if y > chapel["cliff"] - 1.0:
            return False
        if self.path_distance(x, y) < place["path"]["width"] / 2 + margin:
            return False
        if not in_gully and ravine["south"] - 1.0 - margin < y < ravine["north"] + 1.0 + margin:
            return False
        if chapel["x"][0] - 2.5 - margin < x < chapel["x"][1] + 2.5 + margin and y > chapel["y"][0] - 2.5 - margin:
            return False
        cx, cy = place["campfire"]["at"]
        if math.hypot(x - cx, y - cy) < 5.5 + margin:
            return False
        sx, sy = place["start"]["at"]
        if math.hypot(x - sx, y - sy) < 3.0 + margin:
            return False
        return True


def inside(place, x, y, margin=0.0):
    hx, hy = place["zone"]["half_size"]
    return abs(x) < hx + margin and abs(y) < hy + margin


def scatter_points(place, keepout, count, margin, spacing, random, taken, near_path=25.0, tries=40000):
    """Up to `count` points of the forest, `spacing` apart from each other and from `taken`, most
    within `near_path` metres of the path, where the hero sees them."""
    ex, ey = place["scatter"]["extent"]
    points = []
    for _ in range(tries):
        if len(points) >= count:
            break
        x, y = random.uniform(-ex, ex), random.uniform(-ey, ey)
        if not keepout.free(x, y, margin):
            continue
        d = keepout.path_distance(x, y)
        if d > near_path and random.random() < 0.7:
            continue
        if any((x - px) ** 2 + (y - py) ** 2 < spacing ** 2 for px, py, _ in taken + points):
            continue
        points.append((x, y, spacing))
    return points
