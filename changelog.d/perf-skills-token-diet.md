### Performance

* `skills get desktop` and `skills get platform` now print short guides (about 3,200 tokens together on Linux, down from 11,000), so the guide an agent loads before its first action no longer dominates every later model call; the detail moved to topics such as `skills get desktop refs-and-retry` and `skills get linux capabilities`, and `skills get <topic>` resolves a core or platform topic with no skill name ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
