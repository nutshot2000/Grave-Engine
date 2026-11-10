# Inferno Run — Stunt Yard level

A 360-metre side-scrolling course with six real terrain jumps, three burning obstacles, water gaps and shallow splashes, a lava gap, gravel, sand and six breakaway wooden props. Nine stage gates award points before the finish.

Space starts. D/right accelerates; A/left reverses; Q/E leans; R restarts the course. C retains the separate wall-crash demonstration.

The camera follows smoothly with speed-dependent look-ahead. It uses the actual world dimensions, follows the full 3600-cell route and clamps at the world edges. Jump warning labels, a course progress bar and distant warehouses moving at a slower speed provide direction and depth.

All terrain, fire, water, lava, loose surfaces and breakaway props are ordinary engine scene data in scenes/stunt-yard.txt. Regenerate with tools/make_stunt_course.py. The motorcycle retains its rubber/iron/brass cells and breakable assemblies.

Verified an automated full-throttle completion (7129 points), all nine gates and restart. The wall-crash and tyre-burning checks also pass. A camera test checks following beyond 1600 cells and clamping near the far end. Inspected the live level window. This remains a prototype: stage scoring uses passage markers, the rider has no ragdoll, and the simulation still updates the full world. R restarts from the beginning; there are no checkpoints yet.
