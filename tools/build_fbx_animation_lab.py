"""Build the local character lab from supplied Mixamo clips and an optional UAL ZIP."""
import argparse
import os
import shutil
import subprocess
from pathlib import Path
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--blender', type=Path)
p.add_argument('--model', type=Path, default=ROOT/'assets/characters/stuntman_main.fbx')
p.add_argument('--animations', type=Path, default=ROOT/'assets/animations')
a = p.parse_args()
required = ['Climbing.fbx','Hanging Idle.fbx','Jump.fbx','Swimming.fbx','Treading Water.fbx']
for f in [a.model,*[a.animations/n for n in required]]:
    if not f.is_file():
        raise SystemExit(f'Missing input: {f.name}')
packs = sorted(a.animations.glob('Universal Animation Library*.zip'))
if packs:
    library = a.animations/'ual-local'
    library.mkdir(exist_ok=True)
    with ZipFile(packs[0]) as archive:
        # Extract only these known files, using fixed local names (no ZIP paths).
        for name in ['UAL1_Standard.glb','License.txt','README.txt']:
            entry = next((i for i in archive.infolist() if i.filename.endswith('/'+name)),None)
            if entry is None or entry.file_size > 64*1024*1024:
                raise SystemExit(f'Animation library is missing a supported {name}')
            (library/name).write_bytes(archive.read(entry))
blender = str(a.blender or os.environ.get('GRAVE_BLENDER') or shutil.which('blender') or r'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe')
subprocess.run([blender,'-b','--factory-startup','--python-exit-code','1','-P',str(ROOT/'tools/fbx_animation_lab.py'),'--','--model',str(a.model.resolve()),'--animations',str(a.animations.resolve()),'--library',str((a.animations/'ual-local/UAL1_Standard.glb').resolve())],check=True,cwd=ROOT)
if (a.animations/'ual-local/UAL1_Standard.glb').is_file():
    out=ROOT/'assets/characters/mixamo-fbx-local'
    subprocess.run([blender,'-b','--factory-startup','--python-exit-code','1','-P',str(ROOT/'tools/blend_character_transition.py'),'--','--blend',str(out/'mixamo-fbx-animations.blend'),'--out',str(out),'--from-action','Imported_climb','--to-action','Imported_idle','--name','climb_finish','--seconds','0.4','--graph',str(ROOT/'assets/animations/mixamo-pack.graph')],check=True,cwd=ROOT)
    subprocess.run([blender,'-b','--factory-startup','--python-exit-code','1','-P',str(ROOT/'tools/bake_movement_blends.py'),'--','--look',str(out),'--graph',str(ROOT/'assets/animations/mixamo-pack.graph')],check=True,cwd=ROOT)
print('Ready. Open Play-Animation-Lab.cmd and use Animations to inspect all clips.')
