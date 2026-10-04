# Parley 0.9 Japanese segmentation backport

Source: crates.io `parley` 0.9.0 (Apache-2.0 OR MIT), matching Bevy 0.19.0.
The package manifest, README, licenses and complete `src/` tree are retained.
Only `src/analysis/mod.rs` is changed: word and all three line segmenter
constructors load ICU's compiled dictionaries with `new_dictionary` instead of
`new_for_non_complex_scripts`. Non-const constructors replace the const blocks.
Public APIs and font/shaping/editor code are unchanged. No logging is disabled.

Upstream context: <https://github.com/bevyengine/bevy/issues/24094> and
<https://github.com/bevyengine/bevy/pull/24683>. The upstream feature targets
Parley 0.10; this backport avoids upgrading Bevy or changing its Parley API.

Remove this patch when the supported Bevy 0.19 dependency graph supplies an
equivalent dictionary-enabled configuration. Audit the registry source diff
before updating; do not change the account's Cargo registry in place.

Tests belong to the application's dependency contract tests and exercise this
patched `LayoutContext` analysis plus bounded Japanese layout and editor paths.
Owned-window native feedback checks actual Japanese rendering separately.
