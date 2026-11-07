use agent_desktop_core::{AdapterError, Deadline};

use super::element_call::ElementCall;
use super::remote_element::{call_function, with_element};
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, ElementReading, ElementToken, PageSession};

pub(crate) const READ_STATE: &str = r"function() {
  const checkable = this.localName === 'input' && (this.type === 'checkbox' || this.type === 'radio');
  const aria = this.getAttribute ? this.getAttribute('aria-checked') : null;
  const raw = this.innerText !== undefined ? this.innerText : this.textContent;
  return {
    value: ('value' in this && this.value !== undefined && this.value !== null) ? String(this.value) : null,
    text: raw ? String(raw).slice(0, 10000) : null,
    checked: checkable ? this.checked : (aria === null ? null : aria === 'true'),
  };
}";

/// Reads the node's live value, text and checked state.
pub fn read_element(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    deadline: Deadline,
) -> Result<ElementReading, AdapterError> {
    with_element(
        connection,
        session,
        token,
        deadline,
        |connection, object| read_object(connection, session, object, deadline),
    )
}

/// Reads an already resolved node's value, text and checked state.
pub(crate) fn read_object(
    connection: &mut CdpConnection,
    session: &PageSession,
    object_id: &str,
    deadline: Deadline,
) -> Result<ElementReading, AdapterError> {
    let value = call_function(
        connection,
        session,
        object_id,
        ElementCall {
            function: READ_STATE,
            arguments: Vec::new(),
        },
        deadline,
    )?;
    serde_json::from_value(value)
        .map_err(|error| transport_failed("Runtime.callFunctionOn", error.to_string()))
}
