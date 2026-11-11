# Voidrunner — Operation Cinder

A new horizontal scrolling shooter built on Grave Engine. Open Play-Voidrunner.cmd in the engine folder.

Controls: Enter launches; WASD or arrows move; Space/Z fires; X spends a pulse bomb; P/Escape pauses; R restarts. Movement speed stays constant while firing. Collect green + pickups for shields and gold W pickups for a wider weapon spread. Three lives and three bombs per run.

The convoy includes hunter wings, shielded tankers and siege groups. Blue shield escorts protect their leader, green repair drones restore armour, and minelayers drop explosive mines. The modular carrier arrives after 64 seconds. Break its two shield generators to expose the gun pods and cooling tanks; destroying both systems exposes the final explosive reactor. Shooting off a gun pod removes that pod's attacks. This replaces the earlier timed-shield/three-reactor carrier.

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


## Squads, modular carrier and pacing revision — 11 November 2026

The combat window is now 640 x 360 cells, up from 480 x 270: 33% more width and about 78% more world area visible at once. Ship scale remains the same in world cells, so formations have more room on screen. HUD layout scales separately.

New squads use a shared group identity. Convoys bring two shield escorts and a repair drone. Siege groups add a minelayer. Hunter wings enter as a three-ship formation. Shield emitters are vulnerable themselves; removing them drops protection from the leader. Repair drones replace a limited number of missing armour cells on a nearby surviving squad member, without recreating explosive fuel or reviving a destroyed ship. Mines contain actual gunpowder and can be shot to detonate them.

The carrier has seven separate material bodies mounted to its physical frame: two generators, two gun pods, two water-filled cooling tanks and one reactor. These are welded components with their own material damage. A sufficiently damaged module detaches and stops its function; gun-pod destruction cancels its pending charged beam. The core remains protected while supporting systems survive. Critical fuel fragments remain targets if native fragmentation replaces the original body ID. Components are disabled at a game-defined remaining-cell threshold; they do not need every cell erased.

User playtesting then found that correct mechanics still felt slow and empty. The revision brings the first target forward to 0.6 seconds, adds early hunter wings and a minelayer to close the opening lull, compresses the approach to the carrier from 80 to 64 seconds, and speeds up ordinary scouts. The primary weapon now fires rapid twin pulses at higher speed and greater penetration, with a small forward aim-assist cone. Upgrades add further spread. This is intended to reduce waiting and prolonged armour chipping; automated completion is not evidence of subjective fun.

Verification: all 18 focused checks pass. New coverage checks escort protection, repair without creating fuel, real explosive mines, intact carrier assembly, attacks disappearing when guns detach, staged core access and targetable reactor fragments after ID changes. The damage-enabled pilot completes the new modular encounter; the current final run log is included with the saved outputs. No broad claim about human difficulty, frame rate on other hardware or fun is made from that result. Existing controls remain the same.
