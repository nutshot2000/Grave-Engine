# Thermal and material update

The heat calculation now uses a shared, conservative exchange across each active cell boundary. Small stable substeps handle low-capacity materials without clamping away their intended heating rate. Edges beside sleeping regions either update both cells or transfer no heat for that frame.

This preserves relative capacity-weighted heat in ordinary closed cells. Burning, fixed-temperature sources and ambient cooling remain intentional heat sources/sinks. Heat capacity remains a relative per-cell control; this does not add physical SI units, latent heat, pressure or density-based heat capacity.

| Preset | Implemented change |
| --- | --- |
| Concrete | Starts weakening at 300 C. Cold strength, toughness, density and melting point are retained. At 900 C its strength/toughness multiplier is 0.25. It remains nonflammable. |
| Snow | Melt yield stays at 0.12 to preserve the intentional reduction in meltwater. Density changes from 300 to 120, describing light fresh snow that matches that yield. Grip, rolling resistance and pile behavior settings are retained. |
| Sand | At 1700 C, silica sand becomes molten glass. Cooling below the molten-glass threshold makes glass, rather than gravel. |

## Regression coverage

- Closed two-cell and mixed-grid heat conservation, including capacities below one.
- Convergence to the capacity-weighted equilibrium without temperature overshoot.
- Sleeping-block boundaries and cross-plane heat exchange.
- Cold concrete overhang remains supported; heated overhang breaks loose.
- Snow produces approximately 12% meltwater with a matching density ratio.
- Sand melts and cools through the glass materials.
- Existing fire, layered buildings, weather, water, metals, vehicles and game tests.

Existing saved scenes and workshop assets are preserved. New material defaults apply unless a personal material override replaces them.

Validation completed: 267 tests passed across 37 targets, including both complete stunt-bike courses and the shooter. In a visual street burn, powder detonated and fire reached the interiors. A 2,400-cell snowbank produced 297 water cells (12.4%); the cold concrete beam held while the heated beam collapsed.

The gantry regression now checks actual body contact rather than requiring one particular unrecoverable post-impact trajectory. Game rider rules and course geometry were retained.
