### Bug Fixes

* Linux: a sibling that vanishes while an unnamed editable field's composite label is read counts as absent, so the field stays certainly unlabelled instead of reading as an uncertain name and a native read failure.

### Performance

* Linux: the composite-label read for unnamed editable fields reads each parent once for all of its fields read together, stops at the second editable child, skips parents with more than 8 children and skips fields inside web documents, so a form of unnamed inputs no longer costs one full sibling scan per field.
