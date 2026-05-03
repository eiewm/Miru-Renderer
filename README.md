<p align="center">
  <img src="assets/logo2.png" alt="Miru Renderer logo" width="180">
</p>

<h1 align="center">Miru Renderer</h1>

<p align="center">
  A Rust command-line renderer for osu!mania replays, beatmaps, and autoplay videos.
</p>

Miru Renderer turns osu!mania inputs into rendered gameplay videos. It can render real `.osr` replays against their matching `.osu` beatmap, generate autoplay renders from a beatmap or mapset, apply skins and HUD layouts, compose backgrounds and audio, and export the final video through FFmpeg.

> [!TIP]
> Just want to render a replay without building the CLI? Use the Miru Renderer web app: https://app.miru.uno

> [!IMPORTANT]
> Replay rendering always needs the beatmap data too. Use `--replay` with either `--osu` or `--mapset` so the replay can be checked against the exact chart it was played on.

## Highlights

- Render osu!mania `.osr` replays to video.
- Render autoplay from `.osu`, `.osz`, `.zip`, or extracted mapset directories.
- Select mapset difficulties by replay checksum, difficulty name, or index.
- Use custom skins from directories, `.osk`, or `.zip` archives.
- Enable intro screens, HUD, score/combo/accuracy displays, judgment popups, lighting, barlines, storyboards, and background video.
- Tune render size, FPS, scroll speed, lead-in, volumes, motion blur, encoder, and FFmpeg thread count.
- Generate preview frames, inspect beatmaps, list mapset difficulties, and run judgment-only dry runs.
- Support x264 plus optional hardware encoder paths through FFmpeg (`nvenc`, `amf`, `qsv`, or `auto`).

## Requirements

- Rust toolchain with edition 2021 support.
- `ffmpeg` and `ffprobe` available in `PATH`.
- A GPU/backend supported by `wgpu` for rendering.
- For hardware encoding, an FFmpeg build and driver stack that expose the requested encoder.

> [!NOTE]
> The current CLI accepts `1280x720` or `1920x1080`, and `60` FPS. Manual `--start`/`--end` ranges are capped at 600,000 ms by default and can be raised with `--max-render-duration-ms`.

## Quick Start

Build the release binary from the repository root:

```powershell
cargo build --release
```

Run the CLI directly on Windows:

```powershell
.\target\release\miru.exe --help
```

Run the CLI directly on Linux:

```bash
./target/release/miru --help
```

Or install it into your Cargo bin directory:

```powershell
cargo install --path .
miru --help
```

## Usage

Render a replay with an explicit beatmap:

```powershell
miru --replay "replay.osr" --osu "beatmap.osu" --skin "skin-dir" --out "render.mp4"
```

Render a replay using a full mapset:

```powershell
miru --replay "replay.osr" --mapset "mapset.osz" --out "render.mp4"
```

Render autoplay from a single beatmap:

```powershell
miru --osu "beatmap.osu" --out "autoplay.mp4"
```

Render autoplay from a mapset and choose a difficulty:

```powershell
miru --mapset "mapset.osz" --difficulty "Insane" --out "autoplay.mp4"
```

List the osu!mania difficulties inside a mapset:

```powershell
miru --mapset "mapset.osz" --list-diffs
```

Save a preview frame without rendering a full video:

```powershell
miru --replay "replay.osr" --osu "beatmap.osu" --preview-out "preview.png"
```

Run judgment analysis without creating a video:

```powershell
miru --replay "replay.osr" --osu "beatmap.osu" --dry-run --report-out "report.json"
```

Inspect beatmap metadata as JSON:

```powershell
miru --osu "beatmap.osu" --inspect-beatmap
```

## Common Options

| Area | Options |
| --- | --- |
| Inputs | `--replay`, `--osu`, `--mapset`, `--difficulty`, `--diff-index`, `--songs-dir` |
| Output | `--out`, `--preview-out`, `--preview-time-ms`, `--report-out` |
| Render | `--width`, `--height`, `--fps`, `--ss`, `--lead-in`, `--start`, `--end`, `--max-render-duration-ms` |
| Skin and HUD | `--skin`, `--hud-config`, `--skip-hud`, `--hud-editor-preview` |
| Gameplay visuals | `--no-lighting`, `--barlines`, `--no-storyboard`, `--no-skin-animations`, `--no-combo-burst`, `--no-sv` |
| Background | `--bg-opacity`, `--bg-blur`, `--bg-offset-x`, `--bg-offset-y`, `--bg-compose`, `--no-bg-video` |
| Intro | `--no-intro`, `--intro-user-json` |
| Audio | `--music-volume`, `--hitsound-volume` |
| Encoding | `--encoder`, `--preset`, `--motion-blur`, `--ffmpeg-threads`, `--gpu-preference` |
| Diagnostics | `--dry-run`, `--inspect-beatmap`, `--list-diffs`, `--list-autoplay-mods`, `--nd`, `--ap` |

For the full generated help:

```powershell
miru --help
```

## Autoplay Mods

Autoplay renders can use a JSON config file:

```powershell
miru --mapset "mapset.osz" --autoplay-mods-config "autoplay-mods.json" --out "autoplay.mp4"
```

List the supported autoplay mod catalog:

```powershell
miru --list-autoplay-mods
```

Example config:

```json
{
  "version": 1,
  "mode": "autoplay",
  "mods": [
    {
      "acronym": "SV2",
      "enabled": true,
      "settings": {}
    },
    {
      "acronym": "DT",
      "enabled": true,
      "settings": {
        "speed_change": 1.5,
        "adjust_pitch": false
      }
    }
  ]
}
```

Supported categories include speed mods (`DT`, `NC`, `HT`, `DC`, `AS`, `WU`, `WD`), visibility mods (`FI`, `HD`, `FL`, `CO`), pattern mods (`MR`, `IN`, `HO`), score profiles (`SV1`, `SV2`), and audio modifiers such as `MU`.

## Intro User Data

Use `--intro-user-json` to override avatar, country, flag, or team badge data shown in the intro. Relative paths are resolved from the JSON file's directory.

```json
{
  "avatar_path": "avatar.png",
  "country_code": "EC",
  "flag_path": "flag.png",
  "team_badge_path": "team.png"
}
```

## Project Layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | CLI parsing, validation, and top-level render flow |
| `src/converter/` | Replay/autoplay preparation, render orchestration, reports, and temp files |
| `src/parser/` | `.osu`, `.osr`, storyboard, and skin config parsing |
| `src/modes/mania/` | osu!mania conversion, timing, scoring, and judgment logic |
| `src/renderer/` | GPU renderer, frame planning, HUD, storyboards, textures, and effects |
| `src/video/` | FFmpeg composition, audio handling, playback rate changes, muted/nightcore support |
| `src/hud/` | HUD configuration, metrics, text sprites, score, combo, and accuracy helpers |
| `src/intro/` | Intro scene rendering, backgrounds, logos, avatars, badges, and timing |
| `src/results/` | Results screen models, layout, animation, and assets |
| `src/beatmaps/` | Mapset loading, archive extraction, difficulty selection, and beatmap resolution |
| `assets/` | Default logo, avatar, font, mod icons, audio assets, and bundled skin |

## Development

Format the code:

```powershell
cargo fmt
```

Run a local debug build with CLI arguments:

```powershell
cargo run -- --osu "beatmap.osu" --dry-run
```

## License

Miru Renderer is distributed under the PolyForm Strict License 1.0.0. See `LICENSE` for the license text and `CREDITS.md` for third-party credits.

This is source-available software, not an open-source license. Third-party redistribution, derivative works, and commercial use are restricted by the license.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `ffmpeg` or `ffprobe` errors | Install FFmpeg and make sure both commands are in `PATH`. |
| Replay checksum mismatch | Use the exact `.osu` or `.osz` that belongs to the replay. |
| Multiple mapset difficulties found | Pass `--difficulty "Name"` or `--diff-index N`. |
| Unsupported resolution or FPS | Use `1280x720` or `1920x1080`, and `60` FPS. |
| Hardware encoder falls back or fails | Try `--encoder x264`, update GPU drivers, or use an FFmpeg build with the desired encoder. |
| Long manual clip rejected | Increase `--max-render-duration-ms` when using `--start` and `--end`. |
