# Cave crawlers and elemental variants

Open **Play/Mining-Trial.cmd** for mine/craft progression, or
**Play/Elemental-Creatures.cmd** for a gallery with weapons and generous ammo
already supplied. Everything runs in editor Play mode. B opens storage,
F changes tools, aim and hold click/E to use one, and Space flies.

The gallery's chambers are fire, acid, oil, lava and water from left to right.
Drop down a shaft to try a fight, then fly back to the upper passage.
Reset restores creatures, resources, spills, equipment and ammunition.

## Movement and combat

- **Cave skitters** are quick and fragile. **Crystal shellbacks** move more
  slowly, have a larger body and take more punishment.
- Both follow exposed terrain: floors, slopes, walls, ceiling undersides,
  pillars and corners. Bodies and legs turn with the surface. They follow
  connected routes; they cannot fly between islands or squeeze through a
  gap smaller than their body. Removing their support makes them fall.
- They patrol their territory and investigate tool noise. A player seen
  within their territory is pursued, including around walls from a ceiling.
  Solid walls block sight and bites.
- A bite has a visible **BITE!** warning, followed by a recovery. Move out of
  reach or fly to make it miss. The drill cannot erase owned body cells.
- Dangerous heat and chemicals interrupt attacks and cause retreats. They
  can climb away from a poorly aimed stream: aim at the moving creature.
- Heat, acid contact and drowning affect real character bodies. Defeated
  creatures do not respawn until Reset; ceiling creatures drop when killed.

The first cyan and small violet seams in the Mining Trial stay accessible
before the hard fights. A fire shellback guards the richer lower deposit,
and an acid skitter crawls around the sealed cache. DEEPWARD's bonus gallery
also has guards. This is a first balance pass, not a finished enemy roster.

## Elemental death spills

Species and element are independent: either species supports every variant,
and other defined materials work too. Shell colours, glowing accents and
labels identify the payload.

| Variant | On death | Combat consideration |
| --- | --- | --- |
| Fire | A burst of real flames | Resists heat. Acid is a better choice; nearby timber or oil can ignite. |
| Acid | Droplets that collect and flow | Resists its own acid. Use fire and stand back from the pool. |
| Oil | A finite oil spill | Flammable. Fire can ignite the spill; water can help control the aftermath. |
| Lava | Hot molten droplets | Resists heat. Acid can kill it, but the lava remains and reacts with water. |
| Water | A splash that gathers into a pool | Can cool a fight, put out fire or neutralise acid. The gallery has a large water payload. |

These are authored quantities of simulated material, not decorative
particles. Acid eats scenery and is spent to water; oil burns; lava heats
and reacts; water flows and extinguishes through the normal material rules.
A fire burst can ignite an oil death spill without an enemy-specific script.
Other creatures and the player can be caught in the aftermath.

Each creature releases its payload once. Save/load, undo, rewind and Reset
keep health, surface pose, brain, death and release flags with the world.
Rewinding before a kill removes its spill and restores a living creature;
replaying the kill creates one fresh spill. Saved corpses cannot keep emitting.

## Editing

Placements and variants are scene data. A creature line names its species,
X and starting feet Y. Optional fields include element, burst quantity,
health, home, surface normal and runtime state. Quantities are bounded to
1–2048 cells; the default is 80. Changing an acid skitter to a water variant
turns the same encounter into a cooling spill. Geometry and resources remain
independently editable.

`assets/creatures/cave.creatures` defines health, speed, sight, hearing,
territory, bite damage, warning/recovery durations, avoidance, colour and
body height. The editor reloads changed profiles while preserving damage
and brain state. The standalone game reads them on load. Native articulated
artwork and the material body use the same rotated mask.

Current placements are authored in finite trial and expedition scenes.
Automatic population of newly streamed endless regions is separate work.
