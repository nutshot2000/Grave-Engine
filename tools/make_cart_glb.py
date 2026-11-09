# Builds assets/cart.glb with Blender, as an example of importing models.
#
#   blender -b --factory-startup --python tools/make_cart_glb.py
#
# The trick: name each Blender material after a Grave material ("wood", "iron",
# "stone"). Grave Engine looks at the model from the front (numpad 1) and turns
# it into cells. 1 Blender metre = 10 cells.

import os
import bpy

bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete()


def material(name, rgb):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*rgb, 1.0)
    return m


wood = material("wood", (0.48, 0.29, 0.15))
iron = material("iron", (0.54, 0.56, 0.6))
stone = material("stone", (0.42, 0.4, 0.44))


def add(obj, mat):
    obj.data.materials.append(mat)
    return obj


# Cart body: a wooden tray with sides.
bpy.ops.mesh.primitive_cube_add(location=(0, 0, 0.9))
add(bpy.context.object, wood).scale = (1.5, 0.6, 0.12)
for x in (-1.45, 1.45):
    bpy.ops.mesh.primitive_cube_add(location=(x, 0, 1.3))
    add(bpy.context.object, wood).scale = (0.08, 0.6, 0.3)

# Iron axle bar and two wheels (cylinders facing the front view).
bpy.ops.mesh.primitive_cube_add(location=(0, 0, 0.62))
add(bpy.context.object, iron).scale = (1.2, 0.1, 0.06)
for x in (-0.95, 0.95):
    bpy.ops.mesh.primitive_cylinder_add(radius=0.45, depth=0.15, location=(x, 0, 0.5), rotation=(1.5708, 0, 0))
    add(bpy.context.object, iron)

# A load of stone.
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.35, location=(-0.5, 0, 1.3))
add(bpy.context.object, stone)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.28, location=(0.3, 0, 1.25))
add(bpy.context.object, stone)

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "cart.glb")
bpy.ops.export_scene.gltf(filepath=os.path.abspath(out), export_format="GLB")
print("wrote", os.path.abspath(out))
