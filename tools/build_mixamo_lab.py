"""Generate fresh Kimodo clips, retarget a local Mixamo rig and build its test scene.
Use --kimodo RUNTIME to regenerate the prompts; omit it to render saved motion data.
The model and rendered look remain local. The original FBX is never changed.
"""
import argparse,json,os,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--kimodo',type=Path)
p.add_argument('--model',type=Path,default=ROOT/'assets/characters/stuntman_main.fbx')
p.add_argument('--blender',type=Path)
p.add_argument('--render-only',action='store_true')
a=p.parse_args()
motions=ROOT/'assets/motions/mixamo-lab'
if a.kimodo and not a.render_only:
 runtime=a.kimodo.resolve();env=os.environ.copy();env['KIMODO_BACKEND']='cpu';env['PATH']=str(runtime/'build/Release')+os.pathsep+env.get('PATH','')
 for kind in ['idle','walk','jump','fall','swim','die']:
  folder=motions/kind;meta=json.loads((folder/'generation.json').read_text(encoding='utf-8-sig'))
  print('Generating fresh NVIDIA Kimodo motion:',kind,flush=True)
  subprocess.run([str(runtime/'build/Release/kmd-generate.exe'),str(runtime/'models/kimodo-soma-rp-v1.1-f32.gguf'),str(runtime/'Llama-3-Kimodo-Q4_K_M.gguf'),str(folder/'prompt.txt'),str(meta['frames']),str(meta['steps']),str(meta['seed']),str(folder)],check=True,cwd=runtime,env=env)
if not a.model.is_file():raise SystemExit('Put your rigged Mixamo FBX in assets/characters/stuntman_main.fbx, or supply --model FILE.')
blender=str(a.blender or os.environ.get('GRAVE_BLENDER') or shutil.which('blender') or r'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe')
out=ROOT/'assets/characters/mixamo-lab-local'
subprocess.run([blender,'-b','--factory-startup','--python-exit-code','1','-P',str(ROOT/'tools/kimodo_to_mixamo.py'),'--',str(a.model.resolve()),str(motions),str(out),'--save-blend'],check=True,cwd=ROOT)
if not(out/'character.txt').is_file():raise SystemExit('Blender did not finish the character look.')
scene=(ROOT/'scenes/animation-lab.txt').read_text(encoding='utf-8').replace('look hero','look mixamo-lab-local')
(ROOT/'scenes/animation-lab.local.txt').write_text(scene,encoding='utf-8')
print('Ready: scenes/animation-lab.local.txt. Open Play-Animation-Lab.cmd.')
