# Buried Dawn: first-night engine exercise

## Implemented reusable changes

- ActorSpec.control_override: independent optional Controls for AI/other players;
  None preserves existing global-input behavior. Both movement and animation
  consume the same effective controls. Included in snapshots/rewind.
- ActorSpec.respawn: defaults true for existing demos; false allows a game to
  keep a character dead until its checkpoint/result flow restores it.
- Physics.set_actor_controls, damage_actor and heal_actor expose checked,
  indexed game interaction without bypassing the shared health/death path.
  Invalid indices/non-finite or non-positive amounts are rejected.
- Named Shift input (`key Shift` or `modifier Shift`) and synthetic modifier
  input support. Press edges prevent a held roll key from repeating.

Regression coverage: independent opposite-direction movement, player isolation,
damage, non-respawn deaths, invalid input, healing rejection on corpses and
snapshot restoration; existing player/physics/editor behavior is preserved.

## Next priorities exposed by the prototype

1. Clearance-aware navigation with regional invalidation after digging, doors
   and collapse. Current game AI has an authored ramp route and local steering;
   arbitrary constructed mazes are outside its capability.
2. Public material ray/shape queries and terrain edit transactions, including
   changed-cell results, wake regions, reach/clearance and ownership handling.
   Games currently duplicate these loops.
3. Load-bearing connections across depth planes. Background pillars cannot
   currently support foreground terrain; avoid silently treating them as supports.
4. Device enable/load/fuel ports shared by lights, benches and generators;
   gameplay currently owns finite fuel explicitly. Physical fluids remain separate.
5. Scene actor persistence needs optional game fields, health, control ownership
   and lifecycle policy; native scene text currently saves actor placement/look,
   while game envelopes must restore health and actor-ID associations themselves.
6. Cropped renderer capture: Renderer.light clones entire back-plane Worlds on
   each frame. Simulation timing alone does not represent native frame cost.
7. Standard debug inspection for AI intention, blocked route, power state and
   stable entity IDs without exposing these internals in the normal game HUD.

This update adds no game-specific enemy, inventory or fuel rules to materials.
