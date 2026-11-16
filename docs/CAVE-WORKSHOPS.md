# Cave storage and workshops

The mining trial now tests a small **mine → build → explore** loop.
Open `Play/Mining-Trial.cmd`, leave **Player tools** on and press **Play**.

[Elemental crawlers](CAVE-CREATURES.md) now guard the richer pockets. They
can climb onto walls and ceilings, retreat from hazards and release their
material payload on death. The separate gallery supplies weapons and ammo
for quick combat experiments.

Every terrain cell removed by your drill adds exactly one unit of that
material to storage. Rock, sand, sandstone, timber, ore and each crystal
colour have separate whole-number counts. B opens the complete inventory,
or click **Storage** in the toolbar. There is no weight limit in this test.
Liquids still block the drill; these are mining supplies, not a liquid pump.
Materials destroyed by burning or acid are not automatically collected.

The world pauses while storage is open. Close it with B or the window's X.
Controller: left-stick click opens/closes; D-pad up/down chooses a recipe;
Y crafts the selected recipe; LB cycles owned tools. Mouse buttons work too.

## First things to try

1. Mine the pale iron ore seam on the right side of camp and some rock and
   sandstone. The timber stack beside camp is a finite fuel source.
2. Follow the upper passage to the cyan gallery. Bring some crystals back
   to the **mechanic workshop at camp**.
3. Open B within reach of its green marker. Recipe rows show the exact
   quantities held and needed. Build a **reinforced drill** for faster
   cutting, or a **flame projector**. Newly built tools equip themselves.
4. Refine fuel at the mechanic bench. A flame projector needs stored oil;
   holding fire with an empty tank does nothing. Aim at the material you
   want to burn; distant airborne flames may disperse before reaching it.
5. Mine into the lower violet pocket. The **chemistry workshop in the cyan
   gallery** can build an acid sprayer and mix finite acid charges.
6. Try the steel grille and timber roots in the blind passage off the lower
   pocket. Acid dissolves steel; fire burns the timber. Bedrock resists acid.
   These are real material effects: fire can spread, and acid runs downhill
   and becomes water when it dissolves a cell or meets water. Acid contact
   hurts characters; fire and hot surroundings can burn you.

**F cycles tools.** Aim and hold click/E to use the selected tool; Q/right click
attaches/releases the rope, Space/S reels, W jumps/climbs and P pauses. The HUD names your tool and shows its
remaining ammunition. B lists every stored material and its exact count.

Crafting spends resources rather than merely checking lifetime totals.
Spending crystals lowers the corresponding carried mission quota, so bring
enough back to camp after building your equipment. Completing the mission
stays complete; the editor continues running for further experiments.

Reset, undo, rewind and scene save/load carry storage, owned tools, equipped
tool, workshops and terrain together. Inventory is retained on the engine's
normal character respawn for now. Older saves can recover their known
crystal inventory, but cannot reconstruct previously mined ordinary rock.
No standalone save is touched by playing the scene in the editor.

## Editing the prototype

- Scene markers: `workshop mechanic X Y RADIUS` and
  `workshop chemistry X Y RADIUS`. Add more anywhere you want a workshop.
  Bench geometry and lights are ordinary editable cells and scene lights;
  the marker defines the interaction area.
- Starting supplies: `stock MATERIAL COUNT`; equipment:
  `owned_tool ID`, `equipped_tool ID`. Saved scenes write these automatically.
- Tool definitions: `assets/crafting/cave-tools.txt`. Drill rows contain
  radius, reach, power and grade; spray rows name the emitted material,
  launch speed, interval in frames and ammunition material.
- Recipes: `assets/crafting/cave-recipes.txt`. Each row names its required
  workshop, one output, and exact input amounts. Tool recipes cannot charge
  twice for an already-owned tool. Supply recipes can be made repeatedly.
- The editor reloads the tool/recipe files while open. DEEPWARD reads them
  at startup and uses the same engine storage, crafting and tool rules.
- Acid reactions, jet temperature and contact harm are material data.
  `hurts_on_touch` is a generic character damage rate in health/second;
  ordinary materials default to zero. Mining hardness stays separate.

The recipes and yields are provisional game rules. This experiment does
not yet add enemies, a broader progression campaign or a balanced economy.
