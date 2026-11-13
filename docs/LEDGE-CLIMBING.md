# Ledge climbing

Open **Play-Climbing-Lab.cmd**. The course has four rising roofs to reach,
with the imported character and actual hanging/climbing animations when
the local animation look is installed. The public example uses the hero.

- A/D or Left/Right moves.
- Hold W/Up while jumping towards a ledge to catch it.
- While hanging, release W/Up, then press it again to pull up.
- S/Down lets go, including during a climb.

Holding W after the grab leaves you hanging, so you can stop and inspect
the animation. Instructions also appear above the character. The next
platform is reachable by jumping from the roof you just climbed onto.

## Reuse in another level

Traversal is optional per character. Add `climb SECONDS reach CELLS`:

```text
player 50 236 look hero zoom 4 climb 2 reach 6
```

The climb time can be 0.25–8 seconds and reach 1–10 cells. A cell is 10 cm.
The controller detects corners on solid scenery, checks room for the
character above the lip and along the entire route, then raises it outside
the wall before moving onto the platform. It checks current scenery and
moving props again during traversal. It does not pass through a ceiling.
Climbing from either side works. Loose sand and moving debris do not count
as ledges in this first version.

If support burns away or is erased, the character falls. A blast or lethal
damage also releases it. Dropping temporarily prevents grabbing again.
Health, body temperature, breath and death continue working while hanging.
The held corner, progress and button state survive snapshots and rewind;
scene saving preserves the traversal settings and starts with no held ledge.

## Animation graphs and game controls

The controller supplies `hanging`, `climbing`, `traversal_enabled` and
`climb_rate` parameters. The two provided graphs have high-priority
transitions into hang/climb, with death above them. `climb_rate` matches
clip playback to the configured physical climb time. A custom graph can
use the same inputs with its own state/clip names. The supplied graph's
climb state uses `rate climb_rate 1 0.01 20`.

`Physics::Controls` includes `drop` alongside `walk` and `jump`.
`ActorSpec::climb` enables `ClimbSettings`; `ActorStatus` exposes hanging
and climbing. The controller is engine code, independent of this course.
