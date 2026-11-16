# Cave crawlers and elemental variants

Open **Play/Cave-Explorer.cmd** for mine/craft progression, or
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

(The old Mining Trial scene has been removed.) DEEPWARD's bonus gallery
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

## Filling a whole level

`creatures AMOUNT [seed N] [avoid X Y RADIUS]` (after `generate` in a scene)
scatters elemental skitters and shellbacks over the level: real floors
with room to stand, none inside the `avoid` circle (the camp). The element
suits the place: water near flooded dips, oil near gas or oil, lava and
fire deep down, and a mix of fire, oil, water and some acid elsewhere.
Seeded, so a scene always gets the same creatures; at most 128 per scene.
DEEPWARD uses `creatures 0.5 seed 33 avoid 220 590 300` (about 50).
Endless maps don't scatter creatures yet.

Creatures further than `REST_DISTANCE` (260 cells) from the player keep
roaming and seeking, but quietly: their bodies are redrawn without waking
the cave, and they only dig, eat and drop sparks near the player (changing
the cave far away would keep it all awake). A creature found sealed in rock
(caves regenerated around it, or buried) hops to the nearest open floor. They are cold-blooded (they settle to the cave's
temperature): a 37-degree body warms the rock around it for ever, and heat
that is still spreading keeps the simulation awake. DEEPWARD runs at about 10 ms a frame with every creature roaming.

## Seeking, burrowing and feeding

Awake creatures smell their element up to `SMELL` (120 cells) away, once
a second, through rock too: water bugs seek water, oil bugs oil and
volatile gas (including sealed gas pockets), fire bugs fire, lava, embers
and torches, lava bugs lava and fire, acid bugs acid. They move their home
there and crawl over (mood `seeking`). If they stop getting closer they
chew a hole ahead (mood `burrowing`): soft ground up to hardness 0.75 (rock,
shale, sandstone, dirt), never stone, basalt, bedrock or crystals, and
never the ground under their own feet; some of it falls as dust. Next to
their element they eat it (mood `feeding`, 8 cells every third of a
second) and their death burst grows by one cell per cell eaten, up to 400.
So an oil bug can dig into a gas pocket and let the gas out, a water bug
drains a pool, and a fire bug that ate a torch dies in a bigger fireball.
Player hunting still comes first. Creatures far from the player rest, so
this only happens near you. Test:
`creatures_seek_burrow_to_and_feed_on_their_element`.

## More species

`assets/creatures/cave.creatures` also has (element in brackets):

| Species | Eats | On death | Notes |
| --- | --- | --- | --- |
| Crystal mite (`crystal_cyan` / `crystal_violet`) | cyan and violet crystals | bursts out the crystals it ate (mine them for samples) | small and quick; competes for your quota |
| Gas bloater (`volatile_gas`) | volatile gas | a cloud of gas | big and slow; kill it near fire and it explodes |
| Gravel worm (`gravel`) | shale, sandstone, dirt | gravel | tunnels through soft rock leaving gravel |
| Ember beetle (`ember`) | gas and oil | embers | drops sparks while running, setting off gas |

`creatures AMOUNT` mixes them in: about 38% skitters, 20% shellbacks, 14%
worms, 12% beetles, 10% mites, 6% bloaters, and mites wherever crystals are
near. Test: `mites_worms_and_beetles_do_their_thing`.

Each newer species has its own body (`shaped` in `src/creatures.rs`): a
crystal-spiked mite on spindly legs, a breathing gas sac on stubby legs, a
slim ringed worm that ripples, and a beetle with glowing ember spots. The
picture is also the body (heat, contact, acid). Corpses use the shared flat
shape. `cargo run --release --example bug_sheet` draws every species, alive
and dead, to `shots/creature-sheet.png`.
