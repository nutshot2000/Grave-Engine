"""Prepare a local climbing course with the imported character when available."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
source = (ROOT/'scenes/climbing-lab.txt').read_text(encoding='utf-8')
look = ROOT/'assets/characters/mixamo-fbx-local'
if (look/'character.txt').is_file():
    (look/'animation.graph').write_text((ROOT/'assets/animations/mixamo-pack.graph').read_text(encoding='utf-8'),encoding='utf-8')
    source = source.replace('look hero','look mixamo-fbx-local')
(ROOT/'scenes/climbing-lab.local.txt').write_text(source,encoding='utf-8')
print('Climbing course ready: scenes/climbing-lab.local.txt')
