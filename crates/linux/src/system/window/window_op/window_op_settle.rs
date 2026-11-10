use std::ops::ControlFlow;

use super::confirmation::Confirmation;
use super::hold::confirm_rectangle;
use super::settled::Settled;
use super::window_op_verdict::{POLL, SETTLE_BUDGET, holds};
use super::*;

/// Where a write stands while the window manager applies it. A read-back that
/// matches a move or resize can be one frame of an animation, so it is only
/// provisional until the rectangle is unchanged a full confirmation pause
/// later.
#[derive(Debug, Clone, Copy)]
enum Phase {
    /// No read-back has matched yet, and reads repeat until `limit`.
    Polling { limit: Deadline },
    /// `limit` ran out without a match; one read a poll later decides.
    LastLook { limit: Deadline },
    /// A read-back of a move or resize matched as `first`, and waits for its
    /// confirmation inside the command deadline. A confirming read that still
    /// matches but shifted starts the next confirmation while `limit` lasts,
    /// and after it reports the place reached but unconfirmed.
    Provisional { first: WindowFacts, limit: Deadline },
}

impl Phase {
    fn matched(
        write: &WindowWrite,
        first: WindowFacts,
        limit: Deadline,
    ) -> ControlFlow<Settled, Self> {
        if matches!(write, WindowWrite::MoveResize { .. }) {
            ControlFlow::Continue(Self::Provisional { first, limit })
        } else {
            ControlFlow::Break(Settled::Held(first))
        }
    }

    fn step(
        self,
        world: &impl OpWorld,
        client: u32,
        write: &WindowWrite,
        deadline: Deadline,
    ) -> ControlFlow<Settled, Self> {
        match self {
            Self::Polling { limit } => match world.facts(client).ready() {
                Some(facts) if holds(write, &facts) => Self::matched(write, facts, limit),
                _ if limit.is_expired() => {
                    std::thread::sleep(POLL.min(deadline.remaining()));
                    ControlFlow::Continue(Self::LastLook { limit })
                }
                _ => {
                    std::thread::sleep(POLL.min(limit.remaining()));
                    ControlFlow::Continue(self)
                }
            },
            Self::LastLook { limit } => match world.facts(client).ready() {
                Some(facts) if holds(write, &facts) => Self::matched(write, facts, limit),
                observed => ControlFlow::Break(Settled::Unheld(observed)),
            },
            Self::Provisional { first, limit } => {
                match confirm_rectangle(world, client, first, deadline) {
                    Confirmation::Stable(facts) => ControlFlow::Break(Settled::Held(facts)),
                    Confirmation::OutOfTime(facts) if holds(write, &facts) => {
                        ControlFlow::Break(Settled::Unconfirmed(facts))
                    }
                    Confirmation::Changed(facts) if holds(write, &facts) => {
                        if limit.is_expired() {
                            ControlFlow::Break(Settled::Unconfirmed(facts))
                        } else {
                            Self::matched(write, facts, limit)
                        }
                    }
                    Confirmation::Unreadable if limit.is_expired() => {
                        ControlFlow::Break(Settled::Unreadable(first))
                    }
                    Confirmation::OutOfTime(facts) | Confirmation::Changed(facts)
                        if limit.is_expired() =>
                    {
                        ControlFlow::Break(Settled::Unheld(Some(facts)))
                    }
                    _ => {
                        std::thread::sleep(POLL.min(limit.remaining()));
                        ControlFlow::Continue(Self::Polling { limit })
                    }
                }
            }
        }
    }
}

/// Reads the window back until the write holds. Reads poll for a first match
/// for the settle budget and one last look after it; any match then waits out
/// its confirmation inside the command deadline.
pub(super) fn settle(
    world: &impl OpWorld,
    client: u32,
    write: &WindowWrite,
    deadline: Deadline,
) -> Settled {
    let mut phase = Phase::Polling {
        limit: deadline.capped(SETTLE_BUDGET),
    };
    loop {
        match phase.step(world, client, write, deadline) {
            ControlFlow::Continue(next) => phase = next,
            ControlFlow::Break(settled) => return settled,
        }
    }
}
