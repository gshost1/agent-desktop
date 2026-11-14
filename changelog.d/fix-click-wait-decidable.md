### Bug Fixes

* A post-action `--wait-for` now follows the action into the windows its process opened after the action, so a selector in a dialog the click opened matches instead of burning the whole budget; once the ref's window closes it reads every window of that process. It ends at once with `APP_NOT_FOUND` (`details.kind: "wait_target_gone"`) when the process exits, and with `APP_UNRESPONSIVE` (`wait_owner_unresponsive`) after 5 s without an answer. The `--wait-timeout` default drops from 30000 to 10000 ms. ([#PR](https://github.com/gshost1/agent-desktop/pull/PR))
