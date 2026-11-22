### Performance

* Linux: a command that reads the same target several times now walks the application's accessibility tree for modal dialogs once and reuses the unblocked verdict for up to one second, while the application lists the same top-level windows and emits no accessibility event; any event, new window, other target or blocked verdict walks again.
