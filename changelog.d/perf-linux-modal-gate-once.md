### Performance

* Linux: reads before the final pre-dispatch check reuse an unblocked modal verdict established earlier in the same command for the same target, frame and top-level windows; the read right before input still walks the whole tree.
* Linux: after a stateful action opens a modal inside the same window, the post-action check can confirm delivery from the target's own state using the earlier unblocked verdict; new top-level dialogs are still checked as before.
