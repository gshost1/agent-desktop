# Ref scroll coverage

`ref_actions.scroll` uses a native GTK4 window containing one scroll area and two checkboxes. The target is that sole scroll area; GTK4 need not preserve a supplied accessible name. Missing refs and multiple scroll areas fail setup rather than selecting an arbitrary container.

The raw AT-SPI oracle records the vertical adjustment, checkbox bounds and checked states. The headed action must move the upper checkbox out of the viewport and the lower checkbox into it, without toggling either. Cleanup restores the adjustment and independently checks the original geometry and states. X11 observations retain pointer and active-window restoration checks.

`ref_actions.scroll_headless_refusal` targets the same fixture. It requires `POLICY_DENIED`, `not_delivered` / `safe` and unchanged application, pointer and focus observations. Delivery counters are checked when supplied, not inferred from missing fields.

`ref_actions.settings_scroll_identity_refusal` retains the real Settings path in both modes. Its indistinguishable scroll containers must refuse with `AMBIGUOUS_TARGET`, `identity_not_unique` and disposition `not_delivered` / `safe`. This refusal does not supply delivery counters. The viewport, checkboxes, pointer and active window must remain unchanged, and cleanup restores the original scroll value even after an assertion fails. This is identity-safety coverage, not evidence that a scroll route ran.

`ref_actions.scroll_to` remains a separate positive Settings contract.
