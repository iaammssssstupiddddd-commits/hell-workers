# Bevy 0.19 Zstd decoder dependency security patch

Source: crates.io `bevy_image` 0.19.0 (MIT OR Apache-2.0).
The normalized package manifest, original manifest, complete `src/` tree and
both licenses are copied unchanged from the official package, except for the
single dependency requirement below. Registry bookkeeping (`.cargo-ok`,
`.cargo_vcs_info.json`) and the package's standalone lockfile are not vendored;
the root workspace lockfile controls the resolved dependencies.

- Package archive SHA256:
  `37bc41f69a0c6ade2e7961602517545b39ab8e42bc8c4a729bd24ffee3bcd904`
- Upstream repository: <https://github.com/bevyengine/bevy>
- Package VCS revision: `c6f634ca9f406d68ba5109d921247b654cb42c10`
- Upstream package path: `crates/bevy_image`

Only the normalized `Cargo.toml` requirement for optional `ruzstd` changes from
`0.8.0` to `0.9.1`. `Cargo.toml.orig` remains an upstream provenance document,
not the build manifest. All Bevy source, public APIs and feature flags remain
unchanged; the engine stays on Bevy 0.19. No registry files are edited in place.

[RUSTSEC-2026-0344](https://rustsec.org/advisories/RUSTSEC-2026-0344.html)
requires ruzstd 0.9.1 or newer. The existing `zstd_rust` feature uses its
compatible `StreamingDecoder` / `Read` API. The fixed decoder bounds each
decompressed block by the frame window and the Zstandard block size limit.
Do not suppress this advisory or disable the image/tonemapping features.

`crates/bevy_app/tests/zstd_image_decoder.rs` exercises Bevy's actual KTX2
decoder with tiny RLE frames: a valid 1024-byte window boundary, an invalid
1025-byte block for that window, and a truncated block. The invalid fixture
never allocates a large decompression bomb. Normal/profiling compilation,
dependency audit and owned-window feedback are separate verification duties;
the presence of this patch is not evidence that those checks passed.

Remove this patch when the supported Bevy dependency graph supplies an
equivalent fixed decoder. Before updating, compare every vendored file with
the official package, retain license/provenance, rerun the decoder rejection
tests and verify the enabled native image path.
