### Performance

* Connecting to a browser launched with `launch --cdp` sends one request fewer: the registry already holds the browser's own WebSocket URL, so the `Browser.getVersion` probe is dropped and `Browser.setDownloadBehavior` proves the connection; a connection also asks `Browser.getVersion` at most once, so `open-url` no longer repeats it. In headless Chrome 156 on the local VM the whole connection took a median of 0.6 ms before and after, so the cut saves one browser round trip, which matters when the browser is slow to answer.

### Bug Fixes

* A web connection from a `launch --cdp` registry entry refuses a WebSocket URL whose path does not start with `/devtools/browser/`, with error kind `cdp_endpoint_not_browser`; before, a page endpoint passed both the handshake and `Browser.setDownloadBehavior` and was used as the browser.
