### Performance

* A web ref click sends two fewer requests. Attaching to the page no longer sends `Runtime.runIfWaitingForDebugger` when Chrome's `Target.attachedToTarget` event says the page is running, which Chromium always reports for an attach; a page reported paused, or one with no such event, is still released. The live read before each ref action also proves the page's document once instead of twice, so one `Page.getFrameTree` round trip goes. In headless Chrome 156 on the local VM a web ref click went from a median of 4.1 ms to 3.9 ms (three rounds of 7 runs, warm-up dropped).
