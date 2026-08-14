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

Run the focused tests with:

```bash
python3 scripts/dev.py cargo -- test -p hw_infra
```
