# Physical depth layers

Open `scenes/layered-shop-night.txt` for the night street. Press Play and use the Heat brush on the green wooden shopfront or striped awning. Reset restores the whole building, including the interior. Street lamps, warm sign lighting and separately lit interior furnishings make the reveal visible at night.

The reusable asset is `assets/workshop/layered_shop.asset`: front elevation, interior frame/shelves/stock/counter/upstairs furniture, and a plaster-coloured masonry back wall. It has 23 named parts across three aligned planes. The original single-plane `shop_front` remains available.

## Authoring

The workshop's **Depth layers** selector chooses a plane. **Add depth layer** creates a plane with an empty named part. New parts belong to the selected plane. Draw and surface-paint normally: ownership transfers only between parts on that plane. **Ghost other layers on grid** provides alignment guides. The preview can show the complete outside view or isolate a plane. Click heating targets the displayed plane; the whole simulation continues to run and exchange heat.

Assets support four planes total, ordered front to back, sharing one pivot and one 128 × 128 maximum grid. The 24-part limit applies to the entire asset. The optional `layer NAME` command selects the plane for subsequent `part` commands. `front` is plane zero; further names are allocated in order. Older assets default to front. Part names remain unique across the asset. Material and paint cells can overlap across planes, but cannot overlap within a plane. Welds join parts within the same plane.

Scenes can place the layered asset with the usual `asset FILE X Y as PREFIX` command. Additional planes inherit existing fixed terrain when first created; front rigid bodies are never copied behind themselves. Saved scenes bake each plane into a `plane INDEX NAME` / `endplane` block containing a normal scene of matching dimensions. An explicit plane block can provide its own terrain and lighting. This is used for the night shop's warm interior. Planes cannot be nested in the file. Scene save/load, Reset, placement Undo/Redo and rewind retain every plane.

## Simulation and rendering

Each plane runs the same material and rigid-body simulation. Adjacent planes exchange heat at the same world cell using both materials' conductivity and heat capacity. Exchange is bidirectional and conserves thermal energy before normal cooling and material reactions. Fire in one plane can ignite timber or stock in another through this heat transfer. There is no rule that automatically ignites every plane. Insulating materials impede transfer; a missing front wall reveals the actual current interior, including its own burning or damage.

Rendering composites planes nearest first. Opaque cells hide the interior; glass and liquids transmit some of the view, while smoke/fire blend over it. Flying chips and emissive halos remain visible. All callers using `Renderer::light(&sim)` receive the layered view, including the editor, workshop, games and headless screenshots. For independent low-level worlds, `Renderer::draw` still draws just the supplied world.

The first version uses independent collision planes: objects do not collide with objects behind them. Welds, explosions, actors and triggers are local to a plane; heat is the cross-plane interaction implemented here. This avoids flattened collision shapes and permits aligned overlapping walls and shelves. Moving a placed front assembly in the scene editor currently moves its front bodies; edit/reposition the complete asset in the workshop or place it again to keep depth planes aligned. Main-scene editing targets the front plane; use the workshop to edit interior assets.

Rewind stores additional-plane snapshots and includes them in its memory bound. Layered Undo uses complete snapshots. Large layered worlds cost more CPU and memory than a single grid.

## Lighting

Point lights support RGB colours, range, strength, flicker and attachment to a named object. Cones provide spotlights/headlights; sunlight provides directional light and shadows. Materials can glow without heating or emitting fire: `lamp_glass` is a physical glass material with a warm electric glow. The new optional `unshadowed` light flag projects light across a facade without being blocked by the drawing plane. Other lights continue to cast shadows by default. The night shop's sign lights follow its breakable sign; street lights use normal shadowed cones. An interior plane can have its own lighting, while a plane with default lighting inherits the scene lighting.

## Verification

The complete release suite passed after the depth-system changes. Further focused checks passed after night lighting was added: editor controls, material occlusion and crop equivalence, independent overlapping parts, old assets, bidirectional energy-conserving heat, insulating material, cross-plane ignition, asset/scene roundtrips, reset/history/rewind, complete shop fire, per-plane lighting and sign-lamp persistence. The night frontage was also run for 360 frames: all 16 exterior bodies remained intact and stationary before ignition. Its masonry feet are secured to the pavement with breakable scene welds; timber and fabric can still burn away independently.

## Timber facade and exposed air

For the burn-through demonstration, open `scenes/timber-shop-night.txt` and the reusable `timber_shop` asset. This version has cream timber weatherboards instead of brick on the outside; the rear masonry plane remains nonflammable. The original brick shop is still available.

Layered simulations account for air touching the outer faces. The front and rear outside faces receive air, while enclosed middle-plane cells get depth air only from a neighbouring material that feeds fire. Opening a hole can therefore supply air to hot wood behind it. Temperature and material ignition thresholds still govern burning; brick is never made flammable. Standalone single-plane simulations retain their existing air requirement. Tested intact standing, substantial timber loss, interior ignition, exposed faces and sealed middle-plane wood.

Green weld/hinge/axle guides are shown only with the Join tool. Rope lines remain visible as the physical representation of ropes. Normal painting, heating and play no longer draw green squares over buildings.
