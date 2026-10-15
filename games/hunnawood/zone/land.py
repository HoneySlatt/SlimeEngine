"""The land: the forest floor, the path and its branch to the campfire, the gully that the bridge
crosses, and the cliff past the chapel's terrace, where the zone ends."""

from mathutils import Vector

import common

# How far the ground reaches beyond the forest, to the horizon's haze.
FAR = 160.0
# The size of one repeat of each texture, in metres (Poly Haven's dimensions of the sets).
FLOOR_REPEAT = 3.0
PATH_REPEAT = 2.17


def build(place, materials):
    zone = place["zone"]
    ravine = place["ravine"]
    chapel = place["chapel"]
    floor = materials["forest floor"]
    pieces = [
        rectangle("floor south", (-FAR, -FAR), (FAR, ravine["south"]), floor, FLOOR_REPEAT),
        rectangle("floor north", (-FAR, ravine["north"]), (FAR, chapel["cliff"]), floor,
                  FLOOR_REPEAT),
    ]
    gully(ravine, materials["path"])
    cliff_face(chapel["cliff"], materials["path"], common.rng(zone["seed"] + 1))
    bridge = place["bridge"]["y"]
    path = smooth(place["path"]["points"], 0.5)
    width = place["path"]["width"]
    random = common.rng(zone["seed"] + 2)
    before = [p for p in path if p.y < bridge[0] + 0.3]
    after = [p for p in path if p.y > bridge[1] - 0.3]
    ribbon("path south", before, width, materials["path"], random)
    ribbon("path north", after, width, materials["path"], random)
    ribbon("path to the campfire", smooth(place["path"]["branch"], 0.5), width * 0.7,
           materials["path"], random)
    return pieces


def rectangle(name, low, high, mat, repeat):
    (x0, y0), (x1, y1) = low, high
    corners = [(x0, y0, 0.0), (x1, y0, 0.0), (x1, y1, 0.0), (x0, y1, 0.0)]
    uvs = [(x / repeat, y / repeat) for x, y, _ in corners]
    return common.mesh_object(name, corners, [(0, 1, 2, 3)], uvs, mat)


def smooth(points, step):
    """The Catmull-Rom curve through `points`, sampled about every `step` metres."""
    pts = [Vector((x, y, 0.0)) for x, y in points]
    pts = [pts[0] * 2 - pts[1]] + pts + [pts[-1] * 2 - pts[-2]]
    out = []
    for i in range(1, len(pts) - 2):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[i + 1], pts[i + 2]
        count = max(1, int((p2 - p1).length / step))
        for k in range(count):
            t = k / count
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t
                              + (-p0 + 3 * p1 - 3 * p2 + p3) * t * t * t))
    out.append(pts[-2])
    return out


def ribbon(name, points, width, mat, random):
    """A strip of `width` along `points`, a little above the ground, its edges uneven."""
    if len(points) < 2:
        return None
    vertices, uvs_at = [], []
    along = 0.0
    for i, p in enumerate(points):
        ahead = points[min(i + 1, len(points) - 1)] - points[max(i - 1, 0)]
        side = Vector((-ahead.y, ahead.x, 0.0)).normalized()
        if i > 0:
            along += (p - points[i - 1]).length
        half = width / 2
        left = half * random.uniform(0.85, 1.15)
        right = half * random.uniform(0.85, 1.15)
        vertices += [p - side * right + Vector((0, 0, 0.02)), p + side * left + Vector((0, 0, 0.02))]
        uvs_at += [(-right / PATH_REPEAT, along / PATH_REPEAT), (left / PATH_REPEAT, along / PATH_REPEAT)]
    faces, uvs = [], []
    for i in range(len(points) - 1):
        a, b, c, d = 2 * i, 2 * i + 1, 2 * i + 3, 2 * i + 2
        faces.append((a, d, c, b))
        uvs += [uvs_at[a], uvs_at[d], uvs_at[c], uvs_at[b]]
    return common.mesh_object(name, vertices, faces, uvs, mat)


def gully(ravine, mat):
    """The gully: two slopes of earth and stones down to a floor `depth` below the ground, from
    one end of the forest to the other, uneven every few metres."""
    south, north, depth = ravine["south"], ravine["north"], ravine["depth"]
    random = common.rng(11)
    # Across the gully, from the south edge to the north one: (y offset, depth) pairs.
    step = 4.0
    columns = int(2 * FAR / step) + 1
    vertices, faces, uvs = [], [], []
    rows = 6
    for i in range(columns):
        x = -FAR + i * step
        w = random.uniform(-0.5, 0.5)
        floor_low = random.uniform(0.9, 1.1) * depth
        profile = [
            (south, 0.0),
            (south + 1.2 + w, -0.45 * floor_low),
            (south + 2.6 + w, -floor_low),
            (north - 2.6 + w, -floor_low),
            (north - 1.2 + w, -0.5 * floor_low),
            (north, 0.0),
        ]
        for y, z in profile:
            vertices.append((x, y, z))
    for i in range(columns - 1):
        for r in range(rows - 1):
            a = i * rows + r
            quad = (a, a + rows, a + rows + 1, a + 1)
            faces.append(quad)
            for v in quad:
                x, y, z = vertices[v]
                # Along the gully and down its slopes, so that the texture is not stretched there.
                uvs.append((x / PATH_REPEAT, (y + z) / PATH_REPEAT))
    return common.mesh_object("gully", vertices, faces, uvs, mat)


def cliff_face(y_edge, mat, random):
    """The cliff where the ground ends, past the chapel's terrace: a rough wall falling 60 m."""
    step = 3.0
    columns = int(2 * FAR / step) + 1
    levels = [0.0, -4.0, -12.0, -25.0, -40.0, -60.0]
    vertices, faces, uvs = [], [], []
    for i in range(columns):
        x = -FAR + i * step
        for k, z in enumerate(levels):
            y = y_edge + (0.0 if k == 0 else random.uniform(-1.5, 2.5) + k * 1.2)
            vertices.append((x, y, z))
    n = len(levels)
    for i in range(columns - 1):
        for k in range(n - 1):
            a = i * n + k
            quad = (a, a + n, a + n + 1, a + 1)
            faces.append(quad)
            for v in quad:
                x, _, z = vertices[v]
                uvs.append((x / PATH_REPEAT, z / PATH_REPEAT))
    return common.mesh_object("cliff", vertices, faces, uvs, mat)
