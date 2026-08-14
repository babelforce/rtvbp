use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::{ControlChannel, ControlFrame, Envelope, FrameKind};

pub(super) const OFFER_METHOD: &str = "transport.webrtc.offer";
const MAX_SIGNAL_FRAME_LEN: usize = 1 << 20;
const MAX_SDP_LEN: usize = 512 << 10;
static SIGNAL_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize, Deserialize)]
struct Description {
    sdp: String,
}

pub(super) async fn negotiate_offer(
    control: Arc<dyn ControlChannel>,
    envelope: Arc<dyn Envelope>,
    peer: &RTCPeerConnection,
) -> Result<(), crate::Error> {
    let offer = peer.create_offer(None).await.map_err(transport_error)?;
    let mut gathering = peer.gathering_complete_promise().await;
    peer.set_local_description(offer)
        .await
        .map_err(transport_error)?;
    // The promise completes by either sending or dropping its one-shot sender.
    let _ = gathering.recv().await;
    let local = peer
        .local_description()
        .await
        .ok_or_else(|| crate::Error::Transport("local WebRTC offer is missing".to_owned()))?;
    let payload = encode_description(&local.sdp)?;
    let id = format!(
        "webrtc-offer-{}",
        SIGNAL_IDS.fetch_add(1, Ordering::Relaxed)
    );
    let encoded = envelope.encode(&ControlFrame::request(
        id.clone(),
        OFFER_METHOD,
        Some(payload),
    ))?;
    validate_signal(&encoded)?;
    control.send(encoded).await?;

    let received = control.recv().await?;
    let frame = decode_signal(envelope.as_ref(), &received.data)?;
    if frame.kind != FrameKind::Response || frame.correlation_id != id {
        return Err(crate::Error::Transport(format!(
            "unexpected WebRTC answer kind={:?} correlation={:?}",
            frame.kind, frame.correlation_id
        )));
    }
    if let Some(error) = frame.error {
        return Err(crate::Error::Remote(error));
    }
    let answer = decode_description(frame.payload)?;
    peer.set_remote_description(RTCSessionDescription::answer(answer.sdp).map_err(transport_error)?)
        .await
        .map_err(transport_error)
}

pub(super) async fn negotiate_answer(
    control: Arc<dyn ControlChannel>,
    envelope: Arc<dyn Envelope>,
    peer: &RTCPeerConnection,
) -> Result<(), crate::Error> {
    let received = control.recv().await?;
    let frame = decode_signal(envelope.as_ref(), &received.data)?;
    if frame.kind != FrameKind::Request || frame.method != OFFER_METHOD || frame.id.is_empty() {
        return Err(crate::Error::Transport(format!(
            "unexpected WebRTC offer kind={:?} method={:?} id={:?}",
            frame.kind, frame.method, frame.id
        )));
    }
    let offer = decode_description(frame.payload)?;
    peer.set_remote_description(RTCSessionDescription::offer(offer.sdp).map_err(transport_error)?)
        .await
        .map_err(transport_error)?;
    let answer = peer.create_answer(None).await.map_err(transport_error)?;
    let mut gathering = peer.gathering_complete_promise().await;
    peer.set_local_description(answer)
        .await
        .map_err(transport_error)?;
    let _ = gathering.recv().await;
    let local = peer
        .local_description()
        .await
        .ok_or_else(|| crate::Error::Transport("local WebRTC answer is missing".to_owned()))?;
    let payload = encode_description(&local.sdp)?;
    let encoded = envelope.encode(&ControlFrame::response(frame.id, Some(payload), None))?;
    validate_signal(&encoded)?;
    control.send(encoded).await
}

fn encode_description(sdp: &str) -> Result<serde_json::Value, crate::Error> {
    validate_sdp(sdp)?;
    serde_json::to_value(Description {
        sdp: sdp.to_owned(),
    })
    .map_err(crate::Error::envelope)
}

fn decode_description(payload: Option<serde_json::Value>) -> Result<Description, crate::Error> {
    let description: Description = serde_json::from_value(
        payload
            .ok_or_else(|| crate::Error::Transport("WebRTC SDP payload is missing".to_owned()))?,
    )
    .map_err(crate::Error::envelope)?;
    validate_sdp(&description.sdp)?;
    Ok(description)
}

fn decode_signal(envelope: &dyn Envelope, encoded: &[u8]) -> Result<ControlFrame, crate::Error> {
    validate_signal(encoded)?;
    envelope.decode(encoded)
}

fn validate_signal(encoded: &[u8]) -> Result<(), crate::Error> {
    if encoded.is_empty() || encoded.len() > MAX_SIGNAL_FRAME_LEN {
        Err(crate::Error::Transport(
            "WebRTC signaling frame size is invalid".to_owned(),
        ))
    } else {
        Ok(())
    }
}

fn validate_sdp(sdp: &str) -> Result<(), crate::Error> {
    if sdp.is_empty() || sdp.len() > MAX_SDP_LEN {
        Err(crate::Error::Transport(
            "WebRTC SDP size is invalid".to_owned(),
        ))
    } else {
        Ok(())
    }
}

fn transport_error(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::Transport(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_and_sdp_bounds_are_fail_closed() {
        assert!(validate_signal(&[]).is_err());
        assert!(validate_signal(&vec![0; MAX_SIGNAL_FRAME_LEN + 1]).is_err());
        assert!(validate_sdp("").is_err());
        assert!(validate_sdp(&"x".repeat(MAX_SDP_LEN + 1)).is_err());
        assert!(decode_description(Some(serde_json::json!({"sdp": ""}))).is_err());
        assert!(decode_description(Some(serde_json::json!({"other": "x"}))).is_err());
    }
}
