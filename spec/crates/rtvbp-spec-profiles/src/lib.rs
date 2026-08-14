#![forbid(unsafe_code)]

use rtvbp_spec_model::{
    CatalogId, ControlCarrier, MediaCarrier, MediaFormatSpec, NegotiationSpec,
    NegotiationTransport, ProfileMediaSpec, ProfileRegistry, ProfileSpec, SignalingSpec,
    TransportSpec,
};

pub const CLASSIC_PROFILE: &str = "rtvbp.v1";
pub const DEMO_PROFILE: &str = "rtvbp.demo.v1";
pub const WEBRTC_PROFILE: &str = "rtvbp.webrtc.v1";

pub const WEBSOCKET_TRANSPORT: &str = "ws.v1";
pub const WEBRTC_WEBSOCKET_TRANSPORT: &str = "webrtcws.v1";
pub const CLASSIC_ENVELOPE: &str = "classic.v1";

pub const L16_SDK_FORMAT: &str = "l16-8000-16-1-20ms";
pub const PCMU_WIRE_FORMAT: &str = "pcmu-8000-8-1-20ms";
pub const WEBRTC_OFFER_METHOD: &str = "transport.webrtc.offer";

/// The current public transport/profile declaration consumed by every generator target.
#[must_use]
pub fn registry() -> ProfileRegistry {
    ProfileRegistry {
        transports: vec![
            TransportSpec {
                id: WEBSOCKET_TRANSPORT.to_owned(),
                description:
                    "Semantic control in WebSocket text frames with optional binary media."
                        .to_owned(),
                control: ControlCarrier::WebSocketText,
                media_carriers: vec![MediaCarrier::WebSocketBinary],
            },
            TransportSpec {
                id: WEBRTC_WEBSOCKET_TRANSPORT.to_owned(),
                description: "Semantic control on WebSocket with RTP media on WebRTC.".to_owned(),
                control: ControlCarrier::WebSocketText,
                media_carriers: vec![MediaCarrier::WebRtcRtp],
            },
        ],
        media_formats: vec![
            MediaFormatSpec {
                id: L16_SDK_FORMAT.to_owned(),
                encoding: "L16".to_owned(),
                sample_rate: 8_000,
                bit_depth: 16,
                channels: 1,
                packet_time_ms: 20,
            },
            MediaFormatSpec {
                id: PCMU_WIRE_FORMAT.to_owned(),
                encoding: "PCMU".to_owned(),
                sample_rate: 8_000,
                bit_depth: 8,
                channels: 1,
                packet_time_ms: 20,
            },
        ],
        signaling: vec![SignalingSpec {
            method: WEBRTC_OFFER_METHOD.to_owned(),
            transport: WEBRTC_WEBSOCKET_TRANSPORT.to_owned(),
            description: "Exchange a complete non-trickle SDP offer and correlated answer."
                .to_owned(),
        }],
        profiles: vec![
            ProfileSpec {
                id: CLASSIC_PROFILE.to_owned(),
                negotiation_token: CLASSIC_PROFILE.to_owned(),
                transport: WEBSOCKET_TRANSPORT.to_owned(),
                envelope: CLASSIC_ENVELOPE.to_owned(),
                catalog: CatalogId::new("babelforce", 1),
                signaling: Vec::new(),
                media: vec![ProfileMediaSpec {
                    channel: "audio".to_owned(),
                    carrier: MediaCarrier::WebSocketBinary,
                    wire_format: L16_SDK_FORMAT.to_owned(),
                    sdk_format: L16_SDK_FORMAT.to_owned(),
                }],
            },
            ProfileSpec {
                id: DEMO_PROFILE.to_owned(),
                negotiation_token: DEMO_PROFILE.to_owned(),
                transport: WEBSOCKET_TRANSPORT.to_owned(),
                envelope: CLASSIC_ENVELOPE.to_owned(),
                catalog: CatalogId::new("demo", 1),
                signaling: Vec::new(),
                media: Vec::new(),
            },
            ProfileSpec {
                id: WEBRTC_PROFILE.to_owned(),
                negotiation_token: WEBRTC_PROFILE.to_owned(),
                transport: WEBRTC_WEBSOCKET_TRANSPORT.to_owned(),
                envelope: CLASSIC_ENVELOPE.to_owned(),
                catalog: CatalogId::new("babelforce", 1),
                signaling: vec![WEBRTC_OFFER_METHOD.to_owned()],
                media: vec![ProfileMediaSpec {
                    channel: "audio".to_owned(),
                    carrier: MediaCarrier::WebRtcRtp,
                    wire_format: PCMU_WIRE_FORMAT.to_owned(),
                    sdk_format: L16_SDK_FORMAT.to_owned(),
                }],
            },
        ],
        negotiation: NegotiationSpec {
            transport: NegotiationTransport::WebSocketSubprotocol,
            server_preference: vec![
                CLASSIC_PROFILE.to_owned(),
                DEMO_PROFILE.to_owned(),
                WEBRTC_PROFILE.to_owned(),
            ],
            default_profile: CLASSIC_PROFILE.to_owned(),
            headerless_profile: Some(CLASSIC_PROFILE.to_owned()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_names_composition_order_and_headerless_default_are_frozen() {
        let registry = registry();
        assert_eq!(
            registry.negotiation.server_preference,
            [CLASSIC_PROFILE, DEMO_PROFILE, WEBRTC_PROFILE]
        );
        assert_eq!(registry.negotiation.default_profile, CLASSIC_PROFILE);
        assert_eq!(
            registry.negotiation.headerless_profile.as_deref(),
            Some(CLASSIC_PROFILE)
        );

        let classic = &registry.profiles[0];
        assert_eq!(classic.transport, WEBSOCKET_TRANSPORT);
        assert_eq!(classic.envelope, CLASSIC_ENVELOPE);
        assert_eq!(classic.catalog.to_string(), "babelforce.v1");

        let demo = &registry.profiles[1];
        assert_eq!(demo.catalog.to_string(), "demo.v1");

        let webrtc = &registry.profiles[2];
        assert_eq!(webrtc.transport, WEBRTC_WEBSOCKET_TRANSPORT);
        assert_eq!(webrtc.signaling, [WEBRTC_OFFER_METHOD]);
        assert_eq!(webrtc.media[0].wire_format, PCMU_WIRE_FORMAT);
        assert_eq!(webrtc.media[0].sdk_format, L16_SDK_FORMAT);
    }
}
