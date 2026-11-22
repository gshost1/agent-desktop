//! The native reads window resolution combines: the exact CoreGraphics
//! record, the application's `AXWindows` state, Space membership and the
//! accessibility element. Resolution takes them through [`WindowSources`] so
//! its decisions are testable without a live window server.

use agent_desktop_core::AdapterError;
use std::time::Instant;

use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_ax_state::WindowAxState;
use crate::tree::AXElement;

pub(crate) trait WindowSources {
    fn record(
        &self,
        window_number: i64,
        deadline: Instant,
    ) -> Result<Option<WindowRecord>, AdapterError>;

    fn ax_state(&self, pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError>;

    fn is_on_any_space(&self, window_number: i64) -> Option<bool>;

    fn element(
        &self,
        pid: i32,
        window_number: i64,
        fallback_title: Option<&str>,
        deadline: Instant,
    ) -> Result<Option<AXElement>, AdapterError>;
}

pub(crate) struct LiveWindowSources;

impl WindowSources for LiveWindowSources {
    fn record(
        &self,
        window_number: i64,
        deadline: Instant,
    ) -> Result<Option<WindowRecord>, AdapterError> {
        crate::system::window::cg_window_exact::exact_window_record_until(window_number, deadline)
    }

    fn ax_state(&self, pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError> {
        crate::system::window::window_ax_state::read_until(pid, deadline)
    }

    fn is_on_any_space(&self, window_number: i64) -> Option<bool> {
        crate::system::window::window_space::is_on_any_space(window_number)
    }

    fn element(
        &self,
        pid: i32,
        window_number: i64,
        fallback_title: Option<&str>,
        deadline: Instant,
    ) -> Result<Option<AXElement>, AdapterError> {
        crate::system::window::window_element_lookup::ax_window_element_for_number(
            pid,
            window_number,
            fallback_title,
            deadline,
        )
    }
}
