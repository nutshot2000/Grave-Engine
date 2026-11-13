"""Prepare a local climbing course with the imported character when available."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
source = (ROOT/'scenes/climbing-lab.txt').read_text(encoding='utf-8')
look = ROOT/'assets/characters/mixamo-fbx-local'
if (look/'character.txt').is_file():
    graph=(ROOT/'assets/animations/mixamo-pack.graph').read_text(encoding='utf-8')
    clips={l.split()[1]:l.split()[3] for l in graph.splitlines() if l.startswith('state ')}
    lines=[]
    for line in graph.splitlines():
        w=line.split()
        if w and w[0]=='blend' and not (look/'blends'/(clips[w[1]]+'--'+clips[w[2]])/'blend.txt').is_file():continue
        lines.append(line)
    (look/'animation.graph').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    source = source.replace('look hero','look mixamo-fbx-local')
(ROOT/'scenes/climbing-lab.local.txt').write_text(source,encoding='utf-8')
print('Climbing course ready: scenes/climbing-lab.local.txt')
