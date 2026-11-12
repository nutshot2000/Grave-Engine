# Asset Workshop

Open Grave Engine and click **Asset workshop**, or launch `grave-engine --workshop` from the project folder. The workshop opens with a fuel-crate example. `--workshop --workshop-file assets/workshop/NAME.asset` opens a particular saved draft. A framed-glass example is in **Open saved asset...**.

## Make and test a prop

1. Click **New asset** or open an example. Give it a filename using letters, numbers, `-` and `_`.
2. Choose a named part on the left, then a material. Add another part for a window, tank, base or other component. Rename parts with **Rename part**.
3. Draw with Pencil, Erase, Fill, Line, Rectangle or Ellipse. **Move part** drags the selected part. White cross = placement pivot; amber circles = weld anchors. Other parts are dimmed so the selected part is clear.
4. Pick parent and child parts and click **Click weld anchor on grid**, then click the anchor position. Set the break load in kN; 0 means an unbreakable attachment. Existing weld loads can be adjusted individually. **Weld all nonempty parts** creates a simple assembly automatically.
5. Use **Drop**, **Impact wall**, **Burn**, **Step**, **Run / pause** and **Heat view**. Adjust impact speed as needed. Clicking the preview applies a small heat torch. Tests always use a copy; **Reset preview** rebuilds it from the draft.
6. Click **Save asset**, then **Place at scene centre** to add all parts and welds to the scene. They arrive selected, so drag the assembly into position. One scene Undo removes the whole placement.

Each cell is 10 cm. Grid dimensions can be 1–128 cells each, with up to 256 named parts. Thickness affects physical mass. Resize crops material outside the new grid; Undo restores it. Draft Undo/Redo is separate from scene history. Leaving the workshop retains its in-memory draft, but save before closing the application.

The preview uses actual engine cells, gravity, material heat/burning, contact damage and breakable joints. Wood may survive an impact that shatters glass: survival is a valid test result, not a canned break animation. Gunpowder in the sample really detonates. The impact test advances a few settling frames before launch so a newly spawned body's impact-grace period does not hide the result.

The checks panel highlights empty parts, overlapping cells, disconnected islands and an entirely unjoined multi-part prop. These are authoring diagnostics, not proof of structural stability; use the test bay. Painting into another part's cell transfers ownership, so the physical bodies do not overlap. Fill treats other occupied parts as boundaries.

## Paint colours and lettering

**Material** chooses what a cell physically is. **Surface paint** changes its visible colour while keeping that material, temperature, density, strength and burning/explosive rules. Click the colour swatch for the RGB/HSV colour picker, or use the Red, Black and White shortcuts. Pencil, Fill, Line, Rectangle and Ellipse all work with surface paint. In this mode Erase removes only the paint. Fill stops at material and existing paint boundaries and only affects the selected part.

For a TNT crate:

1. Draw the case and its middle plank using wood in Material mode.
2. Select Surface paint, Red and Rectangle to paint the middle plank.
3. Choose Black, enter `TNT`, select Stamp lettering, and click its top-left corner on the plank. Letter size is in cells; capitals, digits, spaces, hyphens and exclamation marks are supported. Letters only paint cells that already exist in the selected part.
4. Add a separate named part for gunpowder and draw it into the enclosed space. Weld it to the case if it should move together. These are real material cells, not an explosive-looking colour.
5. Save, inspect the preview, then use Burn to test a copy. Open the supplied `tnt_box` example to see the arrangement.

This is a 2D material cross-section: two parts cannot occupy the same cell. The example has two gunpowder pockets above and below a solid painted wooden middle plank. It is not a second hidden layer behind the writing.

Surface paint follows moving bodies, fragments, flying chips, copied cells and Undo/Redo. Heat and burning still affect the surface appearance. When the underlying material turns into another substance (for example wood into ash), its paint is removed. Paint is saved in both assets and placed scene copies.

## Save and AI collaboration

Assets live in `assets/workshop/*.asset`. Both the workshop and AI tools read the same versioned text format. **Reload from disk** reads edits made outside the editor. Loading another file or starting a new asset is undoable, allowing recovery of the previous draft during the session. Save/reload is explicit; there is no automatic merge of concurrent edits.

Part names and weld references persist in the asset file. The scene editor places ordinary body-and-joint copies; changing the source asset does not automatically update previously placed or damaged copies. Saving that scene bakes the copies into normal scene bodies and joints. The source `.asset` remains independently editable with its original part names.

## Text format for AI tools

A concise asset can use named shape commands; saving in the workshop emits an equivalent material legend and exact grid for each part. Names must be unique within the asset and use letters/numbers/`-`/`_`.

```text
format 1
asset fuel_crate 48 40
depth 0.04
pivot 24 20
part frame
rect wood 9 10 30 3
rect wood 9 13 3 20
rect wood 36 13 3 20
rect wood 12 30 24 3
part base
rect iron 7 33 34 3
part tank
rect gunpowder 16 17 16 10
weld frame base 24 33 50
weld frame tank 24 23 25
```

Commands:

- `asset NAME WIDTH HEIGHT`: shared local grid. `depth METRES` and `pivot X Y` set thickness and placement origin.
- `part NAME`: select/create a named component.
- `rect MATERIAL X Y WIDTH HEIGHT`, `line MATERIAL X1 Y1 X2 Y2`, `circle MATERIAL CX CY RADIUS`, `fill MATERIAL`: draw into the current part. `air` erases cells in that part. New non-air paint takes ownership of its cells from other parts.
- `legend CHARACTER MATERIAL`, then `grid`, exactly HEIGHT rows of WIDTH characters, then `end`: an exact part bitmap. Dot is empty. Overlapping explicit part grids are rejected.
- `paint X Y #RRGGBB`: paint an occupied cell of the current part without changing its physical material. Exact black (`#000000`) is supported. This is optional in format 1, so older unpainted assets still load. Scene files use `bodypaint X Y #RRGGBB` for the preceding body, or `paint X Y #RRGGBB` for scenery.
- `weld PARENT CHILD X Y BREAK_KN`: an attachment at a grid-space anchor; 0 = no break limit.

AI workflow: load the file; preserve unrelated named parts; edit shapes or cell grids; reload the workshop; inspect diagnostics and preview; run a destructive test. For a headless preview, put the asset in a scene and use the engine's existing screenshot command. The parser reports invalid materials, duplicate names, bad grid dimensions and missing joint references, and bounds the input size.

Scenes can load assemblies directly:

```text
asset ../assets/workshop/fuel_crate.asset 115 85 as crate
```

Coordinates place the asset's pivot. `as crate` exposes names such as `crate.frame` and `crate.base` for later scene joints. Multiple assemblies get correctly offset joint references. See `scenes/asset-workshop.txt`. Reloading a source scene containing these commands rereads the asset; saving from the editor expands the placed result as described above.

## Verification

94 targeted checks passed: 25 editor/workshop checks and 69 integration checks across assets, editor history, imports, joints, physics, input robustness, templates and triggers. The new coverage includes real grid strokes and their undo, new-asset editing, isolated burning tests, undoable assembly placement, save/load, part renaming, multiple scene assemblies, actual explosive ignition, brittle-glass impact fracture, sample weld detachment and pivot-safe preview placement.

The two sample assemblies also ran headless for 300 frames with no load problems, retaining their six bodies. Inspected the resulting scene, destructive-test screenshot and live workshop layout. This is an initial workshop: it provides welds rather than every joint type, whole-part movement rather than arbitrary region copy/paste, and explicit source reload rather than live instance synchronisation. It does not contain a hosted AI service; assistants can edit the same local files without one.

Surface-paint update: the full release test suite passed, including seven new paint checks and the colour/lettering controls test. Coverage includes material invariance, paint boundaries, asset/scene saving, moving bodies, fracture, flying chips, clipboard, history, ignition and a real TNT detonation.
