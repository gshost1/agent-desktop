### Bug Fixes

* `list-notifications` gives each entry a `key` over its app, title and body, with a count that tells identical twins apart; `dismiss-notification --key` and `notification-action INDEX ACTION --key` act only on the entry that carries it, so a list that shifted between the listing and the call cannot redirect a dismiss or an action
