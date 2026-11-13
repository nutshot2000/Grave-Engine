# NVIDIA Kimodo / Mixamo animation lab

For the newer supplied FBX and Universal Animation Library setup, see
[Imported animation lab](IMPORTED-ANIMATION-LAB.md). The launcher uses that
local scene once it has been built. The original Kimodo outputs remain intact.

Open `Play-Animation-Lab.cmd`, then click **Play** in the editor.
Move with A/D or the arrow keys; W or Up jumps and swims upward.
Stand still to watch the greeting wave. The course has steps, a raised
ledge, a recoverable gap, a contained swimming pool, a pushable crate,
and an optional lava pit for testing death and automatic respawn.

Six fresh NVIDIA Kimodo clips drive the supplied Mixamo character:
idle greeting, brisk walk, jump, airborne fall, breaststroke, and a fall
onto its side. The generated 30-fps motion data, prompts, seeds and
generation settings are saved in `assets/motions/mixamo-lab/`.

The engine displays rendered animation frames in this 2D course.
The actual rig and six editable Blender actions are also saved in
`assets/characters/mixamo-lab-local/kimodo-mixamo-lab.blend`.
Actions are named `Kimodo_idle`, `Kimodo_walk`, `Kimodo_jump`,
`Kimodo_fall`, `Kimodo_swim` and `Kimodo_die`.

## Rebuild or make new motions

Keep the rigged model at `assets/characters/stuntman_main.fbx`, or pass
`--model FILE`. The original FBX is never changed. Model-derived frames,
the Blender file and the local scene stay local; the public example scene
uses the default hero so it works without that model.

Re-render the saved motions:

```sh
py tools/build_mixamo_lab.py --render-only
```

Generate the prompts again using an installed Kimodo runtime, then render:

```sh
py tools/build_mixamo_lab.py --kimodo PATH_TO_KIMODO_RUNTIME
```

Edit a clip's `prompt.txt` and matching `generation.json` prompt before
generating a different motion. The runtime must contain its models and
`build/Release/kmd-generate.exe`. Blender can be selected with `--blender`
or `GRAVE_BLENDER`. No model weights are bundled with the engine.
