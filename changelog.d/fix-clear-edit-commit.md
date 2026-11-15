### Bug Fixes

* macOS `clear` written through the accessibility API (always headless) on a plain text field or text area now reports `delivered_unverified` with `application_commit: "unobserved"` and the same `commit_hint` as `set-value`, instead of `delivered_verified` while the app's model, which takes the text only when editing ends, still held the old value; a headed keyboard clear is judged by its read-back as before.
