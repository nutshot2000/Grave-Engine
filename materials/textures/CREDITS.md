# Material texture artwork

The active set is 28 original material surfaces generated for Grave Engine
with the built-in image_gen tool, then packaged as 128x128 RGB tiles by
`tools/import_material_textures.py`. Sandstone is natural sedimentary rock;
only brick and cobblestone use masonry-like joints. The legacy filename
`rock.png` is a compatibility copy of `cave_rock.png`.

The full prompts for each final tile are in [generated-prompts.json](generated-prompts.json).
A labelled preview is in [the texture sheet](../../docs/material-textures.png).
The install step removes source metadata. Source images are not taken from
third-party texture packs, photographs, or other games.

These files replace the earlier piiixl-derived tiles and procedural rock/
wood swatches. The unused third-party source pack remains outside the repo.
The former procedural cave script now writes only to `shots/`, and the
Copper Creek example preserves an existing shared wood texture.
