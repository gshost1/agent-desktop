use std::ops::ControlFlow;

use super::settled::Settled;
use super::window_op_verdict::{POLL, SETTLE_BUDGET, Verdict, judge};
use super::*;

/// How much of the command deadline the confirming read may take once the
/// confirmation pause is over.
const CONFIRM_READ: Duration = Duration::from_millis(100);

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
    /// A read-back of a move or resize matched as `first`. It holds once a
    /// read a confirmation pause later, inside the command deadline, finds the
    /// same rectangle. A confirming read that matches a shifted rectangle
    /// starts a new confirmation while `limit` lasts.
    Provisional { first: WindowFacts, limit: Deadline },
}

impl Phase {
    fn matched(
        write: &WindowWrite,
        first: WindowFacts,
        limit: Deadline,
        deadline: Deadline,
    ) -> ControlFlow<Settled, Self> {
        if !matches!(write, WindowWrite::MoveResize { .. }) {
            return ControlFlow::Break(Settled::Held(first));
        }
        if deadline.remaining() < CONFIRM_PAUSE + CONFIRM_READ {
            return ControlFlow::Break(Settled::Unconfirmed(first));
        }
        ControlFlow::Continue(Self::Provisional { first, limit })
    }

    fn step(
        self,
        world: &impl OpWorld,
        client: u32,
        write: &WindowWrite,
        deadline: Deadline,
    ) -> ControlFlow<Settled, Self> {
        match self {
            Self::Polling { limit } => {
                let at_limit = limit.is_expired();
                match world.facts(client).ready() {
                    Some(facts) if judge(write, &facts, false) == Verdict::Satisfied => {
                        Self::matched(write, facts, limit, deadline)
                    }
                    _ if at_limit => {
                        std::thread::sleep(POLL.min(deadline.remaining()));
                        ControlFlow::Continue(Self::LastLook { limit })
                    }
                    _ => {
                        std::thread::sleep(POLL.min(limit.remaining()));
                        ControlFlow::Continue(self)
                    }
                }
            }
            Self::LastLook { limit } => match world.facts(client).ready() {
                Some(facts) if judge(write, &facts, true) == Verdict::Satisfied => {
                    Self::matched(write, facts, limit, deadline)
                }
                observed => ControlFlow::Break(Settled::Unheld(observed)),
            },
            Self::Provisional { first, limit } => {
                std::thread::sleep(CONFIRM_PAUSE);
                let latest = world.facts(client).ready();
                match latest {
                    Some(facts) if judge(write, &facts, true) == Verdict::Satisfied => {
                        if deadline.is_expired() {
                            ControlFlow::Break(Settled::Unconfirmed(facts))
                        } else if facts.client == first.client {
                            ControlFlow::Break(Settled::Held(facts))
                        } else if limit.is_expired() {
                            ControlFlow::Break(Settled::Unheld(Some(facts)))
                        } else {
                            Self::matched(write, facts, limit, deadline)
                        }
                    }
                    _ if limit.is_expired() => {
                        ControlFlow::Break(Settled::Unheld(latest.or(Some(first))))
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
