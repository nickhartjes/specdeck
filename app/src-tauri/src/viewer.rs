//! Viewing a shared session as the app sees it: the engine's viewer, with terminal records sent
//! to the window over a binary channel and everything else as events.

use std::time::Duration;

use serde::Serialize;
use specdeck_core::peer::PeerConfig;
use specdeck_core::viewer::{
    CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession, ViewerStatus,
};
use specdeck_protocol::control::{
    Outcome, PROTOCOL_VERSION, Participant, Suggestion, valid_suggestion,
};
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::timeline::TimelineEvent;
use specdeck_protocol::{AgentId, AgentStatus};
use tauri::ipc::{Channel, InvokeResponseBody};

use specdeck_protocol::terminal_stream::encode_records;

/// What a viewer's window hears about.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ViewEvent {
    Joined {
        viewer_id: u32,
        agent: AgentId,
        timeline_supported: bool,
        cols: u16,
        rows: u16,
    },
    Reconnecting,
    Reconnected,
    Timeline {
        event: TimelineEvent,
    },
    Participants {
        participants: Vec<Participant>,
    },
    Status {
        status: AgentStatus,
    },
    /// What the host lets viewers send.
    Allowed {
        suggestions: bool,
    },
    /// A request was numbered; its replies carry this id.
    RequestSent {
        id: u32,
    },
    /// Whether a request got through to the host.
    Reply {
        id: u32,
        outcome: Outcome,
    },
    /// This viewer's suggestions and what became of each.
    Suggestions {
        mine: Vec<Suggestion>,
    },
    /// The session is over for this viewer; `reason` says why, for the window to explain.
    Ended {
        reason: EndReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndReason {
    /// The host closed the session.
    Closed,
    Removed,
    Full,
    InvalidCode,
    TooManyAttempts,
    ServiceUnavailable,
    NetworkBlocked,
    /// This app is older than the host's.
    UpdateApp,
    /// The host's app is older than this one.
    HostUpdate,
}

pub struct ViewerRuntime {
    session: ViewerSession,
}

impl ViewerRuntime {
    pub fn join(
        code: InviteCode,
        username: String,
        signalling_url: String,
        peer: PeerConfig,
        terminal: Channel<InvokeResponseBody>,
        events: Channel<ViewEvent>,
        on_ended: impl Fn(EndReason) + Send + 'static,
    ) -> Self {
        let config = ViewerConfig {
            signalling_url,
            username,
            peer,
            connect_timeout: CONNECT_TIMEOUT,
            protocol_version: PROTOCOL_VERSION,
        };
        let (session, mut viewer_events) = ViewerSession::join(code, config);
        tokio::spawn(async move {
            while let Some(event) = viewer_events.recv().await {
                let sent = match event {
                    ViewerEvent::Records(records) => {
                        terminal.send(InvokeResponseBody::Raw(encode_records(&records))).is_ok()
                    }
                    other => match translate(other) {
                        Some(event) => {
                            if let ViewEvent::Ended { reason } = event {
                                on_ended(reason);
                            }
                            events.send(event).is_ok()
                        }
                        None => true,
                    },
                };
                if !sent {
                    break;
                }
            }
        });
        Self { session }
    }

    pub fn leave(&self) {
        self.session.leave();
    }

    /// What the connection is doing, for the Connection status panel.
    pub fn status(&self) -> ViewerStatus {
        self.session.status().borrow().clone()
    }

    /// Suggests a prompt to the host; refused when the host would refuse it, so a slip in the
    /// window never costs the viewer its connection.
    pub fn suggest(&self, text: String) -> Result<(), String> {
        let text = text.replace("\r\n", "\n");
        if !valid_suggestion(&text) {
            return Err("A suggestion is 1 to 4096 bytes of text.".into());
        }
        self.session.suggest(text);
        Ok(())
    }

    /// Drops the connection and reconnects at once, keeping the viewer's place; refused while
    /// not connected.
    pub fn reconnect(&self) -> Result<(), String> {
        crate::diagnostics::check_reconnect(false, Some(&self.status()))?;
        self.session.interrupt(Duration::ZERO);
        Ok(())
    }
}

fn translate(event: ViewerEvent) -> Option<ViewEvent> {
    use specdeck_protocol::control::RefusalReason as Refusal;
    Some(match event {
        ViewerEvent::Welcome { reconnected: true, .. } => ViewEvent::Reconnected,
        ViewerEvent::Welcome { viewer_id, agent, timeline_supported, cols, rows, .. } => {
            ViewEvent::Joined { viewer_id, agent, timeline_supported, cols, rows }
        }
        ViewerEvent::Records(_) => return None,
        ViewerEvent::Timeline { event, .. } => ViewEvent::Timeline { event },
        ViewerEvent::Participants(participants) => ViewEvent::Participants { participants },
        ViewerEvent::Status(status) => ViewEvent::Status { status },
        ViewerEvent::Reconnecting => ViewEvent::Reconnecting,
        ViewerEvent::Allowed { suggestions } => ViewEvent::Allowed { suggestions },
        ViewerEvent::RequestSent { id } => ViewEvent::RequestSent { id },
        ViewerEvent::Reply { id, outcome } => ViewEvent::Reply { id, outcome },
        ViewerEvent::Suggestions(mine) => ViewEvent::Suggestions { mine },
        ViewerEvent::Ended => ViewEvent::Ended { reason: EndReason::Closed },
        ViewerEvent::Refused(reason) => ViewEvent::Ended {
            reason: match reason {
                Refusal::Removed => EndReason::Removed,
                Refusal::SessionFull => EndReason::Full,
                Refusal::SessionEnded => EndReason::Closed,
                Refusal::ViewerOutdated => EndReason::UpdateApp,
                Refusal::HostOutdated => EndReason::HostUpdate,
            },
        },
        ViewerEvent::InvalidCode => ViewEvent::Ended { reason: EndReason::InvalidCode },
        ViewerEvent::RateLimited => ViewEvent::Ended { reason: EndReason::TooManyAttempts },
        ViewerEvent::ServiceUnavailable => {
            ViewEvent::Ended { reason: EndReason::ServiceUnavailable }
        }
        ViewerEvent::CannotConnect => ViewEvent::Ended { reason: EndReason::NetworkBlocked },
    })
}
