### Performance

* `wait --element` on a ref without an element token (Linux, Windows) now polls the element at its recorded place after the first full read instead of searching the whole window on every poll, and counts the window once more before it reports the predicate met; `get` and `is` resolve and read under one 5 s budget instead of two. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
