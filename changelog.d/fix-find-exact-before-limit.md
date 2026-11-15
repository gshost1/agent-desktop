### Bug Fixes

* `find --name` keeps the matches whose whole name equals `--name` when it cuts its matches to `--limit`, including the default of 50. The cut took the first 50 matches in traversal order before whole-name matches were moved to the front, so a whole-name match that came after 50 longer names, such as `Save` after a list of `Save As ...` items, was dropped unless the caller passed `--limit 0` or `--exact`. Without a whole-name match the limit keeps the same matches as before.
* `find --help` says that `--text` also matches the ancestors whose subtree holds the text, as the skills and docs already did.
