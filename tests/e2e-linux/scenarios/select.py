from oracle import gsettings
from runner.helper import changed, command, managed, ref, require, root
from runner.legs import implementation
from runner.verdict import LegContext

SCHEMA, KEY = "org.gnome.desktop.session", "idle-delay"
NEVER, FIVE_MINUTES = "uint32 0", "uint32 300"


def screen_blank(ctx, app):
    tree = root(ctx, app)
    candidates = [node for node in tree.find(role=11) if "Screen Blank" in node.name]
    require(len(candidates) == 1, "Screen Blank combo missing or ambiguous")
    return ref(ctx, app, role="combobox", name=candidates[0].name)


def read(ctx):
    return gsettings.get(ctx.tier, SCHEMA, KEY)


def refused_headless(ctx, response):
    """Headless: the dropdown has no semantic opener, so the select needs the pointer and the
    interaction policy refuses it before anything is sent (skill: capabilities, pointer_required)."""
    error = response.error or {}
    require(error.get("code") == "POLICY_DENIED"
            and (error.get("details") or {}).get("kind") == "pointer_required"
            and (error.get("disposition") or {}).get("delivery") == "not_delivered", response.raw)


def select_contract(ctx, *, absent=False):
    initial = read(ctx)
    with managed(ctx, "Settings", ("power",)) as app:
        target = screen_blank(ctx, app)
        ctx.before("gsettings", initial)
        headed = ctx.mode == "headed"
        try:
            if not headed:
                response = command(ctx, ["select", target, "absent-option" if absent else "Never"],
                                   app, code="POLICY_DENIED")
                refused_headless(ctx, response)
                ctx.after("gsettings", changed(lambda: read(ctx), initial, floor=0.3))
            elif absent:
                response = command(ctx, ["select", target, "absent-option"], app, code="ELEMENT_NOT_FOUND")
                details = (response.error or {}).get("details") or {}
                require(details.get("searched") is True and details.get("popup_closed") is True,
                        response.raw)
                ctx.after("gsettings", changed(lambda: read(ctx), initial, floor=0.3))
            else:
                choice, expected = ("5 minutes", FIVE_MINUTES) if initial == NEVER else ("Never", NEVER)
                response = command(ctx, ["select", target, choice], app)
                require((response.data or {}).get("disposition", {}).get("delivery")
                        in ("delivered_verified", "delivered_unverified"), response.raw)
                ctx.after("gsettings", changed(lambda: read(ctx), expected))
        finally:
            if read(ctx) != initial:
                gsettings.set(ctx.tier, SCHEMA, KEY, initial.split()[-1])
            ctx.restored("gsettings", changed(lambda: read(ctx), initial))


@implementation("select.screen_blank_value")
def screen_blank_value(ctx: LegContext) -> None:
    select_contract(ctx)


@implementation("select.refusal_without_semantic_route")
def refusal_without_semantic_route(ctx: LegContext) -> None:
    select_contract(ctx, absent=True)
