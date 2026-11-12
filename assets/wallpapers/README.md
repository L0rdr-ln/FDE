# Theme wallpapers

Drop a picture here named like the theme's `file` key (see `themes/*.toml`) and that theme shows it
instead of its gradient. Themes without a picture here fall back to their gradient.

Your own pictures (any licence) go in `~/.local/share/fde/wallpapers` under the same names:
that folder is looked at before this one, so they win over what FDE ships.

| theme | file | what fits | status |
|---|---|---|---|
| Frutiger Eco | `eco.webp` | green leaves with water drops, a bright meadow under a sky | needed |
| Dorific | `dorific.webp` | warm orange sky or soft orange glow | needed |
| Vector Flourish | `vector-flourish.webp` | pastel floral swirls on white | needed |
| Frutiger Aqua | `aqua.jpg` | bright coral reef or aquarium | added (public domain) |
| Frutiger Metro | `frutiger-metro.jpg` | bold green abstract, colourful bokeh | needed |
| Frutiger Aero | (built in, `assets/wallpaper.webp`) | | done |

PNG, JPEG or WebP, at least 2560x1440 (the picture is scaled to the monitor with the chosen
mode). Files are looked up in `$FDE_WALLPAPERS`, `~/.local/share/fde/wallpapers`,
`/usr/share/fde/wallpapers` and this folder.

## Licences

Only add pictures that may be redistributed in an MIT-licensed repository: CC0, public domain
(for example NOAA or NASA photographs), or a licence that allows redistribution with attribution
(then credit it below). Check the licence on the picture's own page, not on a collection that
holds it. Pexels, Unsplash and Pixabay pictures are free to use but their licences do not allow
redistributing the picture on its own, so they do not belong here.

| file | title and author | source link | licence |
|---|---|---|---|
| `aqua.jpg` | "GRNMS - Painted wrasse and sargassum", Gray's Reef National Marine Sanctuary (NOAA) | https://commons.wikimedia.org/wiki/File:GRNMS_-_Painted_wrasse_and_sargassum_(27626284033).jpg | public domain (work of the US federal government); scaled to 3840 px wide |

The three pictures added earlier (an Aqua render, an orange Dorific picture and a Metro
illustration) had no known source or licence and were taken out again.
