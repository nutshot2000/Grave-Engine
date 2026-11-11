# Voidrunner — Operation Cinder

A new horizontal scrolling shooter built on Grave Engine. Open Play-Voidrunner.cmd in the engine folder.

Controls: Enter launches; WASD or arrows move; Space/Z fires; X spends a pulse bomb; P/Escape pauses; R restarts. Movement speed stays constant while firing. Collect green + pickups for shields and gold W pickups for a wider weapon spread. Three lives and three bombs per run.

The authored convoy begins with scouts, then adds weaving interceptors, explosive fuel tankers, asteroids and spread-firing frigates. The refinery backdrop leads into the reactor-carrier arena after 80 seconds. Its blue shield cycles on and off. Shoot its glass ports while the shield is down. Destroy surviving marked reactor fragments as well as the hull to finish the mission.

Ships are real aluminium, steel, iron, glass, copper, plastic, oil and gunpowder cells. Shots sweep their paths to hit thin shapes, heat their impact point and cut a small patch of armour. Heating the gunpowder invokes the engine's actual explosion, heat, fire and fragmentation simulation. Weapon damage is currently a game-specific carve/heat approximation rather than a calibrated toughness/penetration model. The native material textures remain visible. Stars, distant refinery silhouettes, planet, engine plumes, projectiles, pickups and HUD are presentation layers.

The scroll is a fixed combat window with advancing waves and moving enemies against parallax scenery; this is not a persistent kilometre-long world. The player uses a rotation-locked kinematic controller, so collecting pickups and contacting wreckage cannot deflect the controls. Shield damage and lives are game rules; enemy bodies and their explosive contents remain physical. Debris is budgeted and removed at the playfield boundaries. The carrier enters a cleaned arena to avoid spawning inside old hot wrecks.

Level timings and enemy types: assets/voidrunner/level.txt. Gameplay source: src/bin/voidrunner.rs. No third-party art or asset pack was added. Original synthesised music and layered shot, impact, explosion, upgrade, charge-warning and dodge sounds are now included. M toggles sound; music pauses with the game or when focus is lost. Audio is optional if no output device is available.

Earlier verification: seven focused tests passed: real fuel explosions, swept projectile hits, varied waves/final boss, life/bomb limits, stable controls after weapon pickup, stable boss entrance, and no victory while a reactor survives. Three library tests pass. The --verify route uses an automatic aiming pilot with damage immunity: it checks wave progression and reachable victory, not ordinary-player survival or balanced difficulty. See the separate engine-improvement file for limitations and proposed future work.


## Combat and presentation update — 11 November 2026

- A slimmer original blue-ceramic fighter, a distinct red interceptor silhouette and additional muzzle flashes, sparks, upgrade notices and destruction feedback. Ship pixels still use engine materials rather than covering damage with an intact sprite.
- Shift gives a short invulnerable dodge boost with a visible recharge indicator. Movement remains rotation-locked and diagonal input stays normalised. The white centre dot is the small enemy-bullet hitbox: grazing a wing is safe. Enemy physical hull collisions still cost shields.
- Frigates and the carrier charge a thin red warning line before firing a heavy beam. Destroying its source during the charge cancels the attack. Boss attack density and sweeping spreads increase as reactors fail. Surviving reactor fragments continue attacking and remain marked targets.
- Consecutive kills build a capped chain bonus; taking damage breaks it. Pickups are attracted when nearby and show clear upgrade/healing messages.
- Original procedural synth music and six combat sound effects, mixed concurrently. Press M to mute. No downloaded music or sound pack is used.

Controls: Enter starts; WASD/arrows move; Space/Z fires; Shift dodges; X bombs; M sound; P/Escape pauses; R restarts. Dodging has a cooldown; holding Shift triggers again when ready. Three lives and three bombs remain the normal rules.

Validation: all 12 focused checks passed, including laser-warning timing, dodge movement/cooldown, stable pickup controls, safe wing grazes versus damaging core hits, combo reset rules, explosive fuel and reactor completion. The audio test checks generated samples are finite and bounded; it does not judge their subjective quality. A normal-damage automatic pilot completed the entire mission in 5920 frames (98.7 simulated seconds), with 30 kills, 32800 points, three lives and 74 shields. It used the same damage, dodge and pickup rules, without damage immunity. This demonstrates a viable route, not human difficulty balance or low-end performance certification. The older --verify immunity mode remains useful for isolating progression problems; --verify-normal checks ordinary damage-enabled progression.
