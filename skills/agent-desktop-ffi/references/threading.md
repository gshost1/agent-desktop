# Threading

## Host-thread contract

FFI entrypoints may be called from any host thread. The library applies no
blanket macOS main-thread guard to Accessibility (`AXUIElement`) or Quartz event
(`CGEvent`) calls, and Apple publishes no main-thread requirement for them
([Thread Safety Summary](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html)).
Code paths that use Cocoa objects create their own autorelease pools, and FFI
initialization starts one `NSThread`, so Cocoa installs its multithreading locks
before any worker-thread AppKit call.

## Concurrency and mutation ordering

Adapter registry handles can be acquired concurrently. Read operations carry a
finite deadline and do not take the interaction lease.

| Concurrent calls | Contract |
|------------------|----------|
| read + read | Both may run concurrently; neither waits for the interaction lease |
| read + mutation | The read does not wait for the lease and may overlap the mutation; its result is a point-in-time observation, not a transaction |
| mutation + mutation | One lease holder runs at a time for the same OS user; the waiter consumes its command deadline and returns a structured timeout if it cannot acquire the lease |
| lease-taking call from a log callback while the same thread's lease-taking call holds the lease | Refused at once with `AD_RESULT_ERR_POLICY_DENIED`, details kind `reentrant_interaction_lease`; see Log callback threading for the list |
| native-handle call on another thread | Rejected with `AD_RESULT_ERR_INVALID_ARGS`; use a qualified ref instead |

Snapshots, finds, gets, display/window/app listings, screenshots, clipboard
reads, permission reads, status, and trace reads are observations. Ref actions,
direct native-handle actions, input synthesis, clipboard writes/clear, app and
window changes, notification actions, and permission requests are mutations.
For example, `ad_snapshot` and `ad_execute_by_ref` may be called from different
threads, but the snapshot can overlap the action; callers that require
observe-then-act ordering must wait for the snapshot result before dispatching
the action.

Desktop mutations take a process-independent lock: on macOS and Linux an
advisory lock at `/tmp/agent-desktop-<uid>/interaction.lock`, on Windows a
token-derived lock file. CLI and FFI builds therefore serialize mutations across
threads, processes and different HOME values for the same user. The lock is
command-scoped, not transaction-scoped: another actor may change the UI between
an observation and a later action, or between two separate actions.

The lease cannot coordinate:

- older `agent-desktop` binaries that predate the lock;
- direct human input or unrelated automation tools;
- state changes initiated by the target application itself.

Callers must still use strict refs/exact window identities and treat stale or
ambiguous targets as normal retryable automation outcomes.

## Adapter destruction

Adapter pointers are opaque registry tokens. Each call acquires a retained
adapter owner before platform work. `ad_adapter_destroy` revokes the token:
calls that already acquired it finish safely, while calls that begin afterward
return `AD_RESULT_ERR_INVALID_ARGS`. Concurrent destruction cannot free memory
still referenced by an in-flight call.

Destroying an adapter does not release native handles created by it. Using such
a handle afterward fails because the owner adapter token no longer exists, but
the handle stays allocated until `ad_free_handle` (with the same adapter pointer
value) or the end of the thread that created it.

## Native-handle thread ownership

`ad_find_exact` returns an opaque `AdNativeHandle`. `ad_resolve_element_exact`
never produces one: it fails with `AD_RESULT_ERR_ACTION_NOT_SUPPORTED`. Native handles
are adapter-bound and thread-affine:

- resolve, use, and release a handle on the same thread;
- pass the same adapter that produced the handle;
- do not use a handle after destroying its adapter.

Violations are rejected with `AD_RESULT_ERR_INVALID_ARGS`; the library does not
dereference forged, cross-adapter, cross-thread, released, or revoked tokens.
Prefer snapshot-qualified refs and `ad_execute_by_ref` when a handle would need
to cross an async task or thread boundary.

## Log callback threading

`ad_set_log_callback(cb)` may be called from any thread. The callback may be
invoked from any thread that calls an `ad_*` function.

The callback may itself call `ad_*` functions. Events those nested calls emit
are not delivered to the callback.

These calls take the interaction lease through the FFI layer: `ad_mouse_event`, `ad_mouse_event_with_modifiers`, `ad_mouse_wheel`, `ad_drag`, `ad_set_clipboard`, `ad_clear_clipboard`, `ad_execute_action`, `ad_execute_action_with_policy`, `ad_execute_by_ref`, `ad_execute_by_ref_timeout`, `ad_launch_app_exact`, `ad_close_app`, `ad_focus_window_exact`, `ad_window_op_exact`, `ad_notification_action`, `ad_dismiss_notification` and `ad_dismiss_all_notifications`.
One of them made from the callback while the call that logged is one of them
and holds the lease fails at once with `AD_RESULT_ERR_POLICY_DENIED`, details
kind `reentrant_interaction_lease` and `retryable: false`. When the call that
logged is `ad_execute_by_ref*` or `ad_close_app`, whose lease core takes
internally, the refusal covers the whole call, including the auto-wait and ref
resolution before the lease is taken.

Any other nested call that needs the interaction lease while the call that
logged holds it, including an observation that takes the lease internally (for
example to switch on a Chromium or Electron renderer's accessibility), waits
for a release that happens only after the callback returns, and can time out.
Do not make such calls from the callback; queue them for after the outer call
returns. Another host thread, or a thread still waiting for the lease, is not
refused and waits for the lease as usual.

A callback unregistered via `NULL` may still receive an invocation already in
flight on another thread. Keep the callback and captured state alive for the
process lifetime, or quiesce active adapter calls before unregistering it.

## Language runtimes

Runtime serialization is not thread affinity. For example, CPython's GIL does
not guarantee that two FFI calls execute on the same OS thread. Store native
handles only in thread-confined objects, or avoid them and use
snapshot-qualified refs. Rust, Swift, Node, Go, and managed runtimes need the
same discipline when tasks can migrate between worker threads.

## Accessibility permission identity

`ad_check_permissions` calls macOS `AXIsProcessTrusted()`, which reports trust
for the hosting executable (`python3`, `node`, a Swift app, and so on), not for
the dylib as a separate executable. Permission prompts and deployment guidance
must identify the host process that loads `libagent_desktop_ffi.dylib`.

## Last error and blocking calls

The last-error slot is thread-local (see error-handling.md).

`ad_wait` blocks its calling thread for at most its finite deadline and retains
the adapter for that duration. Destroying the adapter token concurrently stops
new calls but does not invalidate an in-flight wait.
