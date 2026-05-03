# Credits

Miru Renderer Rust is original Miru code by eie, with technical reference,
compatibility work, and bundled third-party materials credited below.

This file records credits and provenance known from the repository audit. If a
source or license is marked as unconfirmed, verify it before distributing a
public release.

## Technical References

- danser-go, by Sebastian Krajewski (@Wieku) and contributors:
  reference and inspiration for replay rendering workflows, video output,
  and ranking/results presentation. No danser source code is intentionally
  copied into this repository. danser-go source files are distributed under
  GNU GPLv3 unless stated otherwise.
  https://github.com/Wieku/danser-go

- osu!lazer, by ppy Pty Ltd and contributors:
  reference for lazer replay metadata, mania gameplay behavior, skin
  conventions, storyboards, mod behavior, and scoring. osu!lazer's code and
  framework are MIT-licensed.
  https://github.com/ppy/osu

- rosu-pp:
  used as a Rust dependency for beatmap parsing, difficulty, and performance
  calculations.
  https://github.com/MaxOhn/rosu-pp

- FFmpeg:
  used as an external command-line tool for audio/video encoding and
  composition. FFmpeg is not bundled by this repository.
  https://ffmpeg.org/

## Bundled Assets

- Ubuntu Regular font:
  bundled as `assets/Ubuntu-Regular.ttf` under the Ubuntu Font Licence 1.0.
  The local license text is in `assets/Ubuntu-Regular-LICENSE.txt`.

- Lain Memories skin, by Yoush:
  bundled as `assets/skin/- Lain memories.osk`. The source forum topic is
  `Lain Memories v1.1`.
  https://osu.ppy.sh/community/forums/topics/1960824?n=1

## Direct Rust Dependencies

The renderer uses these direct Cargo dependencies:

- `anyhow`
- `byteorder`
- `lzma-rs`
- `wgpu`
- `bytemuck`
- `clap`
- `indicatif`
- `md5`
- `pollster`
- `serde`
- `serde_json`
- `zip`
- `regex-lite`
- `image`
- `ab_glyph`
- `bitflags`
- `rosu-pp`
- `hound`
- `woff2-patched`

Their transitive dependencies and exact versions are listed in `Cargo.lock`.
Each dependency remains under its own license.

## Audit Notes

The source audit searched for URLs, license headers, copyright notices,
author fields, and project names including `danser`, `lazer`, `ppy`, `rosu`,
`FFmpeg`, `Ubuntu`, `Lain`, and `Yoush`.

The audit found explicit references to danser in a results-layout test, many
compatibility references to osu!lazer behavior, the bundled Ubuntu font
license, the bundled Lain Memories skin metadata, direct `rosu-pp` use, and
FFmpeg integration. It did not find intentional copied source-code notices from
danser or osu!lazer.
