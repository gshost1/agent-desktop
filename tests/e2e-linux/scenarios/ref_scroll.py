from oracle.atspi_raw import poll
from runner.helper import changed, command, managed, nodes, one, ref, require, root, snapshot
from runner.legs import implementation
from scenarios.headed_actions import assert_ok
from scenarios.ref_effects import desktop, desktop_restored, policy_refusal
from scenarios.scroll import raw_value


def content_bar(tree):
    bars = [node for node in tree.find(role=48) if node.value() is not None
            and "VERTICAL" in node.states]
    require(bars, "no vertical scrollbar")
    return max(bars, key=lambda node: node.extents(window_coords=True)[0])


def inside(control, viewport):
    _, y, _, height = control.extents(window_coords=True)
    return y >= viewport[1] and y + height <= viewport[1] + viewport[3]


@implementation("ref_actions.scroll")
@implementation("ref_actions.scroll_headless_refusal")
def scroll(ctx):
    with managed(ctx, "Zoo Scroll") as app, desktop_restored(ctx):
        tree = root(ctx, app)
        bar = content_bar(tree)
        upper = one(tree, role=7, name="Upper marker")
        lower = one(tree, role=7, name="Lower marker")
        viewport = one(tree, role=49).extents(window_coords=True)
        initial = bar.value()
        read = lambda: (bar.value(), tuple((node.extents(window_coords=True), "CHECKED" in node.states)
                                         for node in (upper, lower)))
        before = read()
        require(inside(upper, viewport) and not inside(lower, viewport), "wrong initial visible rows")
        target = scroll_ref(snapshot(ctx, app))
        ctx.before("atspi.viewport", before)
        try:
            argv = ["scroll", target, "--direction", "down", "--amount", "3"]
            if ctx.mode == "headless":
                no_input(policy_refusal(ctx, argv, read, before))
            else:
                assert_ok(ctx.product(argv))
                require(poll(lambda: bar.value() > initial and not inside(upper, viewport)
                             and inside(lower, viewport), 3), "scroll did not change visible rows")
            after = read()
            require(tuple(bit for _, bit in after[1]) == tuple(bit for _, bit in before[1]),
                    "scroll changed a marker's checked state")
            ctx.after("atspi.viewport", after)
        finally:
            raw_value(bar, initial)
            ctx.restored("atspi.viewport", changed(read, before))


def scroll_ref(data):
    candidates = [item for item in nodes(data.get("tree")) if item.get("role") == "scrollarea"]
    require(len(candidates) == 1 and candidates[0].get("ref_id"), "no unique scroll viewport ref")
    return candidates[0]["ref_id"]


def no_input(response):
    error = response.error or {}
    details = error.get("details", {})
    require(error.get("disposition") == {"delivery": "not_delivered", "retry": "safe"}, response.raw)
    if "delivered_clicks" in details:
        require(details["delivered_clicks"] == 0, "refused scroll sent clicks")
    if "physical_delivery_started" in details:
        require(details["physical_delivery_started"] is False, "refused scroll started physical delivery")


def identity_refusal(response):
    error = response.error or {}
    require(not response.ok and error.get("code") == "AMBIGUOUS_TARGET"
            and error.get("details", {}).get("kind") == "identity_not_unique", response.raw)
    no_input(response)


@implementation("ref_actions.settings_scroll_identity_refusal")
def settings_scroll_identity_refusal(ctx):
    with managed(ctx, "Settings", ("multitasking",)) as app, desktop_restored(ctx):
        tree = root(ctx, app)
        bar = content_bar(tree)
        controls = [one(tree, role=7, name=name) for name in ("Hot Corner", "Dynamic Workspaces")]
        read = lambda: (bar.value(), tuple((node.extents(window_coords=True), "CHECKED" in node.states)
                                         for node in controls))
        before, desktop_before = read(), desktop(ctx.tier)
        ctx.before("atspi.viewport", before)
        succeeded = False
        try:
            data = snapshot(ctx, app)
            candidates = [item["ref_id"] for item in nodes(data.get("tree"))
                          if item.get("ref_id") and item.get("role") == "scrollarea"
                          and any(child.get("name") == "Hot Corner" for child in nodes(item))]
            require(len(candidates) == 1, "no unique Settings content scroll ref")
            identity_refusal(ctx.product(["scroll", candidates[0], "--direction", "down", "--amount", "3"]))
            after = read()
            require(after == before, "identity refusal changed Settings")
            require(desktop(ctx.tier) == desktop_before, "identity refusal moved pointer or focus")
            ctx.after("atspi.viewport", after)
            succeeded = True
        finally:
            try:
                raw_value(bar, before[0])
                ctx.restored("atspi.viewport", changed(read, before))
            except Exception as error:
                message = f"Settings scroll cleanup: {error}"
                if succeeded:
                    raise AssertionError(message) from error
                ctx.finding(message)


@implementation("ref_actions.scroll_to")
def scroll_to(ctx):
    with managed(ctx, "Settings", ("multitasking",)) as app, desktop_restored(ctx):
        tree = root(ctx, app)
        label = "Include apps from each monitor only"
        control = one(tree, role=7, name=label)
        bar = content_bar(tree)
        target = ref(ctx, app, name=label, role="checkbox")
        viewport = bar.extents(window_coords=True)
        def read():
            return bar.value(), control.extents(window_coords=True), "CHECKED" in control.states
        visible = lambda: inside(control, viewport)
        before = read()
        require(not visible(), "scroll-to control must start beyond the viewport")
        ctx.before("atspi.viewport", before)
        try:
            command(ctx, ["scroll-to", target], app)
            require(poll(visible, 3), "scroll-to did not reveal its control")
            after = read()
            require(after[0] != before[0] and after[2] == before[2],
                    "scroll-to did not scroll or changed the setting")
            ctx.after("atspi.viewport", after)
        finally:
            raw_value(bar, before[0])
            ctx.restored("atspi.viewport", changed(read, before))
