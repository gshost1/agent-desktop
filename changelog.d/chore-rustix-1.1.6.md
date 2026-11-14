### Bug Fixes

* On Linux, the pinned `rustix` moves from 1.1.5 to 1.1.6 for the unsoundness advisory RUSTSEC-2026-0486 in its xattr functions, which agent-desktop does not call. The advisory failed the dependency audit on every pull request. ([#150](https://github.com/gshost1/agent-desktop/pull/150))
