use super::{
    AXElement, menu_search::find_menu_descendant_with, properties::element::element_for_pid,
    surface_read,
};
use agent_desktop_core::{AdapterError, ErrorCode, SnapshotSurface};
use std::time::Instant;

pub(crate) fn observe<T>(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Instant,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    surface_read::ensure_before_deadline(deadline)?;
    let mut reacquired = false;
    super::resolve::retry_incomplete_until(deadline, || {
        reacquire_once(deadline, &mut reacquired, || {
            require_generation(identity)?;
            let result = operation();
            require_generation(identity)?;
            result
        })
    })
}

fn require_generation(
    identity: crate::system::process::process_identity::ProcessIdentity,
) -> Result<(), AdapterError> {
    if identity.still_matches()? {
        Ok(())
    } else {
        Err(AdapterError::stale_ref_because(
            "Application process instance changed during observation",
        ))
    }
}

fn reacquire_once<T>(
    deadline: Instant,
    reacquired: &mut bool,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    loop {
        surface_read::ensure_before_deadline(deadline)?;
        match operation() {
            Err(mut error) if invalidated(&error) => {
                if !*reacquired {
                    *reacquired = true;
                    continue;
                }
                error.code = ErrorCode::AppUnresponsive;
                let mut details = error
                    .details
                    .take()
                    .unwrap_or_else(|| serde_json::json!({}));
                if let Some(fields) = details.as_object_mut() {
                    fields.insert("complete".into(), false.into());
                    fields.insert("retryable".into(), false.into());
                    fields.insert("menu_root_reacquired".into(), true.into());
                }
                return Err(error.with_details(details));
            }
            result => return result,
        }
    }
}

pub(super) fn invalidated(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details["ax_error"].as_i64())
        == Some(i64::from(accessibility_sys::kAXErrorInvalidUIElement))
}

pub(crate) fn root_for_pid(
    pid: i32,
    surface: SnapshotSurface,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    match surface {
        SnapshotSurface::Menubar => menubar_for_pid(pid, deadline),
        SnapshotSurface::Menu => menu_element_for_pid(pid, deadline),
        _ => Err(AdapterError::not_supported("native menu surface")),
    }
}

pub(super) fn require_root<T>(
    read: Result<Option<T>, AdapterError>,
    surface: SnapshotSurface,
    message: &str,
) -> Result<T, AdapterError> {
    read?.ok_or_else(|| AdapterError::surface_absent(surface, message))
}

#[cfg(target_os = "macos")]
fn menubar_for_pid(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    let Some(menubar) = surface_read::element(&app, "AXMenuBar", deadline)? else {
        return Ok(None);
    };
    if !has_role(&menubar, "AXMenuBar", deadline)? {
        return Err(invalid_root_error());
    }
    Ok(Some(menubar))
}

#[cfg(target_os = "macos")]
fn menu_element_for_pid(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    Ok(open_menus_for_pid(pid, deadline)?
        .into_iter()
        .next()
        .map(|(_, _, menu)| menu))
}

pub(crate) fn open_menus_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Vec<(&'static str, String, AXElement)>, AdapterError> {
    let mut menus = Vec::new();
    if let Some(menubar) = menubar_for_pid(pid, deadline)? {
        menus.extend(
            open_menubar_menus_with(
                &menubar,
                |element| surface_read::elements(element, "AXChildren", deadline),
                |element| {
                    Ok(has_role(element, "AXMenuBarItem", deadline)?
                        && surface_read::boolean(element, "AXSelected", deadline)? == Some(true))
                },
                |element| has_role(element, "AXMenu", deadline),
            )?
            .into_iter()
            .map(|(id, menu)| ("menu", id, menu)),
        );
    }
    if menus.is_empty()
        && let Some((id, menu)) = context_menu_from_app(pid, deadline)?
    {
        menus.push(("context_menu", id, menu));
    }
    Ok(menus)
}

fn open_menubar_menus_with<T>(
    menubar: &T,
    mut children: impl FnMut(&T) -> Result<Vec<T>, AdapterError>,
    mut selected: impl FnMut(&T) -> Result<bool, AdapterError>,
    mut is_menu: impl FnMut(&T) -> Result<bool, AdapterError>,
) -> Result<Vec<(String, T)>, AdapterError> {
    let mut menus = Vec::new();
    for (item_index, item) in children(menubar)?.into_iter().enumerate() {
        if !selected(&item)? {
            continue;
        }
        for (child_index, child) in children(&item)?.into_iter().enumerate() {
            if is_menu(&child)? {
                menus.push((
                    format!("app/menubar/children/{item_index}/children/{child_index}"),
                    child,
                ));
            }
        }
    }
    Ok(menus)
}

#[cfg(target_os = "macos")]
fn context_menu_from_app(
    pid: i32,
    deadline: Instant,
) -> Result<Option<(String, AXElement)>, AdapterError> {
    let app = element_for_pid(pid);
    for (index, menu) in surface_read::elements(&app, "AXMenus", deadline)?
        .into_iter()
        .enumerate()
    {
        if displayed_menu(&menu, deadline)? {
            return Ok(Some((format!("app/menus/{index}"), menu)));
        }
    }
    let focused = surface_read::element(&app, "AXFocusedUIElement", deadline)?;
    if let Some(focused) = &focused
        && let Some(menu) = find_menu_descendant(
            focused.clone(),
            "app/focused-element".into(),
            deadline,
            None,
        )?
    {
        return Ok(Some(menu));
    }
    for (index, child) in surface_read::elements(&app, "AXChildren", deadline)?
        .into_iter()
        .enumerate()
    {
        let source = format!("app/children/{index}");
        if let Some(menu) = find_menu_descendant(child, source, deadline, focused.as_ref())? {
            return Ok(Some(menu));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn find_menu_descendant(
    root: AXElement,
    source: String,
    deadline: Instant,
    searched: Option<&AXElement>,
) -> Result<Option<(String, AXElement)>, AdapterError> {
    let names = super::menu_search_node::attribute_names();
    let inspect =
        |element: &AXElement| match super::menu_search_node::read(element, &names, deadline)? {
            Some(node) => Ok((node.is_menu, node.children)),
            None => inspect_by_attribute(element, deadline),
        };
    let already_searched = |element: &AXElement| {
        searched.is_some_and(|searched| super::same_element(element, searched))
    };
    find_menu_descendant_with(
        root,
        source,
        deadline,
        super::menu_search::outside_searched(already_searched, inspect),
    )
}

#[cfg(target_os = "macos")]
fn inspect_by_attribute(
    element: &AXElement,
    deadline: Instant,
) -> Result<(bool, Vec<AXElement>), AdapterError> {
    let role = surface_read::string(element, "AXRole", deadline)?;
    if role.as_deref() == Some("AXMenuBar") {
        return Ok((false, Vec::new()));
    }
    if role.as_deref() == Some("AXMenu")
        && surface_read::boolean(element, "AXVisible", deadline)? != Some(false)
    {
        return Ok((true, Vec::new()));
    }
    Ok((
        false,
        surface_read::elements(element, "AXChildren", deadline)?,
    ))
}

#[cfg(target_os = "macos")]
fn displayed_menu(element: &AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    Ok(has_role(element, "AXMenu", deadline)?
        && surface_read::boolean(element, "AXVisible", deadline)? != Some(false))
}

#[cfg(target_os = "macos")]
fn has_role(element: &AXElement, expected: &str, deadline: Instant) -> Result<bool, AdapterError> {
    Ok(surface_read::string(element, "AXRole", deadline)?.as_deref() == Some(expected))
}

fn invalid_root_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Application returned an invalid menu bar root",
    )
    .with_details(serde_json::json!({
        "kind": "surface_root_invalid", "surface": "menubar", "complete": false,
        "retryable": false,
    }))
}

#[cfg(test)]
#[path = "app_menu_tests.rs"]
mod tests;
