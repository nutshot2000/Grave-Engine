# Builds a small watchtower in Blender and exports it as .glb, .obj and .fbx
# into assets/, to test Grave Engine's importers. Run it inside Blender
# (Scripting tab, or through an MCP connection). It works in its own scene,
# "Grave Engine test", so your scene is left alone.
#
# What it shows:
# - parts named after Grave materials (stone, wood) become those materials
# - the roof has a texture picture; its colours pick the closest materials
# - the flag is a plain paint colour; it picks the closest material

import os
import bpy

ASSETS = os.path.join(os.path.expanduser("~"), "Desktop", "Grave-Engine", "assets")
os.makedirs(ASSETS, exist_ok=True)

old_scene = bpy.context.window.scene
scene = bpy.data.scenes.get("Grave Engine test") or bpy.data.scenes.new("Grave Engine test")
for o in list(scene.collection.objects):
    bpy.data.objects.remove(o, do_unlink=True)
bpy.context.window.scene = scene


def mat(name, rgb):
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    m.diffuse_color = (*rgb, 1.0)
    return m


def box(name, loc, size, material):
    bpy.ops.mesh.primitive_cube_add(location=loc)
    o = bpy.context.object
    o.name = name
    o.scale = (size[0] / 2, size[1] / 2, size[2] / 2)
    o.data.materials.append(material)
    return o


stone = mat("stone", (0.42, 0.4, 0.44))
wood = mat("wood", (0.48, 0.29, 0.15))
flag = mat("Flag Paint", (0.66, 0.85, 0.94))

# Roof texture: wood-brown and iron-grey stripes, made right here and saved
# next to the exports (OBJ files need the picture as a separate file).
img = bpy.data.images.get("roof_tiles") or bpy.data.images.new("roof_tiles", 32, 32)
px = []
for y in range(32):
    for x in range(32):
        if (y // 4) % 2 == 0:
            px += [0.48, 0.29, 0.15, 1.0]  # wood brown
        else:
            px += [0.54, 0.56, 0.6, 1.0]  # iron grey
img.pixels = px
img.filepath_raw = os.path.join(ASSETS, "tower_roof.png")
img.file_format = "PNG"
img.save()
roof = bpy.data.materials.get("Roof Tiles") or bpy.data.materials.new("Roof Tiles")
roof.use_nodes = True
nodes = roof.node_tree.nodes
tex = nodes.get("Image Texture") or nodes.new("ShaderNodeTexImage")
tex.image = img
roof.node_tree.links.new(tex.outputs["Color"], nodes["Principled BSDF"].inputs["Base Color"])

# The tower (1 Blender unit = 1 metre = 10 cells in Grave Engine).
box("Base", (0, 0, 0.3), (2.4, 1.0, 0.6), stone)
for x in (-0.9, 0.9):
    box(f"Leg{x}", (x, 0, 1.6), (0.2, 0.2, 2.0), wood)
box("Platform", (0, 0, 2.7), (2.4, 1.2, 0.2), wood)
bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1.6, depth=1.0, location=(0, 0, 3.3), rotation=(0, 0, 0.785398))
r = bpy.context.object
r.name = "Roof"
r.data.materials.append(roof)
box("Pole", (0.0, 0, 4.3), (0.08, 0.08, 1.0), wood)
box("Flag", (0.3, 0, 4.6), (0.5, 0.05, 0.3), flag)

bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(filepath=os.path.join(ASSETS, "tower.glb"), export_format="GLB", use_selection=True)
bpy.ops.wm.obj_export(filepath=os.path.join(ASSETS, "tower.obj"), export_selected_objects=True, path_mode="RELATIVE", export_pbr_extensions=True)
bpy.ops.export_scene.fbx(filepath=os.path.join(ASSETS, "tower.fbx"), use_selection=True, path_mode="COPY", embed_textures=True)

bpy.context.window.scene = old_scene
print("exported tower to", ASSETS)
