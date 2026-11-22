### Performance

* Linux: reads before the final pre-dispatch check reuse an unblocked modal verdict established earlier in the same command for the same target, frame and top-level windows; the read right before input still walks the whole tree.
