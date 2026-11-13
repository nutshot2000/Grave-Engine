# Imported animation lab

Open **Play-Animation-Lab.cmd**, then press **Play**. A/D moves and W/Up
jumps or swims upward. The course has steps, a gap, a contained pool and
an optional lava pit to test death and respawn.

The local rider now uses the supplied Mixamo clips and the Universal
Animation Library. The original files and earlier Kimodo look are preserved.
No generated Kimodo motion is needed when the full library is installed.

## What is connected

- Idle, walk, jog and death come from the Universal Animation Library.
  Movement speed chooses walk or jog; playback follows movement speed.
- Takeoff uses the supplied Jump FBX. Falling uses a looping airborne pose
  from the library until ground contact; landing uses its landing/recovery
  clip. Ground movement, takeoff, falling and landing have phase-matched
  pose blends. Release and press W again for another jump after landing.
- Swimming and treading water come from the two supplied water FBXs.
  Swimming input selects the forward stroke; releasing it selects treading.
- Hanging and climbing come from the supplied FBXs. **Play-Climbing-Lab.cmd**
  opens a course with physical ledge grabbing: hold W/Up while jumping
  towards a ledge, release and press it again to climb, or S/Down to drop.
  See [Ledge climbing](LEDGE-CLIMBING.md) for reusable scene settings.
- The other library actions have named states and triggers: crouching,
  rolling, driving, talking, interaction, pistol, sword, punches, spells,
  sitting and more. Once actions return when finished; looped actions hold
  until another action or release. Death has higher interruption priority.
  Sitting/spell entry states continue into their matching idle states.

## Inspect any animation

Click **Animations**. Under **Inspect clip**, choose a state and click
**Play clip only**. This isolates that clip from movement transitions.
Click **Restart preview** to restore the full graph.

To test transitions, choose an **Action** and click **Trigger action**.
For hanging, trigger **grab**, then **climb** or **release**. For water,
set swimming to 1 and toggle swim_moving between 0 and 1. Inputs in this
window affect the isolated preview, so the character in the course stays safe.

The edited graph is `assets/animations/mixamo-pack.graph`; the local
character loads its copy at `assets/characters/mixamo-fbx-local/animation.graph`.
The working Blender file is
`assets/characters/mixamo-fbx-local/mixamo-fbx-animations.blend`, with an
`Imported_` action for each rendered clip on the actual rider rig. The
reference T-pose is excluded from the playable states.

## Rebuild

```sh
py tools/build_fbx_animation_lab.py
```

This extracts only the non-root-motion GLB, README and licence from the
supplied ZIP, retargets by humanoid bone names while preserving the rider's
proportions, removes controller-owned root travel, renders transparent
frames, and writes the local look and scene. Use `--blender FILE` or
`GRAVE_BLENDER` to select Blender. The standalone FBXs, ZIP, extracted
library and rig-derived outputs stay local.

Universal Animation Library Standard is by Quaternius. The supplied
archive includes its CC0 1.0 licence; that licence stays beside the local
extracted library. The original Mixamo model and animations are kept intact.
