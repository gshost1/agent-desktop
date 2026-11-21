### Performance

* A web snapshot reads its iframes concurrently: requests to every frame and iframe target are pipelined on the one DevTools connection, at most 16 awaiting a reply, under the same deadline and per-frame budget, with output and ref order unchanged; on the Cambridge Dictionary serendipity page in headless Chrome 156 (25 iframe targets) a snapshot took a median of 93 ms instead of 144 ms (7 runs).
