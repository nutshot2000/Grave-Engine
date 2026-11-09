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
| `glow` | How much it lights up its surroundings, 0 to 1 | 0 |
| `crushable` | `yes` if falling objects crush it to nothing instead of landing on it (ash is) | no |
| `flows` | Liquids and gases: how far it spreads sideways each frame | 4 |
| `strength` | Solids: how many cells it can stick out sideways (or hang down) from something holding it up before it snaps. Wood 50, stone 25, iron 150, ice 15. Standing on something solid counts as fully held up | 1 |

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
