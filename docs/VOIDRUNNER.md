# Voidrunner — Operation Cinder

A new horizontal scrolling shooter built on Grave Engine. Open Play-Voidrunner.cmd in the engine folder.

Controls: Enter launches; WASD or arrows move; Space/Z fires; X spends a pulse bomb; P/Escape pauses; R restarts. Movement speed stays constant while firing. Collect green + pickups for shields and gold W pickups for a wider weapon spread. Three lives and three bombs per run.

The authored convoy begins with scouts, then adds weaving interceptors, explosive fuel tankers, asteroids and spread-firing frigates. The refinery backdrop leads into the reactor-carrier arena after 80 seconds. Its blue shield cycles on and off. Shoot its glass ports while the shield is down. Destroy surviving marked reactor fragments as well as the hull to finish the mission.

Ships are real aluminium, steel, iron, glass, copper, plastic, oil and gunpowder cells. Shots sweep their paths to hit thin shapes, heat their impact point and cut a small patch of armour. Heating the gunpowder invokes the engine's actual explosion, heat, fire and fragmentation simulation. Weapon damage is currently a game-specific carve/heat approximation rather than a calibrated toughness/penetration model. The native material textures remain visible. Stars, distant refinery silhouettes, planet, engine plumes, projectiles, pickups and HUD are presentation layers.

The scroll is a fixed combat window with advancing waves and moving enemies against parallax scenery; this is not a persistent kilometre-long world. The player uses a rotation-locked kinematic controller, so collecting pickups and contacting wreckage cannot deflect the controls. Shield damage and lives are game rules; enemy bodies and their explosive contents remain physical. Debris is budgeted and removed at the playfield boundaries. The carrier enters a cleaned arena to avoid spawning inside old hot wrecks.

Level timings and enemy types: assets/voidrunner/level.txt. Gameplay source: src/bin/voidrunner.rs. No third-party art or asset pack was added. No sound/music yet.

Verification: seven focused tests pass: real fuel explosions, swept projectile hits, varied waves/final boss, life/bomb limits, stable controls after weapon pickup, stable boss entrance, and no victory while a reactor survives. Three library tests pass. The --verify route uses an automatic aiming pilot with damage immunity: it checks wave progression and reachable victory, not ordinary-player survival or balanced difficulty. See the separate engine-improvement file for limitations and proposed future work.
