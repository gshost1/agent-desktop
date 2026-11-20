### Testing

* The Linux live E2E text legs restore the document with `set-value` instead of `clear` and a `--headed type`, so a document that reopened with a multi-line draft from an earlier leg is restored instead of failing `INVALID_ARGS` on its newlines (`text_entry.typed_text_reaches_document`). ([#215](https://github.com/gshost1/agent-desktop/pull/215))
