### Bug Fixes

* Linux: a sibling destroyed while an unnamed editable field's composite label is read, shown by its own failed role read, counts as absent, so the field stays certainly unlabelled instead of reading as an uncertain name and a native read failure; a live sibling that refuses a state or interface read leaves the label unknown.
* Linux: unnamed editable fields inside web documents no longer take a composite label from their parent's relations or a sibling combo box.

### Performance

* Linux: the composite-label read for unnamed editable fields reads each parent once for all of its fields read together and stops reading siblings at the second editable child, so a form of unnamed inputs no longer costs one full sibling scan per field.
