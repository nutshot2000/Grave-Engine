# Outside temperature

Open **Weather** to set **Outside temperature** in degrees Celsius. The default is 20°C, so falling snow can melt on warm air and ground.

Changing this value changes the environment's thermal target on every depth plane. Existing warm surfaces cool gradually. **Cold start (-8°C)** immediately chills ordinary air, ground and props for a snow test. **Match scene temperature** does the same using your chosen value. These actions pause the test and are undoable. Fires, fixed-temperature heaters, deliberately hot/cold materials and very hot cells are preserved.

The setting saves with the scene and survives Reset and rewind. Scene text accepts `outside_temperature -8` (finite values from -100 to 100°C); this initializes ordinary scene cells cold when loaded. Explicit heat commands still take precedence. The older `ambient` command remains a target-only command for existing files.

Use `scenes/snowy-shop-night.txt` for the timber shop with snowfall and a -8°C cold start. Enable Snow separately in Weather when testing other scenes. Cold air alone does not instantly remove heat already stored in warm pavement; use Cold start or let it cool.

Verified cold-vs-warm snow accumulation, ground initialization, all depth planes, preserved hot sources/explicit heat, scene roundtrip, Undo/Redo, rewind and the Weather control. The night shop ran for 1,200 frames at -8°C with 1,697 snow cells (including flying flakes), snow visibly piled on its roof and pavement, and all 16 facade bodies intact.
