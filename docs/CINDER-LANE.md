# Cinder Lane

A fresh night-time street for destructive play. Open **Scenes... > cinder-lane**.

- Two furnished timber houses with separate exterior and interior layers.
- The Night Owl shop and Fuel Depot: burnable timber fronts, shelves and rooms behind them, with masonry at the back.
- Five warm street lamps, coloured shop lighting and a small bench plaza.
- Four wooden powder crates and three stacked steel powder barrels. All explosive contents use real engine gunpowder.
- Oil spills link several of the gaps to nearby timber. Nothing starts on fire automatically.

## Burn it

Press **Play**, choose **Heat** on the left, and hold the brush over a red TNT crate or the oil beside it. The first stack between the rose house and Night Owl is a good starting point. Hold Shift and use the mouse wheel to enlarge the brush if needed. Escape cancels any asset-placement preview first.

Press **Pause** to inspect damage. **Reset** (R) restores the street to the state before Play. Mouse wheel zooms the view in Select mode; middle-drag pans. Fit restores the full-street view.

All eight new prop/building types are also reusable assets in the Assets library. The street is saved separately from your other scenes.

## Validation

The authoring example's `--verify` option settles the street for 180 frames and checks that every front-plane asset retains its original part count. It then ignites only the first alley, checks real powder consumption, timber loss and fire reaching the interior planes, and writes standing/fire/aftermath images into `shots`. This damages a temporary simulation, not the saved street.
