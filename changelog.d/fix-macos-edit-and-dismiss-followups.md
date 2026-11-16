### Bug Fixes

* macOS: `dismiss-notification` and `dismiss-all-notifications` carry `data.list_restore: "unverified"` when Notification Center could not be closed or focus restored after the dismissal, as `notification-action` already did
* macOS: `clear` on a text element that is already empty writes nothing and returns `not_delivered` with no commit hint, instead of `delivered_unverified` with a hint to end the edit
