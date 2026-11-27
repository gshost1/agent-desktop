use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, ErrorCode, StepMechanism,
    actions::action_request::ActionRequest,
};

mod imp {
    use super::*;
    use crate::actions::activation::activation_surface::PressedSurface;
    use crate::actions::{
        chain::chain_defs,
        chain::chain_delivery::DeliveryOutcome,
        chain::{ChainContext, ChainDef, execute_chain},
        inline_editor_target, toggle_state,
    };
    use crate::tree::AXElement;

    pub(crate) fn perform_action(
        el: &AXElement,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let budget = ChainContext {
            dynamic_value: None,
            verified_point: request.verified_point(),
            deadline,
        };
        crate::tree::properties::attributes::set_messaging_timeout(el, deadline)?;
        budget.ensure_budget()?;
        let action = &request.action;
        let label = action.name();
        let mut steps = Vec::new();
        tracing::debug!("action: perform {label}");
        match action {
            Action::Click => {
                let (role, native_menu) =
                    crate::tree::native_menu::target_role_and_native_menu(el, deadline)?;
                let native_menu = native_menu
                    || (request.policy.is_headed()
                        && role.as_deref() == Some("AXMenuBarItem")
                        && !crate::tree::shell_surface::is_status_item(
                            el,
                            crate::tree::locator_deadline::from_operation(deadline)?,
                        ));
                let definition = chain_defs::click_chain(native_menu);
                let surface = (!native_menu)
                    .then(|| PressedSurface::capture(el, deadline))
                    .flatten();
                steps.extend(
                    if !native_menu
                        && role
                            .as_deref()
                            .is_some_and(crate::actions::select::container_select::is_selection_row)
                    {
                        crate::actions::select::row_selection::click_settled_by_selection(
                            el,
                            definition,
                            &budget,
                            request.policy,
                        )?
                    } else {
                        execute_chain(el, definition, &budget, request.policy)?
                    },
                );
                if let Some(surface) = surface {
                    let result = ActionResult::from_execution(action, steps);
                    return Ok(surface.judge(result, deadline));
                }
            }

            Action::DoubleClick => {
                steps.extend(chain_defs::double_click(el, request, deadline)?);
            }

            Action::RightClick => {
                steps.extend(crate::actions::chain::menu_settle::execute_menu_chain(
                    el,
                    &chain_defs::RIGHT_CLICK_CHAIN,
                    &budget,
                    request.policy,
                )?);
            }

            Action::Toggle => {
                steps.extend(toggle_state::toggle(el, request.policy, deadline)?);
            }

            Action::SetValue(val) => {
                if let Some(editor) = inline_editor_target::editor_for(el, deadline)? {
                    let steps = run_chain(
                        &editor,
                        &chain_defs::SET_VALUE_CHAIN,
                        Some(val),
                        request,
                        deadline,
                    )?;
                    return Ok(awaiting_commit(action, steps));
                }
                let steps = run_chain(
                    el,
                    &chain_defs::SET_VALUE_CHAIN,
                    Some(val),
                    request,
                    deadline,
                )?;
                return Ok(value_write_result(action, el, steps, deadline));
            }

            Action::SetFocus => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::FOCUS_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::TypeText(text) => {
                let editor = inline_editor_target::editor_for(el, deadline)?;
                let step = crate::actions::type_text::execute_type(
                    editor.as_ref().unwrap_or(el),
                    text.as_str(),
                    request.policy,
                    deadline,
                )?;
                if editor.is_some() {
                    return Ok(awaiting_commit(action, vec![step]));
                }
                steps.push(step);
            }

            Action::PressKey(combo) => {
                crate::actions::physical_keyboard::press(el, combo, request.policy, deadline)?;
                steps.push(
                    ActionStep::succeeded("PressKey")
                        .with_mechanism(StepMechanism::PhysicalSynthetic)
                        .with_verified(false),
                );
            }

            Action::Expand => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::EXPAND_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Collapse => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::COLLAPSE_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Select(value) => {
                let outcome = crate::actions::extras::select_value(el, value.as_str(), deadline)?;
                let step = if outcome == DeliveryOutcome::SatisfiedNoDelivery {
                    ActionStep::skipped("Select")
                } else {
                    ActionStep::succeeded("Select")
                };
                steps.push(
                    step.with_mechanism(StepMechanism::SemanticApi)
                        .with_verified(outcome.was_verified()),
                );
            }

            Action::Scroll(direction, amount) => {
                let (mechanism, verified) = crate::actions::scroll::ax_scroll(
                    el,
                    direction,
                    *amount,
                    request.policy,
                    deadline,
                )?;
                steps.push(
                    ActionStep::succeeded("Scroll")
                        .with_mechanism(mechanism)
                        .with_verified(verified),
                );
            }

            Action::Check => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    true,
                    request.policy,
                    deadline,
                )?);
            }

            Action::Uncheck => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    false,
                    request.policy,
                    deadline,
                )?);
            }

            Action::TripleClick => {
                steps.extend(chain_defs::triple_click(el, request, deadline)?);
            }

            Action::ScrollTo => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::SCROLL_TO_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Clear => {
                let probe = crate::actions::clear_probe::read_clear_probe(el, deadline);
                return crate::actions::clear_probe::clear_with(
                    action,
                    &probe,
                    || run_chain(el, &chain_defs::CLEAR_CHAIN, Some(""), request, deadline),
                    || crate::actions::edit_commit::read_edit_commit(el, deadline),
                );
            }

            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    format!(
                        "{} requires adapter-level handling, not element action",
                        label
                    ),
                )
                .with_suggestion("Use the top-level command (e.g. 'hover', 'drag', 'press') instead of targeting an element."));
            }
        }

        Ok(ActionResult::from_execution(action, steps))
    }

    fn value_write_result(
        action: &Action,
        el: &AXElement,
        steps: Vec<ActionStep>,
        deadline: Deadline,
    ) -> ActionResult {
        crate::actions::edit_commit::value_write_result(action, steps, || {
            crate::actions::edit_commit::read_edit_commit(el, deadline)
        })
    }

    fn awaiting_commit(action: &Action, steps: Vec<ActionStep>) -> ActionResult {
        ActionResult::awaiting_commit(action, steps, inline_editor_target::COMMIT_KEY)
    }

    fn run_chain(
        element: &AXElement,
        definition: &ChainDef,
        dynamic_value: Option<&str>,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        execute_chain(
            element,
            definition,
            &ChainContext {
                dynamic_value,
                verified_point: request.verified_point(),
                deadline,
            },
            request.policy,
        )
    }
}

pub(crate) use imp::perform_action;
