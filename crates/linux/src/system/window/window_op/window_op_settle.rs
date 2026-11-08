use std::ops::ControlFlow;

use super::hold::confirm_rectangle;
use super::window_op_verdict::{POLL, SETTLE_BUDGET, judge};
use super::*;

/// How long the confirming read may take once the confirmation pause is over
/// and still count as part of the settle.
const CONFIRM_READ: Duration = Duration::from_millis(100);

/// Where a write stands while the window manager applies it. A read-back that
/// matches a move or resize can be one frame of an animation, so it is only
/// provisional until the rectangle is unchanged a full confirmation pause
/// later.
#[derive(Debug, Clone, Copy)]
enum Phase {
    /// No read-back has matched yet. A match must be seen before `limit`.
    Pending { limit: Deadline },
    /// A read-back of a move or resize matched as `first`. It holds once a
    /// read taken a confirmation pause later, before `until`, finds the same
    /// rectangle. `until` reaches past `limit` when the match came late in it,
    /// and never past the command deadline.
    Provisional {
        first: WindowFacts,
        until: Deadline,
        limit: Deadline,
    },
}

impl Phase {
    fn provisional(
        first: WindowFacts,
        limit: Deadline,
        deadline: Deadline,
    ) -> ControlFlow<(Verdict, Option<WindowFacts>), Self> {
        let confirmation = deadline.capped(CONFIRM_PAUSE + CONFIRM_READ);
        let until = if confirmation.remaining() > limit.remaining() {
            confirmation
        } else {
            limit
        };
        if until.remaining() < CONFIRM_PAUSE {
            return ControlFlow::Break((Verdict::Mismatch, Some(first)));
        }
        ControlFlow::Continue(Self::Provisional {
            first,
            until,
            limit,
        })
    }

    fn step(
        self,
        world: &impl OpWorld,
        client: u32,
        write: &WindowWrite,
        deadline: Deadline,
    ) -> ControlFlow<(Verdict, Option<WindowFacts>), Self> {
        match self {
            Self::Pending { limit } => {
                let observed = world.facts(client).ready();
                let at_limit = limit.is_expired();
                let verdict = match observed {
                    Some(facts) => judge(write, &facts, at_limit),
                    None if at_limit => Verdict::Mismatch,
                    None => Verdict::Pending,
                };
                match (verdict, observed) {
                    (Verdict::Satisfied, Some(first))
                        if matches!(write, WindowWrite::MoveResize { .. }) =>
                    {
                        if at_limit {
                            ControlFlow::Break((Verdict::Mismatch, observed))
                        } else {
                            Self::provisional(first, limit, deadline)
                        }
                    }
                    (Verdict::Pending, _) => {
                        std::thread::sleep(POLL.min(limit.remaining()));
                        ControlFlow::Continue(self)
                    }
                    _ => ControlFlow::Break((verdict, observed)),
                }
            }
            Self::Provisional {
                first,
                until,
                limit,
            } => {
                let (stable, latest) = confirm_rectangle(world, client, first, until);
                match latest {
                    Some(facts) if stable && judge(write, &facts, true) == Verdict::Satisfied => {
                        ControlFlow::Break((Verdict::Satisfied, Some(facts)))
                    }
                    _ if limit.is_expired() => ControlFlow::Break((Verdict::Mismatch, latest)),
                    _ => {
                        std::thread::sleep(POLL.min(limit.remaining()));
                        ControlFlow::Continue(Self::Pending { limit })
                    }
                }
            }
        }
    }
}

/// Reads the window back until the write holds or the settle budget, plus a
/// confirmation pause for a match that came late in it, runs out.
pub(super) fn settle(
    world: &impl OpWorld,
    client: u32,
    write: &WindowWrite,
    deadline: Deadline,
) -> (Verdict, Option<WindowFacts>) {
    let mut phase = Phase::Pending {
        limit: deadline.capped(SETTLE_BUDGET),
    };
    loop {
        match phase.step(world, client, write, deadline) {
            ControlFlow::Continue(next) => phase = next,
            ControlFlow::Break(settled) => return settled,
        }
    }
}
