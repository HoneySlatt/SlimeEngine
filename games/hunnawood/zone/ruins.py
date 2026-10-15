"""The stonework: the bridge over the gully, the ruined chapel, its sealed door's wall and the walled
terrace behind it. Each piece of masonry is a mesh in the stone material; each piece the hero may
not cross is a collider."""

import math

import bpy

import common

STONE_REPEAT = 1.5
WALL = 0.9


def box(name, low, high, mat):
    """A box from corner `low` to corner `high`, (x, y, z) each."""
    (x0, y0, z0), (x1, y1, z1) = low, high
    v = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
         (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    return common.mesh_object(name, v, faces, material=mat)


def prism(name, center, radius, bottom, top, mat, sides=8, broken=None, random=None):
    """An upright prism of `sides` around `center` (x, y), from `bottom` to `top`; a `broken` top
    rises and falls by up to that many metres, as a shaft snapped off."""
    cx, cy = center
    v = []
    for z in (bottom, top):
        for k in range(sides):
            a = 2 * math.pi * (k + 0.5) / sides
            dz = 0.0
            if z == top and broken:
                dz = random.uniform(-broken, broken)
            v.append((cx + radius * math.cos(a), cy + radius * math.sin(a), z + dz))
    faces = [tuple(range(sides - 1, -1, -1)), tuple(range(sides, 2 * sides))]
    for k in range(sides):
        n = (k + 1) % sides
        faces.append((k, n, sides + n, sides + k))
    return common.mesh_object(name, v, faces, material=mat)


def arch_prism(name, along_x, center, width, bottom, spring, depth):
    """A solid in the shape of a pointed arch, to cut an opening from a wall: `width` wide, from
    `bottom` up to `spring`, where two arcs of radius `width` rise to the apex. The arch's plane
    is across the x axis when `along_x` (a wall that runs along y), else across y; it is `depth`
    thick through the wall."""
    outline = [(-width / 2, bottom), (width / 2, bottom), (width / 2, spring)]
    steps = 8
    # The right arc, centered on the left springing point, then the left one.
    for k in range(1, steps + 1):
        a = math.radians(60) * k / steps
        outline.append((-width / 2 + width * math.cos(a), spring + width * math.sin(a)))
    for k in range(steps - 1, -1, -1):
        a = math.radians(60) * k / steps
        outline.append((width / 2 - width * math.cos(a), spring + width * math.sin(a)))
    cx, cy = center
    v = []
    for side in (-depth / 2, depth / 2):
        for u, z in outline:
            v.append((cx + side, cy + u, z) if along_x else (cx + u, cy + side, z))
    n = len(outline)
    faces = [tuple(range(n)), tuple(range(2 * n - 1, n - 1, -1))]
    for k in range(n):
        m = (k + 1) % n
        faces.append((k, m, n + m, n + k))
    return common.mesh_object(name, v, faces)


def cut_out(obj, cutters):
    """Removes each of `cutters` from `obj`, then deletes them."""
    for cutter in cutters:
        modifier = obj.modifiers.new("cut", "BOOLEAN")
        modifier.operation = "DIFFERENCE"
        modifier.solver = "EXACT"
        modifier.object = cutter
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=modifier.name)
        bpy.data.objects.remove(cutter)
    return obj


def wall(name, low, high, mat, openings=(), random=None, ragged=0.0):
    """A wall from corner `low` to `high`, with pointed `openings` (center along the wall, width,
    bottom, spring) cut through it; a `ragged` top is broken into steps up to that deep."""
    (x0, y0, _), (x1, y1, z1) = low, high
    along_y = (y1 - y0) > (x1 - x0)
    pieces = [box(name, low, high, mat)]
    if ragged:
        # Broken masonry: blocks of the top course gone, stepping down along the wall.
        length = (y1 - y0) if along_y else (x1 - x0)
        cuts = []
        s = 0.0
        while s < length:
            step = random.uniform(0.6, 1.6)
            drop = random.uniform(0.0, ragged)
            if drop > 0.15:
                if along_y:
                    lo, hi = (x0 - 0.1, y0 + s, z1 - drop), (x1 + 0.1, y0 + s + step, z1 + 0.1)
                else:
                    lo, hi = (x0 + s, y0 - 0.1, z1 - drop), (x0 + s + step, y1 + 0.1, z1 + 0.1)
                cuts.append(box(name + " gap", lo, hi, None))
            s += step
        cut_out(pieces[0], cuts)
    holes = []
    for along, width, bottom, spring in openings:
        if along_y:
            center = ((x0 + x1) / 2, along)
        else:
            center = (along, (y0 + y1) / 2)
        thickness = (x1 - x0) if along_y else (y1 - y0)
        holes.append(arch_prism(name + " opening", along_y, center, width, bottom, spring,
                                thickness + 0.2))
    cut_out(pieces[0], holes)
    return pieces[0]


def finish(objects, name):
    """Joins the masonry into one mesh, triangulated, its texture projected from the world."""
    obj = common.join(objects, name)
    common.world_box_uvs(obj, STONE_REPEAT)
    common.triangulate(obj)
    return obj


def bridge(place, stone):
    """A stone bridge of two pointed arches over the gully, with parapets and a pinnacle at each
    corner, its deck level with the ground."""
    (x0, x1), (y0, y1) = place["bridge"]["x"], place["bridge"]["y"]
    ravine = place["ravine"]
    random = common.rng(21)
    parts = [box("deck", (x0, y0, -0.6), (x1, y1, 0.03), stone)]
    body = box("spandrels", (x0 + 0.1, ravine["south"] - 0.4, -ravine["depth"] - 0.3),
               (x1 - 0.1, ravine["north"] + 0.4, -0.5), stone)
    middle = (ravine["south"] + ravine["north"]) / 2
    span = (ravine["north"] - ravine["south"]) / 2 - 0.6
    openings = [
        arch_prism("arch", True, ((x0 + x1) / 2, middle - span / 2 - 0.3), span,
                   -ravine["depth"] - 0.5, -ravine["depth"] - 0.4, x1 - x0 + 0.4),
        arch_prism("arch", True, ((x0 + x1) / 2, middle + span / 2 + 0.3), span,
                   -ravine["depth"] - 0.5, -ravine["depth"] - 0.4, x1 - x0 + 0.4),
    ]
    parts.append(cut_out(body, openings))
    for (a, b) in ((x0, x0 + 0.35), (x1 - 0.35, x1)):
        parts.append(wall("parapet", (a, y0 + 0.3, 0.0), (b, y1 - 0.3, 0.95), stone,
                          random=random, ragged=0.35))
    for cx in (x0 + 0.175, x1 - 0.175):
        for cy in (y0 + 0.3, y1 - 0.3):
            parts += pinnacle(cx, cy, stone)
    obj = finish(parts, "bridge")
    # The gully stops the hero but for the bridge: one collider on each side of it, the width of the
    # ground.
    half = place["zone"]["half_size"][0] + 2.0
    south, north = ravine["south"] - 0.4, ravine["north"] + 0.4
    colliders = [
        common.collider_box("gully west", ((-half + x0) / 2, (south + north) / 2), (x0 + half, north - south), 3.0),
        common.collider_box("gully east", ((half + x1) / 2, (south + north) / 2), (half - x1, north - south), 3.0),
        common.collider_box("bridge parapet west", (x0 + 0.175, (y0 + y1) / 2), (0.5, y1 - y0), 1.2),
        common.collider_box("bridge parapet east", (x1 - 0.175, (y0 + y1) / 2), (0.5, y1 - y0), 1.2),
    ]
    return obj, colliders


def pinnacle(cx, cy, stone):
    """A gothic pinnacle: a square post with a pointed spire."""
    post = box("pinnacle", (cx - 0.3, cy - 0.3, 0.0), (cx + 0.3, cy + 0.3, 1.9), stone)
    v = [(cx - 0.34, cy - 0.34, 1.9), (cx + 0.34, cy - 0.34, 1.9), (cx + 0.34, cy + 0.34, 1.9),
         (cx - 0.34, cy + 0.34, 1.9), (cx, cy, 2.9)]
    faces = [(0, 3, 2, 1), (0, 1, 4), (1, 2, 4), (2, 3, 4), (3, 0, 4)]
    spire = common.mesh_object("spire", v, faces, material=stone)
    return [post, spire]


def chapel(place, stone):
    """The ruined chapel: broken walls with pointed windows, two rows of pillars, the north wall
    with the sealed door's opening, and the terrace behind it, walled on three sides."""
    c = place["chapel"]
    (x0, x1), (y0, y1) = c["x"], c["y"]
    north = c["terrace_north"]
    random = common.rng(31)
    parts, colliders = [], []

    def add_wall(name, low, high, openings=(), ragged=0.0):
        parts.append(wall(name, low, high, stone, openings, random, ragged))
        (a, b, _), (d, e, h) = low, high
        colliders.append(common.collider_box(name, ((a + d) / 2, (b + e) / 2), (d - a, e - b), h))

    # West wall, from the portal northwards: a stump, a tall piece with a window, a broken piece,
    # a tall piece with a window.
    add_wall("west wall", (x0, y0, 0), (x0 + WALL, y0 + 4.0, 1.4), ragged=0.8)
    add_wall("west wall", (x0, y0 + 4.0, 0), (x0 + WALL, y0 + 10.0, 5.6), [(y0 + 7.0, 1.6, 1.6, 3.6)], ragged=0.6)
    add_wall("west wall", (x0, y0 + 10.0, 0), (x0 + WALL, y0 + 15.0, 3.0), ragged=1.6)
    add_wall("west wall", (x0, y0 + 15.0, 0), (x0 + WALL, y1, 6.6), [(y0 + 18.0, 1.6, 1.8, 4.0)], ragged=0.5)
    # East wall: a tall piece with a window, a fallen stretch, a tall piece with a window.
    add_wall("east wall", (x1 - WALL, y0, 0), (x1, y0 + 6.0, 6.0), [(y0 + 3.0, 1.6, 1.6, 3.8)], ragged=0.5)
    add_wall("east wall", (x1 - WALL, y0 + 6.0, 0), (x1, y0 + 11.0, 1.8), ragged=1.0)
    add_wall("east wall", (x1 - WALL, y0 + 11.0, 0), (x1, y1, 6.8), [(y0 + 14.0, 1.6, 1.8, 4.0), (y0 + 18.0, 1.6, 1.8, 4.0)], ragged=0.4)
    # The portal's wall in the south: two jambs of different heights around a 3.6 m gap.
    add_wall("portal wall", (x0, y0, 0), (-1.8, y0 + WALL, 4.6), ragged=1.2)
    add_wall("portal wall", (1.8, y0, 0), (x1, y0 + WALL, 2.6), ragged=1.4)
    # The north wall, whole, with the door's opening and a tall window above it; its collider is
    # cut in two around the door, which has its own.
    door_half = 1.05
    parts.append(wall("north wall", (x0, y1, 0), (x1, y1 + WALL, 8.0), stone,
                      [(0.0, 2.1, -0.1, 2.0), (0.0, 1.4, 4.2, 6.4)], random))
    # The stone that fills the arch above the door, which is lower than the opening.
    parts.append(box("tympanum", (-1.1, y1 + 0.3, 2.95), (1.1, y1 + 0.6, 3.9), stone))
    colliders.append(common.collider_box("north wall", ((x0 - door_half) / 2, y1 + WALL / 2),
                                         (-door_half - x0, WALL), 8.0))
    colliders.append(common.collider_box("north wall", ((x1 + door_half) / 2, y1 + WALL / 2),
                                         (x1 - door_half, WALL), 8.0))
    # The terrace behind the door: its two walls, as high as a man and more, and the low parapet on
    # the edge of the cliff, over which the view opens.
    add_wall("terrace wall", (x0, y1 + WALL, 0), (x0 + WALL, north, 3.4), ragged=0.6)
    add_wall("terrace wall", (x1 - WALL, y1 + WALL, 0), (x1, north, 3.4), ragged=0.6)
    add_wall("terrace parapet", (x0, north, 0), (x1, north + 0.6, 1.1), ragged=0.25)
    parts.append(box("terrace floor", (x0 + WALL, y1 + WALL, 0.0), (x1 - WALL, north, 0.04), stone))
    # The pillars of the nave, some whole, some snapped, one fallen in drums.
    heights = [6.4, 2.3, 6.4, 4.1, 1.2, 6.4, 3.2, 6.4]
    k = 0
    for x in (-3.0, 3.0):
        for y in (y0 + 4.5, y0 + 8.5, y0 + 12.5, y0 + 16.5):
            h = heights[k]
            k += 1
            parts.append(box("plinth", (x - 0.55, y - 0.55, 0.0), (x + 0.55, y + 0.55, 0.4), stone))
            whole = h > 6
            parts.append(prism("shaft", (x, y), 0.38, 0.4, h, stone,
                               broken=None if whole else 0.25, random=random))
            if whole:
                parts.append(prism("capital", (x, y), 0.55, h, h + 0.35, stone))
            colliders.append(common.collider_cylinder("pillar", (x, y), 0.6, h))
    # Drums of the fallen shafts, on the floor of the nave.
    for (x, y, yaw) in ((1.2, y0 + 10.0, 0.4), (1.9, y0 + 9.0, 1.3), (-1.4, y0 + 13.2, 2.2)):
        parts.append(drum(x, y, yaw, stone))
        colliders.append(common.collider_box("fallen drum", (x, y), (0.95, 0.8), 0.8, yaw))
    return finish(parts, "chapel"), colliders


def drum(x, y, yaw, stone):
    """A drum of a fallen shaft, lying on its side."""
    obj = prism("drum", (0.0, 0.0), 0.38, -0.45, 0.45, stone)
    obj.rotation_euler = (math.pi / 2, 0.0, yaw)
    obj.location = (x, y, 0.36)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.select_set(False)
    return obj
