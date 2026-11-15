"""List price of a claude -p run from its token usage, and the corrected token and turn counts of a stored row.

claude -p reports total_cost_usd only in its final result event, so a run the watchdog kills has no price and
used to count as $0 against a spend cap. The rates below are the ones total_cost_usd uses: a least-squares fit
over the 737 stored Sonnet 5.5 rows with a reported price reproduces every one of them to 1e-13 relative error.
A model with no fitted rate has no estimate; callers treat that as unknown, never as free.

Stream-json assistant events carry the usage of the first chunk of each message, so their output_tokens is a
few tokens per call. Only the final result event has the real output count. A run without a result event keeps
the per-call sum, flagged as a lower bound.
"""

PER_MTOK = {
    "claude-sonnet-5-5": {"input_tokens": 2.0, "cache_read_input_tokens": 0.1,
                          "cache_creation_input_tokens": 2.5, "output_tokens": 10.0},
}
INPUT_KEYS = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")


def list_cost(model, usage):
    """Dollars at list price for one run's usage, or None when the model has no fitted rate."""
    rates = PER_MTOK.get(model)
    if rates is None or not usage:
        return None
    return round(sum(rates[k] * (usage.get(k) or 0) for k in rates) / 1e6, 6)


def row_cost(row):
    """(dollars, estimated): the reported price, else the list price of the recorded usage."""
    if row.get("cost_usd_list") is not None:
        return float(row["cost_usd_list"]), bool(row.get("cost_estimated"))
    return list_cost(row.get("model"), row.get("result_usage") or {}), True


def row_output_tokens(row):
    """Output tokens from the final usage when the row has it, else the recorded per-call sum."""
    usage = row.get("result_usage") or {}
    if usage.get("output_tokens") is not None:
        return int(usage["output_tokens"])
    return int(row.get("output_tokens") or 0)


def row_turns(row):
    """claude -p num_turns, or the API call count for a run stopped before its result event."""
    if row.get("num_turns") is not None:
        return int(row["num_turns"])
    return int(row.get("api_calls") or 0)
