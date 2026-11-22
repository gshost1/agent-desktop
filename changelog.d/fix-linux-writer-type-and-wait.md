### Bug Fixes

* Linux: type into the uniquely focused writable paragraph or heading through a document ref, report the actual input target, reuse stable refs across edits, and find document content with `wait --text` or `find --text`.
* Ref identity: document, paragraph, heading, and webarea values no longer count as identity, including on macOS (AXWebArea and AXHeading), Windows (UIA Document), and the web surface. Without a stable name, description, native ID, or retained element token, identity is Unknown; editable content alone cannot identify a ref. Windows behavior is untested.
