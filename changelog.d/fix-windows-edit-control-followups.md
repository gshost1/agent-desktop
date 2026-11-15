### Bug Fixes

* On Windows, a `set-value` or `clear` whose classic edit does not answer `EM_REPLACESEL` in time no longer keeps its text buffer for the rest of the process. An edit in another process is still sent inline with `SMTO_ABORTIFHUNG`, because the system copies the text at send time. An edit on another thread of the calling process (an FFI host's own window) gets the text from a worker thread that owns it until the edit reads it or the edit's thread exits. At most 8 such writes wait per process, and a write past that is refused with `not_delivered` before anything is sent. ([#225](https://github.com/gshost1/agent-desktop/pull/225))

### Testing

* Unit tests pin the edit write dispositions (a refused reservation sends nothing, a failed or unanswered `EM_SETSEL` is `not_delivered` and sends no text, an unanswered `EM_REPLACESEL` is `delivered_unverified`) and the inline-or-worker route. Live Win32 fixture tests cover a held edit thread's late delivery, the pending-write bound, a waiting caller that still answers messages sent to its own windows, and disabled and over-limit edits keeping `ValuePattern`. ([#225](https://github.com/gshost1/agent-desktop/pull/225))
