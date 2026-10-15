"""Views of the zone rendered with Blender's EEVEE, to look at it without the game: from where the
hero starts, on the bridge, at the campfire, in the chapel and on the terrace, and from above."""

import os

import bpy
from mathutils import Vector

# (name, eye, target), in Blender's axes.
VIEWS = [
    ("start", (0.0, -54.0, 2.4), (0.0, -40.0, 1.6)),
    ("bridge", (1.0, -26.0, 2.2), (2.0, -10.0, 1.0)),
    ("gully", (-8.0, -15.0, 1.8), (6.0, -15.5, -1.5)),
    ("campfire", (-5.0, 0.0, 2.2), (-13.0, 4.0, 0.5)),
    ("chapel", (0.0, 14.0, 2.2), (0.0, 30.0, 3.0)),
    ("nave", (-1.0, 26.0, 1.9), (0.0, 44.0, 2.5)),
    ("terrace", (0.0, 47.0, 1.8), (0.0, 120.0, -6.0)),
    ("above", (-40.0, -80.0, 70.0), (0.0, 0.0, 0.0)),
]


def render(directory):
    os.makedirs(directory, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 1280
    scene.render.resolution_y = 720
    scene.view_settings.view_transform = "AgX"
    world = bpy.data.worlds.new("overcast")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.62, 0.66, 0.72, 1.0)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 1.0
    scene.world = world
    # The moon and the lanterns light the night; the views show the day.
    for obj in scene.objects:
        if obj.type == "LIGHT" and obj.get("time") == "night":
            obj.hide_render = True
    camera = bpy.data.cameras.new("view")
    camera.lens = 24
    cam = bpy.data.objects.new("view", camera)
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.view_settings.exposure = -3.6
    for name, eye, target in VIEWS:
        cam.location = eye
        cam.rotation_euler = (Vector(target) - Vector(eye)).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)
