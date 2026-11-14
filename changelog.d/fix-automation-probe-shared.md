### Bug Fixes

* On macOS, permission reports that overlap while an Automation consent prompt is pending share one probe instead of each starting its own. A caller took the in-flight probe out of its slot while it waited, so a second caller found the slot empty and left another thread blocked on the prompt; eight overlapping callers now start one probe. (PRLINK)
