### Performance

* `skills get desktop` and `skills get platform` now print short guides (8,383 bytes, about 3,500 tokens, together on Linux, down from 28,157 bytes and 11,000 tokens), so the guide an agent loads before its first action no longer dominates every later model call; the detail moved to topics such as `skills get desktop refs-and-retry` and `skills get linux capabilities`, and `skills get <topic>` resolves a core or platform topic with no skill name ([#167](https://github.com/gshost1/agent-desktop/pull/167))
