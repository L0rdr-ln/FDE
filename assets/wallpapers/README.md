# Theme wallpapers

Drop a picture here named like the theme's `file` key (see `themes/*.toml`) and that theme shows it
instead of its gradient. Themes without a picture here fall back to their gradient.

Your own pictures (any licence) go in `~/.local/share/fde/wallpapers` under the same names:
that folder is looked at before this one, so they win over what FDE ships.

| theme | file | what fits | status |
|---|---|---|---|
| Frutiger Eco | `eco.jpg` | green leaves with water drops, a bright meadow under a sky | added (CC0) |
| Dorific | `dorific.jpg` | warm orange sky or soft orange glow | added (CC0) |
| Vector Flourish | `vector-flourish.jpg` | pastel floral swirls on white | added (public domain) |
| Frutiger Aqua | `aqua.jpg` | bright coral reef or aquarium | added (public domain) |
| Frutiger Metro | `frutiger-metro.jpg` | bold green abstract, colourful bokeh | added (CC0) |
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
| `dorific.jpg` | "Sydney sunset sea paddling (Unsplash)", Samuel Scrimshaw (samscrim) | https://commons.wikimedia.org/wiki/File:Sydney_sunset_sea_paddling_(Unsplash).jpg (original: https://unsplash.com/photos/c-_3x6Sk43U) | CC0 1.0 (public domain dedication, as shown on the file's own page); scaled to 3840 px wide |
| `frutiger-metro.jpg` | "Translucent leaf in macro 1 (Unsplash)", Rodion Kutsaev (frostroomhead) | https://commons.wikimedia.org/wiki/File:Translucent_leaf_in_macro_1_(Unsplash).jpg (original: https://unsplash.com/photos/K715ldqJ5JI) | CC0 1.0 (public domain dedication, as shown on the file's own page); scaled to 3840 px wide |
| `eco.jpg` | "Meadow and peaks in Cascade Canyon, Grand Teton National Park", Z3lvs (own work) | https://commons.wikimedia.org/wiki/File:Meadow_and_peaks_in_Cascade_Canyon,_Grand_Teton_National_Park.jpg | CC0 1.0 (as shown on the file's own page); scaled to 3840 px wide |
| `vector-flourish.jpg` | "Apple-tree blossoms 2017 G3", George Chernilevsky (own work) | https://commons.wikimedia.org/wiki/File:Apple-tree_blossoms_2017_G3.jpg | public domain (dedicated by the author, as shown on the file's own page); scaled to 3840 px wide |
| `aqua.jpg` | "GRNMS - Painted wrasse and sargassum", Gray's Reef National Marine Sanctuary (NOAA) | https://commons.wikimedia.org/wiki/File:GRNMS_-_Painted_wrasse_and_sargassum_(27626284033).jpg | public domain (work of the US federal government); scaled to 3840 px wide |

(The Dorific and Metro pictures were posted on Unsplash before 2017 under CC0 and are kept on Wikimedia
Commons with that licence; the licence line on each file's page was checked when they were added.)

The three pictures added earlier (an Aqua render, an orange Dorific picture and a Metro
illustration) had no known source or licence and were taken out again.
