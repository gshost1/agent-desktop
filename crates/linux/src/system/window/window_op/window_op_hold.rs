use super::confirmation::Confirmation;
use super::*;

/// Reads the window again a full confirmation pause after `first` matched,
/// inside the deadline. This is the one place the pause and its deadline rule
/// live, for a move, a resize and a maximize alike.
pub(super) fn confirm_rectangle(
    world: &impl OpWorld,
    client: u32,
    first: WindowFacts,
    deadline: Deadline,
) -> Confirmation {
    if deadline.remaining() < CONFIRM_PAUSE {
        return Confirmation::OutOfTime(first);
    }
    std::thread::sleep(CONFIRM_PAUSE);
    match world.facts(client).ready() {
        None => Confirmation::Unreadable,
        Some(next) if deadline.is_expired() => Confirmation::OutOfTime(next),
        Some(next) if next.client == first.client => Confirmation::Stable(next),
        Some(next) => Confirmation::Changed(next),
    }
}

/// Puts the client at `place`, with a second try through a place 40 px away
/// where the manager skips a move to the place it believes the window holds.
/// A window that reached the place but could not be confirmed there is not
/// nudged away from it.
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
                && matches!(confirm_rectangle(world, client, facts, deadline), Confirmation::Stable(_))
    );
    if there {
        return Ok(());
    }
    match attempt(world, win, client, &back, deadline) {
        Ok(reached @ (Settled::Held(_) | Settled::Unconfirmed(_))) => {
            outcome(world, client, &back, reached, deadline)
        }
        _ => {
            let _ = apply(
                world,
                win,
                client,
                &move_to(place.x + NUDGE, place.y),
                deadline,
            );
            apply(world, win, client, &back, deadline)
        }
    }
}

/// The place to give back to a window whose live state is `live`. A record
/// a shown window no longer matches is stale and is dropped; a hidden read
/// may be a transition, so it keeps the record and gives nothing back.
pub(super) fn recall_place(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    live: &WindowFacts,
) -> Option<Extents> {
    let record = world.remembered(win, client)?;
    let place = record.place_for(live, &world.displays());
    if place.is_none() && !live.hidden {
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

/// Records the place to give back before a maximize is written, replacing
/// any record of an earlier cycle, and says whether a pending record now
/// waits for confirmation. A window already maximized keeps a record of the
/// current cycle, so a retried maximize does not lose the place; a hidden or
/// unreadable window gets no record.
pub(super) fn record_before_maximize(world: &impl OpWorld, win: &WindowInfo, client: u32) -> bool {
    let record = world.remembered(win, client);
    match world.facts(client) {
        ControlRead::Ready(facts) if facts.maximized_vertical || facts.maximized_horizontal => {
            match record {
                Some(RememberedPlace::Pending { .. }) => true,
                Some(RememberedPlace::Maximized { .. }) => false,
                Some(RememberedPlace::Unmaximized { .. }) => {
                    world.remember(win, client, None);
                    false
                }
                None => false,
            }
        }
        ControlRead::Ready(facts) if !facts.hidden => {
            let before = RememberedPlace::carried(record, &facts.client);
            world.remember(win, client, Some(RememberedPlace::Pending { before }));
            true
        }
        _ => {
            if record.is_some() {
                world.remember(win, client, None);
            }
            false
        }
    }
}

/// Promotes a pending record once the maximized rectangle held through a
/// full confirmation pause; a rectangle still moving leaves it pending.
pub(super) fn confirm_maximized(
    world: &impl OpWorld,
    win: &WindowInfo,
    client: u32,
    first: WindowFacts,
    deadline: Deadline,
) {
    let Some(RememberedPlace::Pending { before }) = world.remembered(win, client) else {
        return;
    };
    if let Confirmation::Stable(facts) = confirm_rectangle(world, client, first, deadline)
        && facts.maximized_vertical
        && facts.maximized_horizontal
    {
        let maximized = facts.client;
        world.remember(
            win,
            client,
            Some(RememberedPlace::Maximized { before, maximized }),
        );
    }
}
