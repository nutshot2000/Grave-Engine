# Iron Heron oil field

Open **Play/Iron-Heron.cmd**, then press **Play**. The crank turns, its
counterweight swings, the pitman rocks the walking beam, and the bridle lifts
the polished rod. Up-strokes deliver oil through the pipe into the receiving
tank. The underground pocket visibly drains as the tank fills.

This is existing engine oil, not a painted level or unlimited emitter. It can
flow, spill, heat and burn. The sealed wellhead stops loose surface sand from
falling down the bore; a raised screened foot valve takes oil above the debris.
The pocket has a finite supply, with a little inaccessible oil below the intake.

The machine is saved as **assets/workshop/iron_heron_pumpjack.asset**. Its seven
named parts use steel, iron, brass, concrete, glass and plastic, with paint
separate from their material properties. The original oil pumpjack is unchanged.
Use the workshop to inspect the moving joints. The complete scene is
**scenes/iron-heron-oilfield.txt**; **tools/make_iron_heron.py** regenerates the
asset and the scene's surface paint.

## Reusable engine pumps

After placing/naming a machine, connect a moving point to two liquid ports:

```text
pump oil drive heron.polished_rod at 112 160 intake 103 267 16 7 outlet 288 146 4 3 rate 14
```

- `drive` is an existing named body. `at` is a world-space point attached to it.
- `intake` and `outlet` are world-space rectangles: x, y, width, height.
- `rate` is liquid cells delivered per cell of upward attachment travel.
- Any liquid material can be selected. Other materials and owned body cells
  are not transported. The ports must be separate and inside the world.
- No upward movement means no new stroke credit. Tiny solver tremors are
  ignored. A dry intake or blocked outlet discards unused stroke pressure;
  there is no unlimited backlog when the blockage clears.
- Cells move with their temperature, burning, lifetime, shade and other state.
  Pumps never replace scenery or create new oil. Removing the drive stops it.
- Snapshot/rewind preserves the drive attachment, stroke credit and delivery
  count. Scene saves preserve the ports, attachment, credit and count too.

The component models displacement through an enclosed pipe. It does not yet
simulate a pressure network, pipe friction or the effect of breaking the
drawn delivery pipe. Useful next engine additions are workshop port markers,
connectable hoses, pressure/back-pressure gauges and spill routing at a break.

## Verification

Regression checks cover real motor-driven transfer, finite supply, stationary
and missing drives, blocked outlets, cell-state conservation, snapshot and
scene roundtrips, invalid ports and continuous multi-cycle operation of this
machine. A 30-second run keeps all seven bodies, fills the tank and preserves
the total oil count. The final scene was inspected in headless screenshots and
delivered through the live editor.
