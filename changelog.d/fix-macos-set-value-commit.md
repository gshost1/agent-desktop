### Bug Fixes

* On macOS, `set-value` on a single-line text field or combo box performs `AXConfirm` after the write, as Return does, so a field that hands its text to the document only when editing ends commits the new value. Such a field read back the new text and `set-value` reported `delivered_verified`, but the document kept the old value and Save wrote it: an Automator URL field and Automator action fields in held-out runs. Password fields and text areas are not confirmed.
