# Test Fixtures

Hand-written golden files that tests compare serialized output against. None is captured from a real app.

| Fixture | Asserted by |
|---|---|
| `skeleton-tree.json` | `test_skeleton_fixture_matches_golden` in `crates/core/src/snapshot/snapshot_tests.rs` |
| `skeleton-tree.lines.txt` | `skeleton_fixture_renders_to_its_golden_lines` in `crates/core/src/snapshot/ref_lines_tests.rs` |
| `drilldown-refmap.json` | `test_drilldown_refmap_matches_golden_fixture` in `crates/core/src/snapshot/snapshot_ref_merge_tests.rs` |
| `find-matches.json` | `matches_omit_absent_values_and_empty_lists_as_the_golden_file_shows` in `crates/core/src/commands/find_response_tests.rs` |
| `trace_show/trace/*.jsonl` | `show_merges_fixture_segments_with_expected_shape` in `crates/core/src/commands/trace_tests.rs` |

The JSON fixtures are loaded at compile time, so a missing or renamed file breaks the build:

```rust
let golden = include_str!("../../../../tests/fixtures/skeleton-tree.json");
```

Any change to the shape a fixture pins must update the fixture in the same commit.
