### Testing

* The Linux cursor-overlay Wayland refusal test binds its socket in the temp directory instead of the crate directory, so it no longer fails with `path must be shorter than SUN_LEN` in a checkout whose path is longer than about 60 bytes.
