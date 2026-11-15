"""Install generated material artwork as compact, metadata-free game tiles.

Usage: python tools/import_material_textures.py LOCAL_MANIFEST.json
The local manifest is a list of {name, path, prompt}. Source paths are never
copied into the public provenance file. Pillow is required. Generation uses
the built-in image tool; this script only resizes and packages its artwork.
"""
import json
import sys
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / "materials" / "textures"


def install(manifest):
    records = json.loads(Path(manifest).read_text(encoding="utf-8"))
    provenance = []
    sheet = Image.new("RGB", (7 * 152, ((len(records) + 6) // 7) * 156), (26, 27, 30))
    labels = ImageDraw.Draw(sheet)
    for i, record in enumerate(sorted(records, key=lambda r: r["name"])):
        name = record["name"]
        if not name.replace("_", "").isalnum():
            raise ValueError("Material name must be a plain filename")
        source = Image.open(record["path"]).convert("RGB")
        # One file is an entire material surface, not a sprite sheet. Keep
        # broad forms while avoiding noisy supersampling at one cell/pixel.
        tile = source.resize((128, 128), Image.Resampling.LANCZOS)
        clean = Image.frombytes("RGB", tile.size, tile.tobytes())
        clean.save(DEST / f"{name}.png")
        x, y = (i % 7) * 152 + 12, (i // 7) * 156 + 24
        sheet.paste(clean, (x, y))
        labels.text((x, y - 17), name, fill=(225, 225, 225))
        provenance.append({"file": f"{name}.png", "tool": "built-in image_gen", "prompt": record["prompt"]})
    (DEST / "generated-prompts.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    # Keep the old raw filename usable by external scenes without keeping
    # the superseded tiny third-party rock swatch in the active set.
    if any(r["name"] == "cave_rock" for r in records):
        (DEST / "rock.png").write_bytes((DEST / "cave_rock.png").read_bytes())
    sheet.save(ROOT / "docs" / "material-textures.png")
    print(f"Installed {len(records)} material tiles, 128x128 RGB, with no source metadata.")


if __name__ == "__main__":
    install(sys.argv[1])
