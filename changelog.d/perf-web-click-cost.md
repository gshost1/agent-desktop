### Performance

* Attaching to a page for a web ref action or snapshot no longer sends `Runtime.runIfWaitingForDebugger` when Chrome's `Target.attachedToTarget` event says the page is already running, which saves one round trip per command; a page reported paused, or one with no such event, is still released. In headless Chrome 156 on the local VM a web ref click through the core path went from a median of 5.1 ms to 4.7 ms (7 runs, warm-up dropped, three rounds).
