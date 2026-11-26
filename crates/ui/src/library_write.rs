// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! One place that runs a library write and handles the result (decisions L3 and L8).
//!
//! A write is one user action. It runs once, with no retry and no queue (rule R8).

use std::future::Future;

use gettextrs::gettext;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::gtk::glib;
use ringo_am_api::{Error, TransportError};
use tracing::warn;

/// HTTP status of a response that says the user is not signed in.
const HTTP_UNAUTHORIZED: u16 = 401;

/// Runs `write` off the main thread, then calls one closure on the main thread.
///
/// `anchor` is a widget of the view. The helper finds the toast overlay of the window from it.
/// On failure, the helper logs the error, calls `on_failure` (to undo an optimistic change), and
/// shows a toast. The toast never shows the Apple error code.
pub fn run<T, W>(
    anchor: &impl IsA<gtk::Widget>,
    write: W,
    on_success: impl FnOnce(T) + 'static,
    on_failure: impl FnOnce() + 'static,
) where
    T: Send + 'static,
    W: Future<Output = ringo_am_api::Result<T>> + Send + 'static,
{
    let toasts = toast_overlay(anchor);
    let task = relm4::spawn(write);
    glib::spawn_future_local(async move {
        match task.await {
            Ok(Ok(value)) => on_success(value),
            Ok(Err(error)) => fail(&error, on_failure, toasts.as_ref()),
            Err(join_error) => {
                warn!(%join_error, "the library write task did not finish");
                on_failure();
                show_toast(toasts.as_ref(), Notice::Generic);
            }
        }
    });
}

/// Handles a failed write: log, undo, toast.
fn fail(error: &Error, on_failure: impl FnOnce(), toasts: Option<&adw::ToastOverlay>) {
    let LogFields { status, apple_code } = LogFields::of(error);
    // Debug output keeps the inner transport error. Its Display text does not.
    warn!(?error, ?status, ?apple_code, "the library write failed");
    on_failure();
    show_toast(toasts, Notice::of(error));
}

/// Finds the toast overlay above `anchor`, while the widget is still in the window.
fn toast_overlay(anchor: &impl IsA<gtk::Widget>) -> Option<adw::ToastOverlay> {
    let overlay = anchor
        .ancestor(adw::ToastOverlay::static_type())
        .and_then(|widget| widget.downcast::<adw::ToastOverlay>().ok());
    if overlay.is_none() {
        warn!("no toast overlay above the widget of a library write");
    }
    overlay
}

fn show_toast(toasts: Option<&adw::ToastOverlay>, notice: Notice) {
    if let Some(toasts) = toasts {
        toasts.add_toast(adw::Toast::new(&notice.text()));
    }
}

/// Text that the toast of a failed write shows.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Notice {
    /// The user is not signed in.
    NotSignedIn,
    /// Any other failure.
    Generic,
}

impl Notice {
    fn of(error: &Error) -> Self {
        match error {
            Error::Transport(
                TransportError::NotAuthorized
                | TransportError::Rejected {
                    status: HTTP_UNAUTHORIZED,
                    ..
                },
            ) => Self::NotSignedIn,
            _ => Self::Generic,
        }
    }

    fn text(self) -> String {
        match self {
            Self::NotSignedIn => gettext("Sign in to change your library."),
            Self::Generic => gettext("Could not change your library. Try again later."),
        }
    }
}

/// What the log line of a failed write holds, beside the error text.
#[derive(Debug, Eq, PartialEq)]
struct LogFields<'a> {
    status: Option<u16>,
    apple_code: Option<&'a str>,
}

impl<'a> LogFields<'a> {
    fn of(error: &'a Error) -> Self {
        match error {
            Error::Transport(TransportError::Rejected { status, apple_code }) => Self {
                status: Some(*status),
                apple_code: apple_code.as_deref(),
            },
            _ => Self {
                status: None,
                apple_code: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(status: u16, apple_code: Option<&str>) -> Error {
        Error::Transport(TransportError::Rejected {
            status,
            apple_code: apple_code.map(str::to_owned),
        })
    }

    #[test]
    fn notice_says_not_signed_in_for_an_unauthorized_error() {
        assert_eq!(Notice::of(&rejected(401, None)), Notice::NotSignedIn);
        assert_eq!(
            Notice::of(&Error::Transport(TransportError::NotAuthorized)),
            Notice::NotSignedIn
        );
    }

    #[test]
    fn notice_is_generic_for_other_errors() {
        assert_eq!(Notice::of(&rejected(500, Some("50000"))), Notice::Generic);
        assert_eq!(Notice::of(&Error::EmptyResponse), Notice::Generic);
    }

    #[test]
    fn notice_text_does_not_show_the_apple_code() {
        let text = Notice::of(&rejected(403, Some("40300"))).text();
        assert!(!text.contains("40300"), "text was: {text}");
    }

    #[test]
    fn log_fields_hold_the_status_and_the_apple_code_of_a_rejection() {
        let error = rejected(403, Some("40300"));
        assert_eq!(
            LogFields::of(&error),
            LogFields {
                status: Some(403),
                apple_code: Some("40300")
            }
        );
    }

    #[test]
    fn log_fields_are_empty_for_an_error_without_a_response() {
        let error = Error::Transport(TransportError::NotReady);
        assert_eq!(
            LogFields::of(&error),
            LogFields {
                status: None,
                apple_code: None
            }
        );
    }
}
