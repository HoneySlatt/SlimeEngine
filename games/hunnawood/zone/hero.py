"""The hero: the realistic male body of Blender Studio's Human Base Meshes, its arms lowered,
dressed here in a tunic, trousers, boots and a hooded cloak. A still figure, standing on the
ground at its origin and facing north (Blender's +y, glTF's -z), which the game moves."""

import math

import bmesh
import bpy
from mathutils import Matrix, Vector

import common

BUNDLE = "human_base_meshes/human-base-meshes-bundle-v1.4.1.zip"
BLEND = "human-base-meshes-bundle-v1.4.1/human_base_meshes_bundle.blend"
BODY = "GEO-body_male_realistic"
SKIN = (0.55, 0.39, 0.31)


def body(sources):
    """The body, its multiresolution sculpt left out, turned to face north, its arms lowered."""
    path = sources.from_zip(BUNDLE, BLEND)
    with bpy.data.libraries.load(path) as (src, dst):
        dst.objects = [BODY]
    obj = dst.objects[0]
    bpy.context.scene.collection.objects.link(obj)
    obj.modifiers.clear()
    obj.data = obj.data.copy()
    obj.matrix_world = Matrix.Rotation(math.pi, 4, "Z") @ obj.matrix_world
    common.apply_transform(obj)
    lower_arms(obj)
    low = min(v.co.z for v in obj.data.vertices)
    for v in obj.data.vertices:
        v.co.z -= low
    obj.name = "hero body"
    obj.data.name = "hero body"
    obj.data.materials.clear()
    obj.data.materials.append(common.material("skin", SKIN, roughness=0.6))
    return obj


def lower_arms(obj):
    """Turns each arm down about its shoulder, by an angle that grows from the armpit outwards,
    so that the arms hang beside the body instead of spreading as the base mesh's do."""
    vertices = obj.data.vertices
    top = max(v.co.z for v in vertices)
    shoulder_z = top * 0.815
    for side in (1, -1):
        pivot = Vector((side * 0.17, 0.0, shoulder_z))
        for v in vertices:
            x = v.co.x * side
            if x < 0.13 or v.co.z < shoulder_z - 0.45:
                continue
            weight = min(1.0, (x - 0.13) / 0.1)
            angle = math.radians(52) * weight * side
            v.co = pivot + Matrix.Rotation(angle, 3, "Y") @ (v.co - pivot)


def garment(source, name, keep, mat, offset, repeat):
    """A garment made of the faces of `source` whose center passes `keep`, pushed out along their
    normals by `offset` metres and given a thickness, its fabric projected every `repeat`
    metres."""
    mesh = source.data.copy()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if not keep(f.calc_center_median())], context="FACES")
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * offset
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.clear()
    mesh.materials.append(mat)
    solidify = obj.modifiers.new("thickness", "SOLIDIFY")
    solidify.thickness = 0.008
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=solidify.name)
    common.world_box_uvs(obj, repeat)
    return obj


def cloak(height, mat):
    """A cloak hanging from the shoulders to below the knees, open in front, and its hood."""
    builder_vertices, faces, uvs = [], [], []
    rings, around = 8, 20
    top, bottom = height * 0.81, height * 0.22
    for r in range(rings):
        t = r / (rings - 1)
        z = top + (bottom - top) * t
        radius = 0.21 + 0.23 * t ** 0.8
        if r == 0:
            # Gathered at the neck, above the shoulders.
            z, radius = height * 0.865, 0.1
        for k in range(around + 1):
            # The cloak's opening, in front (+y), is a sixth of the way around.
            a = math.radians(-30 - 300 * k / around) + math.pi / 2
            builder_vertices.append((radius * math.cos(a) * 1.15, radius * math.sin(a), z))
            uvs.append((k / around * 1.4 / 0.27, (z - bottom) / 0.27))
    for r in range(rings - 1):
        for k in range(around):
            a = r * (around + 1) + k
            faces.append((a, a + 1, a + around + 2, a + around + 1))
    corner_uvs = [uvs[i] for f in faces for i in f]
    obj = common.mesh_object("hero cloak", builder_vertices, faces, corner_uvs, mat, smooth=True)
    # The hood: most of a sphere around the head, open at the face.
    hood_center = Vector((0.0, -0.01, height * 0.915))
    v, f = [], []
    lat, lon = 8, 16
    for i in range(lat + 1):
        phi = math.pi * i / lat
        for j in range(lon + 1):
            theta = math.radians(35) + math.radians(290) * j / lon + math.pi / 2
            r = 0.135
            v.append(hood_center + Vector((r * math.sin(phi) * math.cos(theta),
                                           r * math.sin(phi) * math.sin(theta) * 1.1,
                                           r * math.cos(phi) * 1.15)))
    for i in range(lat):
        for j in range(lon):
            a = i * (lon + 1) + j
            f.append((a, a + 1, a + lon + 2, a + lon + 1))
    hood_uvs = [((idx % (lon + 1)) / lon * 3.0, (idx // (lon + 1)) / lat * 1.5) for face in f for idx in face]
    hood = common.mesh_object("hero hood", v, f, hood_uvs, mat, smooth=True)
    for part in (obj, hood):
        solidify = part.modifiers.new("thickness", "SOLIDIFY")
        solidify.thickness = 0.012
        bpy.context.view_layer.objects.active = part
        bpy.ops.object.modifier_apply(modifier=solidify.name)
        part.data.materials[0].use_backface_culling = False
    return common.join([obj, hood], "hero cloak")


def build(sources):
    """The hero, as one object of several materials, under a root named `hero`."""
    figure = body(sources)
    height = max(v.co.z for v in figure.data.vertices)
    tunic_mat = common.poly_haven_set(sources, "hessian_230", "2k", tint=(0.7, 0.68, 0.66))
    leather = common.poly_haven_set(sources, "fabric_leather_02", "2k", tint=(0.45, 0.5, 0.6))
    # The wool is orange as photographed: a dark brown cloak, dyed for the forest.
    wool = common.poly_haven_set(sources, "caban", "2k", double_sided=True, tint=(0.12, 0.3, 1.0))
    # The tunic covers the torso to the hips and the arms to the wrists; its skirt, below, reaches
    # the middle of the thighs.
    tunic = garment(figure, "hero tunic",
                    lambda c: (height * 0.45 < c.z < height * 0.82 and abs(c.x) < 0.2)
                    or height * 0.55 < c.z < height * 0.82,
                    tunic_mat, 0.012, 0.27)
    skirt = flared("hero tunic skirt", height * 0.56, height * 0.37, 0.17, 0.25, tunic_mat)
    trousers = garment(figure, "hero trousers",
                       lambda c: height * 0.09 < c.z < height * 0.5 and abs(c.x) < 0.25,
                       leather, 0.009, 0.5)
    boot_leather = common.poly_haven_set(sources, "fabric_leather_02", "2k", double_sided=True,
                                         tint=(0.3, 0.33, 0.4))
    boots = garment(figure, "hero boots", lambda c: c.z < height * 0.18, boot_leather, 0.022, 0.5)
    hero_cloak = cloak(height, wool)
    hero = common.join([figure, tunic, skirt, trousers, boots, hero_cloak], "hero")
    common.triangulate(hero)
    return hero


def flared(name, top, bottom, top_radius, bottom_radius, mat):
    """An open cone of cloth from `top` down to `bottom`, widening: a skirt."""
    around = 18
    vertices, faces, uvs = [], [], []
    for z, r in ((top, top_radius), (bottom, bottom_radius)):
        for k in range(around + 1):
            a = 2 * math.pi * k / around
            vertices.append((r * math.cos(a) * 1.2, r * math.sin(a), z))
    for k in range(around):
        quad = (k, k + 1, around + 2 + k, around + 1 + k)
        faces.append(quad)
        uvs += [((i % (around + 1)) / around * 1.4 / 0.27, (i // (around + 1)) * (top - bottom) / 0.27)
                for i in quad]
    obj = common.mesh_object(name, vertices, faces, uvs, mat, smooth=True)
    solidify = obj.modifiers.new("thickness", "SOLIDIFY")
    solidify.thickness = 0.01
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=solidify.name)
    return obj
