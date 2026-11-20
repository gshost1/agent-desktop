from oracle import gsettings
from runner.helper import changed, command, managed, one, ref, require, root
from runner.legs import implementation
from runner.verdict import LegContext


def select_contract(ctx, *, absent=False):
    schema, key = "org.gnome.desktop.session", "idle-delay"
    initial = gsettings.get(ctx.tier, schema, key)
    with managed(ctx, "Settings", ("power",)) as app:
        tree = root(ctx, app)
        combos = tree.find(role=11)
        candidates = [node for node in combos if "Screen Blank" in node.name]
        require(len(candidates) == 1, "Screen Blank combo missing or ambiguous")
        combo = candidates[0]
        target = ref(ctx, app, role="combobox", name=combo.name)
        ctx.before("gsettings", initial)
        try:
            if absent:
                response = command(ctx, ["select", target, "absent-option"], app, code="ACTION_NOT_SUPPORTED")
                require("Select" in response.error["message"] or "semantic" in response.error["message"],
                        response.raw)
                expected = initial
            else:
                label, wanted = ("5 minutes", "uint32 300") if initial == "uint32 0" else ("Never", "uint32 0")
                headless = ctx.mode == "headless"
                response = command(ctx, ["select", target, label], app,
                                   code="POLICY_DENIED" if headless else None)
                if headless:
                    details = response.error.get("details", {})
                    require(details.get("kind") == "pointer_required"
                            and details.get("physical_delivery_started") is False, response.raw)
                    require(response.error.get("disposition") == {"delivery": "not_delivered", "retry": "safe"},
                            response.raw)
                else:
                    require((response.data or {}).get("disposition") == {
                        "delivery": "delivered_verified", "retry": "unsafe"}, response.raw)
                expected = initial if headless else wanted
            changed(lambda: gsettings.get(ctx.tier, schema, key), expected, floor=0.3)
            ctx.after("gsettings", gsettings.get(ctx.tier, schema, key))
        finally:
            if not absent and gsettings.get(ctx.tier, schema, key) != initial:
                gsettings.set(ctx.tier, schema, key, initial)
                changed(lambda: gsettings.get(ctx.tier, schema, key), initial)
            require(gsettings.get(ctx.tier, schema, key) == initial)
            ctx.restored("gsettings", gsettings.get(ctx.tier, schema, key))


@implementation("select.screen_blank_value")
def screen_blank_value(ctx: LegContext) -> None:
    select_contract(ctx)


@implementation("select.screen_blank_headless_refusal")
def screen_blank_headless_refusal(ctx: LegContext) -> None:
    select_contract(ctx)


@implementation("select.refusal_without_semantic_route")
def refusal_without_semantic_route(ctx: LegContext) -> None:
    select_contract(ctx, absent=True)
