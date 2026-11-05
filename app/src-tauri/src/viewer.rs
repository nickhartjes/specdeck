//! Viewing a shared session as the app sees it: the engine's viewer, with terminal records sent
//! to the window over a binary channel and everything else as events.

use serde::Serialize;
use specdeck_core::peer::PeerConfig;
use specdeck_core::viewer::{CONNECT_TIMEOUT, ViewerConfig, ViewerEvent, ViewerSession};
use specdeck_protocol::control::{PROTOCOL_VERSION, Participant};
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

    pub fn leave(self) {
        self.session.leave();
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
