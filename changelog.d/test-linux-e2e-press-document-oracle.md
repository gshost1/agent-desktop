### Testing

* Read the Linux E2E keyboard-combo leg's document by its text role instead of the longest text in the app, and restore that document even when the leg fails.
* Preserve a keyboard-combo failure when restoration also fails, recording cleanup errors alongside it; restoration errors still fail an otherwise successful leg.
