### Testing

* Resolve owned Linux E2E applications by PID in list-apps before menu waits, rather than passing toolkit labels or launcher names that are not product application names.
* Wait for the Qt legs' exact process-set restoration after closing menus, allowing temporary key-listener holders to exit without ignoring persistent leaks.
