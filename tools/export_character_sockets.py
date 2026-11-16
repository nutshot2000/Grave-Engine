"""Export animated hand anchors from the existing local Blender rig (no re-render)."""
import bpy
from bpy_extras.object_utils import world_to_camera_view
from pathlib import Path

out = Path(bpy.data.filepath).parent
rig = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
hand = next(b for b in rig.pose.bones if b.name.lower().replace(':', '').endswith('righthand'))
scene = bpy.context.scene
rows = ['# clip frame x y, in the same image pixels as the rendered animation']
for line in (out/'character.txt').read_text().splitlines():
    if ' frames,' not in line:
        continue
    clip, value = line.split(' = ')
    count = int(value.split()[0])
    action = bpy.data.actions.get('Imported_'+clip)
    if action is None:
        continue
    rig.animation_data.action = action
    if action.slots:
        rig.animation_data.action_slot = action.slots[0]
    for i in range(count):
        scene.frame_set(i+1)
        p = world_to_camera_view(scene, scene.camera, rig.matrix_world @ hand.head)
        rows.append(f'{clip} {i} {p.x*256:.3f} {(1-p.y)*256:.3f}')
(out/'hand.sockets').write_text('\n'.join(rows)+'\n')
print('EXPORTED', len(rows)-1, 'hand sockets')
