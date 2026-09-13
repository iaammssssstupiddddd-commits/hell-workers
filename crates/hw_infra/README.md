# hw_infra

`hw_infra` owns infrastructure-domain logic. Its first module,
`hw_infra::lighting`, is the pure indoor-light field core introduced by P03.

The lighting core accepts normalized grid, mask, semantic occlusion, and radial
emitter snapshots. It deterministically produces UNORM16 linear radiance,
luminance, mask, revision/diff counts, diagnostics, and canonical SHA-256
checksums. It also exposes a pure RGBA8 packing helper and the frozen 100x100
performance fixture.

P03 source under `src/lighting/` must stay independent of Bevy ECS, render
assets, GPU APIs, and game-world queries. P04 owns ECS collection and rebuild
scheduling, P05 owns save/load lifecycle, P06 owns GPU upload and rendering,
and P07 owns gameplay and room consumers.

`lighting/properties.rs` uses proptest as a dev-dependency with only `std` enabled.
It checks emitter permutation invariance and no-op rebuilds on generated grids
up to 8x8 with at most six emitters (256 cases, 1024 shrink iterations).
No-op snapshots retain cells, mask, checksums and revision while all changed
counts become zero; whole-snapshot equality is deliberately not required.
Failure seed persistence and replay follow the [development guide](../../docs/DEVELOPMENT.md#property-tests).

Run the focused tests with:

```bash
python3 scripts/dev.py cargo -- test -p hw_infra
```
