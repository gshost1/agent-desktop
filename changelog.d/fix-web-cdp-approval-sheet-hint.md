### Bug Fixes

* A web command that times out waiting for the browser's "Allow remote debugging?" prompt (`PERM_DENIED`, `cdp_approval_pending`) is now `delivery_uncertain` with `retry: unsafe` and `details.retryable: false`, and its suggestion names `list-surfaces` and `snapshot --surface sheet` to read and answer the prompt; before, it was `retry: safe` and said to retry with a longer timeout, and each retry stacked one more prompt. The web skill and `docs/json-output.md` gave this timeout the kind `cdp_approval_required`, which is the read-only refusal; they now document both kinds.
