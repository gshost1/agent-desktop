### Features

* A snapshot of 40 or more nodes starts with `data.contents`, an index of what the tree holds and where: the commonest ref roles by count and, for each list, table, outline, grid, list-like scroll area, toolbar and tab bar, its line in the tree, its ref, how many items it has loaded and whether it can still scroll. A read cut short, or one that stops at a long list, still shows what exists past it. It costs about 300 bytes; `--lines` prints it as lines. ([#219](https://github.com/gshost1/agent-desktop/pull/219))
