# Copper Creek

A Western frontier town inspired by timber ranch porches and balcony-fronted lodging houses. These are original native pixel/material assets, not imported RDR2 models or textures.

Open **Scenes... > copper-creek**. The saved level starts intact, with no fire pre-lit.

## The street

- Ranch house: broad timber roof, central gabled porch, carved longhorn emblem, glazed windows, bed and kitchen table.
- Creek Trading: stepped false front, painted dry-goods sign, shelves and counter.
- Copper Spur Saloon: two storeys, balcony and rail, swinging doors, bar, piano and upstairs bed.
- Sheriff's office: painted sign, desk and iron jail bars behind the timber front.
- Water tower, loaded wagon, hitching rail, watering trough, dry tree, hay bales, oak barrels and powder kegs.
- Warm lanterns and a dusty road against a mountain backdrop.

## Try burning it

Press **Play**, select **Heat**, and heat the TNT keg/hay stack between Creek Trading and the saloon. Shift + mouse wheel changes brush size. The keg has real gunpowder, and a small oil puddle feeds the local fire. Powder charges are deliberately small so one keg does not launch every facade. Fronts burn away to expose the rooms. Buildings use 126–180 physical parts. Cladding and backing boards are at most 40 × 6 cells, with local breakable nail joints. Roofs and posts are also divided into sections; metal floor ties behind the facade help the frame outlast the cladding.

Try the loaded wagon and the tower legs too. The tower contains ordinary free-flowing water behind its front. Its water is held by a rear tank and supports; burning those supports lets it fall and spill. The trough contains water too.

**R / Reset** restores the state before Play. Wheel zooms in Select mode, middle-drag pans and Fit shows the entire street.

## Reuse the kit

Search **west_** in the asset library: 13 reusable assets are included. Double-click a card to edit its parts. The buildings have aligned porch, facade, interior and backboard planes. Glass, iron, cloth, straw and timber retain their own material behavior; colour is surface paint.

Two material presets accompany the kit: dry straw, and weathered construction timber with the density, conductivity and toughness of wood, but slower charring (1,200 frames), a higher ignition temperature (360°C), reduced flame/ember output and gradual softening above 500°C. A quiet original texture keeps painted planks readable. Existing wood and existing levels are preserved.

## Validation

`cargo run --release --example build_copper_creek -- --verify` rebuilds this kit and level, checks that all named part counts stay intact for 180 settling frames, then ignites the first keg in a separate simulation for 2,000 frames. It checks actual timber loss, powder consumption, fire reaching the interiors and more than half the street timber surviving, and writes standing/fire/aftermath images to `shots`.

Automated tests also cover individual cladding bounds and joints, compact runtime bodies, asset save/reload, legitimate independent depth parts, straw burning and water being released when the tower supports burn.

The Blender timber house (`scenes/house.txt`) remains unchanged. Its sparse mixed-material frame is the reference for a slower structural burn; this town adds full facades, so its burn is still visually denser.
