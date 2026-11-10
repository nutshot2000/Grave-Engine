# Stunt Yard — first playable prototype

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
