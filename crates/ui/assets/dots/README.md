These 19 character assets were generated with the SVG export renderer from
[Dots Lab](https://dots-lab.pages.dev/) on 2026-10-06, using its Cartoon, Pills
and Glyphs presets. The original shapes, eyes and colors are preserved.
Each dot's stable identity selects one preset; its message bubbles use that
preset's body color. Assets are embedded locally; runtime network access is
not required.

The `animated/` directory contains the site's original idle, thinking, alert
and error choreography for all 19 presets: transparent lossless WebP sequences
at 128×128, sampled at the same 20 fps as Dots Lab's animated SVG exports.
Clyra decodes visible sequences in the background, renders their frames through
its shared animation clock, and uses the vector rest pose under reduced motion.

To regenerate, download the Dots Lab HTML and run:

```powershell
node scripts/export-dot-animations.cjs dots-lab.html frames.ndjson
cargo run -p clyra-ui --example export-dot-animations -- frames.ndjson crates/ui/assets/dots/animated
```

The asset build tool requires FFmpeg on PATH. The application does not.
