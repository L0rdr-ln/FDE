# Assets

- `wallpaper.webp`: the default desktop wallpaper (a Frutiger Aero landscape from the Frutiger
  Aero Archive, "daniele" collection). Added at the repository owner's request, who states that
  it is free to use. If its author or the archive says otherwise, remove the file: FDE then falls
  back to its generated sky (`wallpaper::bundled()` returns None only if the file cannot be
  decoded; delete the `include_bytes!` line in `src/wallpaper.rs` and `configured()` falls back).

- `logo.svg`: the FDE wordmark: a glass orb with sky and a hill, drawn by hand as SVG, and the
  letters "FDE", whose outlines come from the Inter Display ExtraBold typeface (SIL Open Font
  License 1.1, which allows this), finished with glass gradients. MIT like the rest of the project.

Everything else in the project's artwork is generated in code.

There is no mascot yet. `docs/MASCOT.md` has the design brief and a picture prompt.
