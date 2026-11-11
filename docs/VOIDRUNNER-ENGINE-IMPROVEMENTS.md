# Engine improvements exposed by Voidrunner

11 November 2026. These are proposals for a later engine update, not claims that all have been implemented. The shooter is a second game built against the same material simulation; its source is src/bin/voidrunner.rs.

## 1. Runtime body modes and arcade controllers — highest priority

Observed: the initial player was a dynamic rigid body. Wreckage impulses could rotate it or wedge it while collecting pickups; movement also slowed while firing. User playtesting exposed this immediately.

Implemented for this build: Physics::set_velocity changes velocity without rebuilding bodies; Physics::set_kinematic selects velocity-controlled movement and can lock rotation. The shooter uses a fixed movement speed and a rotation lock. A regression test collects an upgrade, moves forward, fires and reverses while checking heading and displacement.

Later: make dynamic/kinematic/fixed mode, rotation locks and collision layers explicit body/scene properties. Persist them through snapshots, save/load, editing and fragmentation. The new calls are runtime controls; their mode is not yet a complete scene-serialisation feature. Add an arcade-controller preset with input acceleration, bounds, optional slide response and separate gameplay hitboxes.

Acceptance: a ship keeps its orientation through contact and pickups, can reverse immediately, and preserves its controller settings across save/restore. Ordinary dynamic vehicle behaviour remains unchanged.

## 2. Fragment ownership and gameplay entity identity — highest priority

Observed: a carrier hull could fracture and lose its original body ID while explosive reactors remained. Treating a missing hull body as a dead enemy caused premature victory. Detached fuel also needed to remain targetable and survive ordinary debris cleanup.

Game workaround: in the isolated boss arena, count surviving actual gunpowder cells/flyers, mark reactor remnants and wait for all reactors to disappear. The arena is cleared of prior convoy fuel so those counts are meaningful. This is deliberately specific to this game and is not a general entity-ownership solution.

Later: report destruction/split events with parent entity, original body and descendant IDs. Support assemblies with independently damageable reactors, engines, turrets and cargo. Reattach visuals and gameplay ownership after fragments split; allow an enemy to lose an engine without counting as a kill.

Acceptance: breaking a hull leaves surviving reactor children associated with the boss, and score/victory occurs once only after its authored critical components fail.

## 3. Reusable projectiles, raycasts and material damage

Observed: the engine has heat, blasts and impact fracture, but the game needed its own swept bullet path and local carve/heat damage. Fast bullets must not skip thin glass or armour. A hard-coded circular cut is not a physically calibrated penetration model.

Later: add segment/raycast queries that return body/entity, material, hit point and normal. Add a general damage/energy API with penetration, heat transfer, impulse, ricochet and material resistance. Make collision allegiance/layers explicit so friendly and enemy fire need less custom filtering. Preserve deterministic hit order.

Acceptance: a fast shot reliably hits a one-cell plate; glass, aluminium and steel behave differently at the same energy; an exposed fuel hit heats and detonates actual explosive cells rather than calling a scripted visual explosion.

## 4. Configurable gravity and open boundaries

Observed: ships can use existing balance settings with gravity scale zero, but loose powders, droplets, flying cells and wreckage still have terrestrial gravity. Objects and loose material can accumulate at the combat window's boundaries.

Game workaround: controlled ships ignore gravity; wrecks drift and fall out, and the game removes boundary debris. This is an arcade space presentation, not a fully zero-gravity simulation.

Later: one explicit world gravity setting, saved/restored and applied consistently to bodies, loose cells and flyers. Add closed/open/streaming boundary policies and offscreen lifetime rules.

Acceptance: zero-gravity fuel and fragments drift consistently, and an open playfield does not accumulate a pile along its bottom edge.

## 5. Debris and effect budgets

Observed: the first sustained combat run approached the engine's roughly 1500-object limit as old fragments accumulated. A bounded cleanup policy brought the observed count back into the low hundreds.

Game workaround: cap nonessential wrecks, drift fragments out and clean the lower boundary. Critical reactor fragments are retained. The carrier enters a fresh arena so it does not collide with old debris at spawn.

Later: configurable budgets for bodies, tiny fragments, flying cells, smoke and lights, with critical/noncritical flags and distance/age policies. Track allocation, collider rebuild and particle costs. Prefer controlled degradation to a sudden frame-rate collapse.

Acceptance: a sustained multi-minute battle has bounded cost while critical fuel tanks and boss parts are never silently culled as decorative debris.

## 6. Enemy, wave and encounter authoring

This game parses a small time/kind/altitude wave file. Enemy movement, aiming, shielding, pickups and boss rules still live in Rust.

Later: reusable entity templates, movement paths, weapon patterns, component weak points, encounter phases and spawn validation in scene data/editor tools. Validate that a spawn area is clear and a boss does not overlap old physical debris. Keep encounter reset semantics explicit.

Acceptance: build another shooter wave or a three-reactor boss without writing another game-specific manager.

## 7. Audio and combat readability

Later: material-driven shot impacts, glass breaks, metal damage, shield hits, fire and layered explosion sounds with voice limits. Add reusable warning markers, hit flashes, shield windows, damage indicators and critical-part highlights. Connect HUD state to critical components instead of ambiguous hull-only bars.

## Verification limits

Seven focused shooter checks and three library checks passed. An automatic pilot verifies progression with damage immunity; this does not establish fair difficulty for a normal player. Live visual inspection and the user's control/pickup feedback were used. Music, sound, a general projectile system, complete zero gravity and a general entity/fragment framework remain future work. The prior engine-review issues, including narrow-trigger crossings and the locally edited Wreck Run gantry, were not silently changed by this build.
