# Destruction performance update

Cinder Lane exposed a performance problem when explosions created thousands of debris bodies across its depth planes. The editor also compounded slow frames by running up to eight catch-up physics steps before drawing or accepting more input.

## Changes

- Body rasterization now uses a cached occupied rectangle transformed into screen space. Sparse parts no longer scan a large square around the entire asset grid.
- Physical assembly sizes are counted once per update, instead of rescanning every body for every collider. Changes to welded connections still take effect immediately.
- Contact damage skips the cell-owner map when there are no contacts hard enough to cause damage.
- Lamp attenuation is calculated once per distance step and reused across its rays.
- After heat/air exchange, independent depth planes simulate on separate CPU cores. Plane rendering uses separate cores too. A single-core machine retains the sequential path.
- Layer images are composited directly from their retained buffers, avoiding temporary image copies.
- Glow blur traverses rows for contiguous memory access, preserving the old floating-point operation order.
- Rewind takes ownership of layer snapshots rather than cloning them again. Memory eviction releases one recorded moment at a time, promoting the next to a keyframe, instead of periodically freeing forty layered snapshots together.
- The editor limits catch-up work by both step count and elapsed time. Heavy destruction can still reduce simulation speed, but a backlog cannot monopolize mouse/keyboard handling for six or eight extra physics steps.

## Measured on the development machine

A deterministic Cinder Lane run: 1,080 simulation steps, first-alley ignition, rendering every second step, rewind enabled throughout. Both runs ended with the same 3,011 total bodies across all planes.

| Measurement | Before | After |
| --- | ---: | ---: |
| Complete benchmark | 53.95 s | 25.45 s |
| Heavy-fire simulation step, last 180 steps | 40.80 ms | 15.50 ms |
| Layered image, same interval | 28.85 ms | 17.01 ms |
| Worst rewind record, same interval | 112.58 ms | 8.09 ms |

These are benchmark timings, not a promise of 60 FPS in the editor. A fully burning street with thousands of interacting fragments remains substantial CPU work. The scene, explosive contents, body limits and fixed simulation timestep were retained.

Run `cargo run --release --example profile_cinder_lane` to repeat the scenario and inspect timing by subsystem. Timing collection is disabled during ordinary engine use.

## Regression coverage

The parallel fire simulation is compared with the serial path for exact materials, temperatures, flags, flying cells, body IDs and body poses. Parallel rendering is compared pixel-for-pixel with serial rendering in normal and heat views. The blur is checked against the prior strided algorithm. Rewind is checked after forced memory eviction, including the retained interior plane. A delayed editor frame is checked for a bounded physics backlog.
