# Grave Engine — verdict and development feedback

The earlier sections record the build history. The latest tested verdict is the 11 November 2026 update at the end; it supersedes the earlier recommendations where features have since shipped. For current game controls, see WRECK-RUN.md.

## First playable prototype

Open Play-Stunt-Yard.cmd in the Grave-Engine folder.
SPACE starts. D/right accelerates, A/left reverses, Q/E leans, R retries.
One course contains a fire jump, a water landing and a lightweight breakaway obstacle.
The rider art and editable Blender model are assets/stunt-rider.png and assets/stunt-rider.blend.

## What this build taught us about the engine

1. Vehicle control needs a reusable controller. The first bike could not climb the ramp; increasing motor power made it flip. The prototype now uses assisted angular balance and manual lean. Added Physics::steer_rotation so this can act without rebuilding the body and its joints. Next: braking, traction, rider weight transfer and adjustable assist.
2. Rider attachment is missing. The rider uses welded physical pieces plus a Blender picture attached to the chassis. It does not yet grip handlebars, animate with suspension or detach in crashes. A mount/dismount and ragdoll system would help many games.
3. Objectives need data. Gates, scoring and finish conditions currently live in the standalone game. Make trigger zones and objectives scene data next. Current gates award points for passing x positions; they do not establish a safe landing or a genuine wall smash.
4. Material strength needs a gameplay layer. A normal wooden wall is a poor movie prop: the bike can bounce back instead of producing the desired stunt. The current obstacle is a thin loose plank. Add authored breakaway presets with measurable impact thresholds.
5. Terrain authoring needs ramps and curves. This course needs many narrow rectangles to describe slopes. A polygon or ramp command and smooth collision preview would speed level design.
6. Water edges need attention. A deep rectangular pool trapped the bike after a bounce; the prototype uses a shallow pool. Add sloping exits and test wheel contacts at liquid/terrain boundaries.
7. Rendering needs independent presentation layers. The prototype overlays rider art separately from the body simulation. Background scenery, signs, motorcycle artwork and consistent object/sprite attachment would improve presentation.
8. Sound and crash feedback are high-value next steps: engine revs, tyre contact, landing impact, fire and a clear reason for each failed take.

## Verification and limits

All 107 pre-existing tests passed. A new full-course test passes all three gates and checks that retry resets score, time and gates. An automated full-throttle run completed with 1974 points. Inspected the world screenshot and live game window. Human play feel, trick judging and long-session stability still need testing. No sound, persistent best score, rider animation or ragdoll yet. This is one course with three obstacles, not three separate levels.

The original editor files had existing changes and were left alone. Engine addition is the angular-control method; the game is src/bin/stunt-yard.rs and scenes/stunt-yard.txt.

## Update: physical motorcycle implemented

The ten motorcycle assemblies now contain engine rubber, iron and brass cells. Added breakable joints, body thickness, internal collision filtering for welded assemblies and coherent assembly balance. Tests cover tyre ignition, crash detachment, save/load, mass and fragmented parts. All 112 tests passed. C in the game runs a real wall-impact demonstration. The rider remains a visual character and still needs its own physical body/ragdoll.

## Wreck Run: lessons from a varied material course

The previous repeated ramps did not create interesting riding decisions. The new course uses terrain-dependent resistance, brakes, sand hills, a mud crossing, an overhead clearance hazard, and one-shot heat cues that ignite existing fuel/explosives. Checkpoints make retries practical. See WRECK-RUN.md for current controls.

Further engine improvements exposed by the build:

- Vehicle terrain resistance deserves a reusable material property and contact-based wheel model. The game currently samples cells near the tyres and applies assembly drag. It works as a prototype but is not rolling deformation or a full traction model.
- Low-speed motor torque must be tested on uphill starts after mud, not only on ramps approached at speed. Increased this bike's motor strength from 2 to 3.
- Overhead terrain can collapse under its own material rules. The low gantry is now an anchored rigid iron body. An explicit static-scene fixture tool would make this easier to author.
- Water spreads and changes the approach to later obstacles. Pools need contained fills, rideable exits and a recovery stretch; their first-frame appearance is insufficient validation.
- Add a general scene trigger/checkpoint system. This game's cue file only supports crossing an x position and applying heat once; it is not yet an engine-wide scripting system.


## Engine update, 11 November 2026: Vehicles and Level Tools

Done in the engine (each with tests; the game now uses them):

1. **Vehicle contact and traction** (physics/vehicle.rs): brakes hold the drive wheels through their motors, so stopping comes from real tyre grip (hard on stone, a slide on ice, nothing in the air). `rolling_resistance` is a material key (sand 0.6, mud 1.4, water 0.3, gravel 0.3...), read from the cells the drive wheels touch. `upright NAME strength S` and `set_lean` replace the game's hand-written balance. The game's surface table, drag-based brakes and manual steering are gone.
2. **Level-building toolkit**: `slope`, `hill`, `curve`, `poly` with `cover MAT DEPTH`, and `anchor` for pinned scenery. scenes/stunt-yard.txt went from 3,554 lines to 326.
3. **Triggers and checkpoints** (trigger.rs): `trigger X Y W H by frame : heat circle ... ; message ... ; checkpoint X Y ; score N`. The four fuses and four checkpoints are scene data now; scenes/stunt-yard-events.txt is gone.

Still to do from the list below: a physical rider and ragdoll (4), sound (5), active simulation regions (6), tuning scenes and an inspector for contact forces (7). Stage gates and scoring still live in the game (they could become `score` triggers once a jump can be judged on landing).

## Current verdict — 10 November 2026

Grave Engine is a promising material-simulation engine with a distinctive strength: objects and terrain participate in the same physical world. Rubber tyres can burn, metal parts have mass, joints can fail, wood can collapse, and gunpowder can damage nearby structures. These interactions make it more interesting than a simple driving-game framework.

Building Wreck Run proved that the foundation can support a playable scrolling game, not just isolated sandbox demonstrations. However, I would currently describe it as a strong experimental engine, not yet a mature general-purpose game engine. Too much of the work required to turn the simulation into a fair, enjoyable level still lives in custom game code and repeated tuning.

The most important lesson was that realistic interaction does not automatically create good gameplay. A hill that works with a fast approach may be impossible after mud. Water can migrate onto the next ramp. An overhead structure can collapse before the rider arrives. A beautifully rendered bike can hide the materials actually being simulated. The engine needs better tools to expose these problems while authoring, and reusable controls that make their behaviour predictable.

### What I would keep and build on

- The shared material system: burning, melting, reactions, fluids, loose powders and rigid objects belong together.
- Plain-text scenes and materials: easy to inspect, generate, version and automate.
- Destructible assemblies: body thickness and breakable joints let a vehicle behave as separate physical parts.
- Deterministic simulation, snapshots and headless tests: particularly useful for reproducing a bad landing or comparing changes.

### My priorities for the next update

1. **Proper vehicle contact and traction.** Add reusable wheel controls with motor torque, braking, rolling resistance, slip and adjustable balance assistance. Put surface resistance in material data. Our sand/mud slowdown currently samples cells near the tyres and applies drag to the bike; it is a useful game approximation, not a complete tyre model. Acceptance check: one vehicle can start on a slope, recover after mud, brake on stone, slide on ice and remain unaffected by ground braking while airborne.

2. **A level-building toolkit.** Add ramps, curves, filled polygons, material layers and clearly anchored fixtures. Include collision previews and a settle/preview mode. The current course generator needs thousands of small rectangles, and an innocent-looking pool exit can stop the bike completely. Acceptance check: build and adjust a hill, water crossing and overhead obstacle in the editor without writing a generator or repeatedly rebuilding the game.

3. **General triggers, checkpoints and objectives.** Author proximity/impact triggers, delays, one-shot events, ignition, spawning and checkpoints in scene data. Preserve their state in saves and resets. Our ignition cue file is a small game-specific implementation, not a general engine system. Score a jump after a successful landing, rather than merely crossing an invisible line. Acceptance check: create a demolition sequence and restart it reliably from a checkpoint without changing Rust code.

4. **A physical rider and reusable attachments.** Add mount points, rider collision shapes, weight transfer, dismounting and ragdolls. The current rider is artwork with an added helmet-contact crash check; the whole body is not physically simulated. Acceptance check: the rider hits an overhead beam, separates from the bike and reacts to the same world as the wreckage.

5. **Clear presentation and sound.** Keep material identity visible while supporting attached artwork, damage masks and independent background layers. Add material-based impact, rolling, engine, fire and explosion sounds. Give players a clear reason for a crash. These would add more perceived polish than another large batch of materials alone.

6. **Performance tools and active simulation regions.** Profile the long course, show time spent on cells, terrain rebuilding, physics and rendering, then optimise the measured bottlenecks. The game currently updates and uploads the whole world even though only a small section is visible. Sleeping or chunked simulation must preserve important offscreen events and deterministic behaviour. We have not established a low-end hardware performance target yet.

7. **Reusable tuning and regression scenes.** Ship small examples for hill starts, ice braking, deep sand, pool exits, impacts, joint failure and blast damage. Add an inspector for contact forces, joint loads and material changes. This would make engine updates much safer than discovering a regression halfway through a large level.

### What I would ship first

A focused “Vehicles and Level Tools” update: traction/braking, surface resistance, ramps and curves, anchored scenery, and simple triggers/checkpoints. Then rider physics and sound. Those improvements would reduce the amount of custom code needed for the next game and directly improve how this one plays.

### Evidence and limits of this verdict

The eight focused Wreck Run checks passed together: controlled completion of the course, failure at the low gantry when holding full throttle, material slowdown/braking, actual gunpowder detonation, checkpoint recovery, tyre ignition, crash detachment and camera scrolling. The joint/physics checks also passed, including preservation of attachments and vertical movement when applying horizontal drag.

The controlled completion is automated and uses authored braking inputs; it is not evidence that the difficulty is balanced for a human player. The latest broader test run was started, but its final result was not collected before opening the game at your request. The earlier full-suite results above are historical. Long-session stability, sound, rider ragdolls, persistent scores and a general event system remain unfinished or unverified.

My overall recommendation: keep developing it. The material interactions are the reason to use this engine. The next step is to make those interactions easier to author, understand and control, so building the next interesting level takes hours rather than repeated physics troubleshooting.


## Tested update verdict — 11 November 2026

Reviewed commit 114da89 with the current working copy, including the existing local changes to the Wreck Run gantry and its generator. Those changes were preserved. No engine or level fixes were made as part of this review.

### My new verdict

This is a substantial improvement: Grave Engine now has reusable systems for the kind of material-driven games it is meant to support. Vehicle controls, material resistance, terrain shapes, anchors, triggers, camera effects, lighting, backgrounds and simulation sleeping address much of my previous highest-priority feedback. These are implemented systems with useful automated coverage, rather than just items on a roadmap.

I would now call it a capable experimental engine for small 2D physics games. Its strongest selling point remains that materials, vehicles, destruction, heat and fluids interact in the same world. The next update should prioritise reliability, visual authoring and readable feedback over adding another large batch of materials. I would not yet describe it as production-proven: long-session behaviour, human playability and predictable performance on demanding levels still need work.

### What I actually tested

- Release build succeeded.
- The standard full test run produced 173 passing tests and one failure after also running the integration suites separately: 3 library tests, 19 editor tests, 10 passing game tests, and all 141 integration tests. The failing game test stops Cargo's normal run before the integration tests, so I explicitly ran all 22 integration-test files afterward; all passed. Counts exclude the extra diagnostic probes described below.
- All 19 example scenes loaded and simulated for 300 frames each without reported scene errors. This is a smoke check, not proof that each scene stays stable indefinitely.
- The passing checks cover braking on stone versus ice, no ground braking in the air, sand/mud resistance, tyre spray, trials balance, joints, softening and bending metal, materials, embers, triggers, save/restore, terrain shapes, lighting, backgrounds, camera effects and sleeping-area interactions.
- Mountain Run's complete automatic-rider test passed. This verifies a scripted rider can finish; it does not establish human difficulty or enjoyment.
- Inspected the generated house-night, Mountain Run and bonfire screenshots. The night house has readable local lamps against the layered sky and mountains; the fire scene shows a substantial spray of glowing particles. Lighting and backgrounds now provide a much stronger sense of place.

### Findings that matter now

**1. The current Wreck Run level edit breaks its known completion route.** The controlled-rider test failed twice at approximately x=1026.56 cells (103 m), with almost no forward speed. The local scene lowers the iron gantry from y=166 to y=179; its generator has the matching change. I ran the same completion test against a separate copy of the committed scene, without changing your working scene: that version passed. This isolates the failure to the changed level configuration, rather than demonstrating a general vehicle-engine regression. It does not prove that no human can pass the edited version. Next action: check rider/bike clearance and adjust the obstacle or establish a new repeatable safe route before accepting that edit.

**2. Thin triggers can miss fast crossings.** A separate runtime probe moved an iron body from centre (40,41) to approximately (60,41) in one simulation step, across a trigger covering x=45..46. The trigger did not fire. Its speed was 1200 cells/s (120 m/s), representing a fast projectile or blast-thrown object. Current detection tests whether the centre is inside a box at a sampled position. Next action: test the swept path from the previous centre to the new one, with defined rules for teleports and repeat triggers. Also cover ordinary vehicle speeds with thinner boxes, overlapping bodies and actors. A finish/checkpoint should not depend on accidentally sampling the correct frame.

**3. Performance is better supported, but the largest active scene remains expensive.** The scene sweep measured about 0.43–3.30 ms per simulation frame for the smaller examples, 8.31 ms for Wreck Run and 43.37 ms for Mountain Run over the first 300 frames. A separate 1200-frame Mountain Run run averaged 18.27 ms per simulation frame. The latter overlapped a course-test process, so these are indicative measurements on this machine, not controlled benchmarks. The timer covers simulation before the final screenshot, not complete rendering/UI cost, and I have not established steady-state frame rate. Sleeping and window rendering are meaningful improvements, but the data does not justify a blanket claim of 60 fps on large active levels. Next action: profile startup, active fire/water, terrain-collider work, settled simulation and the visible render separately; report median and slow frames, not only an average.

**4. The checking script should trust process exit codes.** tools/check.ps1 determines test success by searching captured text for failure phrases. A compiler failure without those phrases can be labelled as successful. It also loses the detailed assertion output in its displayed summary. Next action: record Cargo's exit code immediately, retain the full log, and run independent test targets even if a game test fails. This review did encounter a real failing game assertion, which the script correctly reported.

### What I would improve next, in order

1. **Reliable triggers and a dependable release gate.** Fix swept crossings; retain regressions for actors and objects; repair the current level-clearance regression; make the check script fail accurately and report every suite. These protect existing work before expanding it.
2. **Visual level and event authoring.** The new slope/hill/poly/anchor commands are a major improvement, but the editor still needs direct handles for these shapes and trigger boxes, an action inspector, an explicit anchor overlay and collision/rider-clearance previews. Add a settle preview and a button that runs the course checks. The gantry failure is a good example of what these tools should catch early.
3. **Contact, damage and performance diagnostics.** Show wheel contact, slip, resistance, motor effort, joint load and bending strain. Add a frame-cost display separating world updates, physics, terrain rebuilds, lighting and drawing. Use a repeatable benchmark with fixed scene, seed, viewport and active effects before optimising further.
4. **A physical rider and reusable attachment system.** This remains unfinished. The current rider is drawn over the bike and has a helmet-contact crash check, not a fully physical body. Mount points, body collisions, weight transfer, dismounting and ragdolls would make crashes much more convincing and help other games too.
5. **Sound and impact feedback.** Engine pitch, tyre slip, gravel/sand contact, landing thuds, metal bending, joint failure, fire and explosions would make the existing simulation easier to understand. Scale effects from actual speed, force and material; avoid a loud effect for every individual cell.
6. **Objectives and persistence beyond passage gates.** Judge a jump after a safe landing; expose failed-stunt reasons; support persistent settings/best scores and versioned save data. Test checkpoint recovery after destruction, changing lighting and timed camera effects, not just position and score. A rewind snapshot and a durable saved game need distinct, documented guarantees.

### What should come off the old wish list

Do not spend the next update reimplementing basic material resistance, brakes, terrain shapes, anchors, scene triggers, camera effects, lighting or backgrounds: they are already here and their relevant tests passed. Likewise, quiet-area sleeping and visible-window rendering are delivered foundations. Their next step is broader stress testing and better tools, not another claim that they still need to be invented.

### Recommendation for the next release

Ship a focused reliability-and-authoring update: robust trigger crossings, validated obstacle clearances, visual trigger/shape editing, trustworthy check results and performance diagnostics. Then add rider physics and sound. That would turn the new capabilities into a smoother process for building reliable, enjoyable levels.

These results do not certify a full human playthrough, hours-long stability, all hardware, all user settings or every possible combination of materials. The failure in the edited Wreck Run scene is recorded, not silently fixed or hidden. The new engine systems are a clear advance; the next milestone is making their interactions easier to inspect and consistently dependable.
