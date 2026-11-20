// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Finds out if the account has an Apple Music subscription. The answer
//! comes from the first API request after the sign-in. The action
//! `app.check-subscription` asks again.

use std::sync::Arc;

use relm4::gtk::gio::SimpleAction;
use relm4::gtk::glib;
use relm4::gtk::prelude::*;
use ringo_am_api::{Error, TransportError, endpoints};
use ringo_core::runner::CoreHandle;
use ringo_core::state::{AuthState, Message};
use tracing::warn;

use crate::services::ApiClient;

/// Starts one check each time the user becomes signed in. It also adds the
/// action that checks again.
pub fn watch(core: &CoreHandle, api: &Arc<ApiClient>) {
    register_recheck_action(core, api);
    let (core, api) = (core.clone(), Arc::clone(api));
    relm4::spawn(async move {
        let mut snapshots = core.subscribe();
        let mut was_signed_in = false;
        loop {
            let signed_in = matches!(
                snapshots.borrow_and_update().auth,
                AuthState::SignedIn { .. }
            );
            if signed_in && !was_signed_in {
                check(&core, api.as_ref()).await;
            }
            was_signed_in = signed_in;
            if snapshots.changed().await.is_err() {
                return;
            }
        }
    });
}

/// Adds the action that checks again. The action is off while a check runs.
fn register_recheck_action(core: &CoreHandle, api: &Arc<ApiClient>) {
    let action = SimpleAction::new("check-subscription", None);
    let (core, api) = (core.clone(), Arc::clone(api));
    action.connect_activate(move |action, _| {
        action.set_enabled(false);
        let (core, api) = (core.clone(), Arc::clone(&api));
        let task = relm4::spawn(async move { check(&core, &api).await });
        let action = action.clone();
        glib::spawn_future_local(async move {
            // A failed task has already logged its error.
            let _ = task.await;
            action.set_enabled(true);
        });
    });
    relm4::main_application().add_action(&action);
}

/// Requests the first page of the library songs. The cache keeps the answer
/// for the library view. The core gets the result.
async fn check(core: &CoreHandle, api: &ApiClient) {
    match endpoints::library_songs(api, 0).await {
        Ok(_) => {
            core.send(Message::SubscriptionChecked { active: true })
                .await;
        }
        Err(error) if is_missing_subscription(&error) => {
            core.send(Message::SubscriptionChecked { active: false })
                .await;
        }
        Err(error) => warn!(%error, "cannot check the subscription"),
    }
}

/// Returns `true` if `error` says that the account has no subscription.
fn is_missing_subscription(error: &Error) -> bool {
    matches!(error, Error::Transport(TransportError::NoSubscription))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subscription_error_is_recognized() {
        let error = Error::Transport(TransportError::NoSubscription);
        assert!(is_missing_subscription(&error));
    }

    #[test]
    fn other_transport_errors_are_not_a_missing_subscription() {
        let error = Error::Transport(TransportError::NotReady);
        assert!(!is_missing_subscription(&error));
    }
}
