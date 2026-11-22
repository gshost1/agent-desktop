### Performance

* Linux reads contents-index scrollbar evidence alongside labels and values, and skips it for non-index reads.
* Linux contents-index row visibility ignores headers and scrollbars and does not infer list overflow from clipping by an outer scrolling page.
* Contents-index lists inherit scroll-area evidence only when the area contains exactly one list descendant, so a scrolling page does not mark several loaded lists partial.
* Compact snapshots preserve a list's own scroll evidence and inherit collapsed-wrapper evidence through at most three wrappers.
* macOS reads a vertical scrollbar's enabled state for contents evidence only during index-building snapshots, not searches or live-state reads.

### Documentation

* Clarify that snapshots of non-window surfaces also carry the contents index when their trees have at least 80 nodes.
