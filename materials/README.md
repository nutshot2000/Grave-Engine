# Materials

One `.txt` file per material. The file name is the material's name (`molten_iron.txt` is
`molten_iron`). Each line is `key = value`. Lines starting with `# ` are comments.
Temperatures are in °C. Any key you leave out uses a sensible default.

Edit a file while the sandbox is running and it reloads instantly. Mistakes (typos,
unknown materials) show up in the window's Messages and in headless output, with
the file and line number.

## Basics

| Key | What it means | Default |
| --- | --- | --- |
| `about` | A short description, shown when you hover the palette button | |
| `state` | `solid` (stays put), `powder` (falls and piles), `liquid` (falls and spreads), `gas` (rises) | `solid` |
| `density` | kg per m³. Heavier things sink below lighter ones. Hot gases get lighter. | 1000 |
| `color` | `#rrggbb` or `r g b` | pink |
| `color_variation` | How speckled each cell is, 0 to 1 | 0.06 |
| `texture` | A picture tiled across everything made of it (e.g. `textures/wood.png`, relative to this folder). Without one, `color` and `color_variation` are used | none |
| `glow` | How much it lights up its surroundings, 0 to 1 | 0 |
| `crushable` | `yes` if falling objects crush it to nothing instead of landing on it (ash is) | no |
| `metal` | `yes` for metals: imported model parts marked metallic (Blender's "Metallic", glTF, OBJ `Pm`) only become metals, and other parts never do | no |
| `launch` | Bounce pad: anything landing on top is flung upwards at this speed (m/s). 15 clears a 12 m gap at a walk; landing after a bounce does no harm | 0 |
| `rolling_resistance` | How much it slows rolling wheels touching it, per second: loose sand 0.6, mud (wet sand) 1.4, water 0.3, gravel 0.3, hard ground 0. Brakes use `friction` instead (ice brakes badly) | 0 |
| `clump` | Tyres throw it in clumps of this many cells (wet sand and dirt 3; dry sand and gravel 1) | 1 |
| `tyres_dig` | `yes` for a solid whose top layer driven tyres chew up and throw (dirt) | no |
| `softens_at` | Above this temperature (°C) it gets weaker the hotter it is (strength, toughness, bending), down to nothing at its melting point. Iron 500 | none |
| `yield` | Metals: objects made of it **bend for good** when hit hard instead of only cracking. The number is the real-world yield strength in MPa (iron 200, brass 150; aluminium would be about 100, copper 70). Thin parts bend first (strength grows with thickness squared), red-hot metal bends easily (`softens_at`), and a huge hit or bending too far tears it in two | none |
| `slides` | Powders: how readily it slides down the side of a pile, 0 to 1. Dry sand 1 (45° slopes); wet sand 0.002 holds steep shapes | 1 |
| `flows` | Liquids and gases: how far it spreads sideways each frame | 4 |
| `strength` | Solids: how many cells it can stick out sideways (or hang down) from something holding it up before it snaps. Wood 50, stone 25, iron 150, ice 15. Standing on something solid counts as fully held up | 1 |

## Forces

| Key | What it means | Default |
| --- | --- | --- |
| `toughness` | How hard a hit (or how much weight) it takes to crack, in kilonewtons. Wood 1500, stone 5000, iron 30000, ice 300. `none` = never cracks | none |
| `breaks_into` | What cracked cells turn into (stone cracks into gravel). Without it, cracked bits splinter into small flying pieces | none |
| `friction` | How grippy it is, 0 to 1. Ice 0.03, wood 0.6, stone 0.8. The slipperier of two surfaces wins | 0.6 |
| `bounciness` | How much things bounce off it, 0 (thud) to 1 (superball). The bouncier of two surfaces wins | 0.05 |
| `explosion_size` | Explosives: blast radius in cells. Needs `catches_fire_at` (that's when it goes off) | 0 |
| `explosion_power` | Explosives: how hard the blast hits at its centre, in kilonewtons. Fades with distance; cracks anything whose `toughness` it beats | 0 |

A hit's force comes from the real physics: heavier and faster objects hit harder.
Lots of explosive going off together makes one bigger blast.

## Heat

| Key | What it means | Default |
| --- | --- | --- |
| `temperature` | Temperature when freshly painted | 20 |
| `conducts_heat` | How easily heat passes through, 0 to 1. Iron 0.9, wood 0.15 | 0.2 |
| `heat_capacity` | How much heat it takes to warm up. Water 4, air 0.25 | 1 |
| `cools_off` | How fast it drifts back to room temperature by itself, 0 to 1 per frame | 0 |
| `stays_at` | Holds this exact temperature (a heat source, like fire) | none |

Heat flows between neighbours as fast as the *worse* conductor of the two allows,
so wood and air insulate and iron spreads heat quickly.

## Fire

| Key | What it means | Default |
| --- | --- | --- |
| `catches_fire_at` | Starts burning at this temperature, if it touches something that feeds fire | none |
| `burns_for` | How many frames it burns | 100 |
| `burn_temperature` | How hot it burns | 800 |
| `burns_into` | What's left afterwards (`none` means air) | air |
| `decays_into_chance` | Chance (0 to 1) that something short-lived turns into `decays_into` when it runs out; otherwise it becomes air. Fire is 0.3, so only some flames leave smoke | 1 |
| `burns_into_chance` | Chance (0 to 1) that a burned cell leaves `burns_into` behind; otherwise it burns away completely. Wood is 0.1, so a burned house leaves a little ash, not a house-sized pile | 1 |
| `gives_off` | What it puffs into nearby air while burning | fire |
| `gives_off_chance` | Chance per frame of puffing | 0.1 |
| `feeds_fire` | `yes` if things can burn next to it (air and flames do) | no |
| `puts_out_fire` | `yes` if it stops burning things it touches (water does) | no |

A burning cell with nothing feeding it (buried, or surrounded by smoke) goes out, but
stays hot, so it may catch again later.

## Changing state

| Key | What it means |
| --- | --- |
| `melts_at`, `melts_into` | Turns into something else when it gets this hot (ice into water) |
| `boils_at`, `boils_into` | Same idea, usually liquid into gas (water into steam) |
| `freezes_at`, `freezes_into` | Turns into something else when it gets this cold (water into ice) |
| `condenses_at`, `condenses_into` | Same idea, usually gas into liquid (steam into water) |
| `lifetime`, `decays_into` | Lasts about this many frames, then becomes something else (fire into smoke) |

## Reactions

```text
reaction = water -> stone + steam, chance 0.2
```

"When I touch water, I become stone and the water becomes steam", with a 20% chance
each frame. You can have several `reaction` lines. Products start at their own normal
temperature.
