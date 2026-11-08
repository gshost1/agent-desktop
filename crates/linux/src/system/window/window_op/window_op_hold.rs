use super::*;

pub(super) fn confirm_rectangle(
    world: &impl OpWorld,
    client: u32,
    first: WindowFacts,
    deadline: Deadline,
) -> (bool, Option<WindowFacts>) {
    if deadline.remaining() < CONFIRM_PAUSE {
        return (false, Some(first));
    }
    std::thread::sleep(CONFIRM_PAUSE);
    let next = world.facts(client).ready();
    let stable = !deadline.is_expired() && next.is_some_and(|next| next.client == first.client);
    (stable, next)
}

/// Puts the client at `place`, with a second try through a place 40 px away
/// where the manager skips a move to the place it believes the window holds.
pub(super) fn move_client(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    place: Extents,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let back = move_to(place.x, place.y);
    let there = matches!(
        world.facts(client),
        ControlRead::Ready(facts)
            if (facts.client.x - place.x).abs() <= POSITION_TOLERANCE
                && (facts.client.y - place.y).abs() <= POSITION_TOLERANCE
                && confirm_rectangle(world, client, facts, deadline).0
    );
    if !there && apply(world, win, client, &back, deadline).is_err() {
        let _ = apply(
            world,
            win,
            client,
            &move_to(place.x + NUDGE, place.y),
            deadline,
        );
        apply(world, win, client, &back, deadline)?;
    }
    Ok(())
}

/// The place to give back to a window whose live state is `live`. A record
/// the live window no longer matches is stale, so it is dropped and nothing
/// is given back.
pub(super) fn recall_place(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    live: &WindowFacts,
) -> Option<Extents> {
    let record = world.remembered(win, client)?;
    let place = record.place_for(live, &world.displays());
    if place.is_none() {
        world.remember(win, client, None);
    }
    place
}

/// Moves the window back to `place` and drops the record once it lands. A
/// give-back that did not land keeps the place for a retry, tied to where
/// the window was left.
pub(super) fn give_back_place(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    place: Extents,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let moved = move_client(world, win, client, place, deadline);
    if moved.is_ok() {
        world.remember(win, client, None);
    } else {
        keep_for_retry(world, win, client, place);
    }
    moved
}

/// Ties the place to the window's current rectangle after a restore that did
/// not finish, when the window reads back un-maximized and shown. Otherwise
/// the record stays as it was and the next restore checks it again.
pub(super) fn keep_for_retry(world: &impl OpWorld, win: &WindowInfo, client: u32, place: Extents) {
    if let ControlRead::Ready(facts) = world.facts(client)
        && !facts.maximized_vertical
        && !facts.maximized_horizontal
        && !facts.hidden
    {
        let kept = RememberedPlace::Unmaximized {
            before: place,
            left: facts.client,
        };
        if world.remembered(win, client) != Some(kept) {
            world.remember(win, client, Some(kept));
        }
    }
}

/// Whether a window of `size` can sit at `place` on one display. A window the
/// displays cannot hold there is placed by the manager as it likes, so the
/// place it was given is no measure of it. Without a display list the place is
/// taken to hold it.
pub(super) fn holds_place(place: &Extents, size: &Extents, displays: &[Rect]) -> bool {
    displays.is_empty()
        || displays.iter().any(|display| {
            f64::from(place.x) >= display.x
                && f64::from(place.y) >= display.y
                && f64::from(place.x) + f64::from(size.width) <= display.x + display.width
                && f64::from(place.y) + f64::from(size.height) <= display.y + display.height
        })
}

pub(super) fn is_resize(write: &WindowWrite) -> bool {
    matches!(
        write,
        WindowWrite::MoveResize {
            x: None,
            y: None,
            width: Some(_),
            height: Some(_)
        }
    )
}

pub(super) fn move_to(x: i32, y: i32) -> WindowWrite {
    WindowWrite::MoveResize {
        x: Some(x),
        y: Some(y),
        width: None,
        height: None,
    }
}

/// The client rectangle to give back after a maximize, read before it: only
/// a window that is shown and not maximized has one.
pub(super) fn place_before_maximize(world: &impl OpWorld, client: u32) -> Option<Extents> {
    match world.facts(client) {
        ControlRead::Ready(facts)
            if !facts.maximized_vertical && !facts.maximized_horizontal && !facts.hidden =>
        {
            Some(facts.client)
        }
        _ => None,
    }
}

/// Records `before` once the maximize was observed, tied to the maximized
/// rectangle the window was seen at.
pub(super) fn remember_maximized(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    before: Option<Extents>,
    observed: Option<WindowFacts>,
) {
    if let (Some(before), Some(facts)) = (before, observed) {
        let maximized = facts.client;
        world.remember(
            win,
            client,
            Some(RememberedPlace::Maximized { before, maximized }),
        );
    }
}
