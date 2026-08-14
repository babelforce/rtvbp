use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Catalog, CatalogId, EnvelopeSpec, RESERVED_TRANSPORT_METHOD_PREFIX};

/// The carrier used for semantic control messages by a transport binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlCarrier {
    Memory,
    #[serde(rename = "websocket-text")]
    WebSocketText,
}

/// A carrier available to one named media channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaCarrier {
    Memory,
    #[serde(rename = "websocket-binary")]
    WebSocketBinary,
    #[serde(rename = "webrtc-rtp")]
    WebRtcRtp,
}

/// The protocol used to negotiate a profile token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NegotiationTransport {
    #[serde(rename = "websocket-subprotocol")]
    WebSocketSubprotocol,
}

/// Declarative capabilities of one transport binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportSpec {
    pub id: String,
    pub description: String,
    pub control: ControlCarrier,
    pub media_carriers: Vec<MediaCarrier>,
}

/// One exact media format referenced by profiles at the wire and SDK boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormatSpec {
    pub id: String,
    pub encoding: String,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
    pub packet_time_ms: u32,
}

/// An envelope-carried operation reserved for transport setup rather than catalog dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingSpec {
    pub method: String,
    pub transport: String,
    pub description: String,
}

/// One profile media channel and its transport/wire/SDK boundary composition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMediaSpec {
    pub channel: String,
    pub carrier: MediaCarrier,
    pub wire_format: String,
    pub sdk_format: String,
}

/// A named interoperable transport × envelope × catalog profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSpec {
    pub id: String,
    pub negotiation_token: String,
    pub transport: String,
    pub envelope: String,
    pub catalog: CatalogId,
    pub signaling: Vec<String>,
    pub media: Vec<ProfileMediaSpec>,
}

/// Profile selection rules independent of any SDK's network implementation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiationSpec {
    pub transport: NegotiationTransport,
    /// Profile ids in accepting-endpoint preference order.
    pub server_preference: Vec<String>,
    pub default_profile: String,
    /// Effective profile when the negotiation token is absent; no token is echoed.
    pub headerless_profile: Option<String>,
}

/// The complete declarative binding/profile registry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRegistry {
    pub transports: Vec<TransportSpec>,
    pub media_formats: Vec<MediaFormatSpec>,
    pub signaling: Vec<SignalingSpec>,
    pub profiles: Vec<ProfileSpec>,
    pub negotiation: NegotiationSpec,
}

impl ProfileRegistry {
    /// Validate uniqueness plus every catalog, envelope, transport, signaling and media reference.
    pub fn validate(
        &self,
        catalogs: &[Catalog],
        envelopes: &[EnvelopeSpec],
    ) -> Result<(), ProfileValidationErrors> {
        let mut issues = Vec::new();

        let mut transport_ids = HashSet::new();
        let mut transports = HashMap::new();
        for transport in &self.transports {
            require_text(&mut issues, "transport id", &transport.id);
            require_text(
                &mut issues,
                &format!("transport {:?} description", transport.id),
                &transport.description,
            );
            if !transport_ids.insert(transport.id.as_str()) {
                issue(
                    &mut issues,
                    "transports",
                    format!("duplicate transport {:?}", transport.id),
                );
            }
            let mut carriers = HashSet::new();
            for carrier in &transport.media_carriers {
                if !carriers.insert(*carrier) {
                    issue(
                        &mut issues,
                        &format!("transport {:?}", transport.id),
                        format!("duplicate media carrier {carrier:?}"),
                    );
                }
            }
            transports.insert(transport.id.as_str(), transport);
        }

        let mut format_ids = HashSet::new();
        for format in &self.media_formats {
            require_text(&mut issues, "media format id", &format.id);
            require_text(
                &mut issues,
                &format!("media format {:?} encoding", format.id),
                &format.encoding,
            );
            if !format_ids.insert(format.id.as_str()) {
                issue(
                    &mut issues,
                    "media formats",
                    format!("duplicate media format {:?}", format.id),
                );
            }
            if format.sample_rate == 0
                || format.bit_depth == 0
                || format.channels == 0
                || format.packet_time_ms == 0
            {
                issue(
                    &mut issues,
                    &format!("media format {:?}", format.id),
                    "sample rate, bit depth, channels and packet time must be positive",
                );
            }
        }

        let mut signaling_methods = HashSet::new();
        let mut signaling = HashMap::new();
        for signal in &self.signaling {
            if !signal.method.starts_with(RESERVED_TRANSPORT_METHOD_PREFIX)
                || signal.method.len() == RESERVED_TRANSPORT_METHOD_PREFIX.len()
            {
                issue(
                    &mut issues,
                    "signaling",
                    format!(
                        "method {:?} must use the reserved transport.* namespace",
                        signal.method
                    ),
                );
            }
            require_text(
                &mut issues,
                &format!("signaling {:?} description", signal.method),
                &signal.description,
            );
            if !signaling_methods.insert(signal.method.as_str()) {
                issue(
                    &mut issues,
                    "signaling",
                    format!("duplicate signaling method {:?}", signal.method),
                );
            }
            if !transports.contains_key(signal.transport.as_str()) {
                issue(
                    &mut issues,
                    &format!("signaling {:?}", signal.method),
                    format!("unknown transport {:?}", signal.transport),
                );
            }
            signaling.insert(signal.method.as_str(), signal);
        }

        let catalog_ids = catalogs
            .iter()
            .map(|catalog| catalog.id.to_string())
            .collect::<HashSet<_>>();
        let envelope_ids = envelopes
            .iter()
            .map(|envelope| envelope.id.as_str())
            .collect::<HashSet<_>>();
        let mut profile_ids = HashSet::new();
        let mut profile_tokens = HashSet::new();
        let mut profiles = HashMap::new();
        for profile in &self.profiles {
            require_text(&mut issues, "profile id", &profile.id);
            if !profile_ids.insert(profile.id.as_str()) {
                issue(
                    &mut issues,
                    "profiles",
                    format!("duplicate profile {:?}", profile.id),
                );
            }
            if !valid_websocket_token(&profile.negotiation_token) {
                issue(
                    &mut issues,
                    &format!("profile {:?}", profile.id),
                    format!(
                        "negotiation token {:?} is not a valid WebSocket subprotocol token",
                        profile.negotiation_token
                    ),
                );
            }
            if !profile_tokens.insert(profile.negotiation_token.as_str()) {
                issue(
                    &mut issues,
                    "profiles",
                    format!(
                        "duplicate negotiation token {:?}",
                        profile.negotiation_token
                    ),
                );
            }
            let Some(transport) = transports.get(profile.transport.as_str()).copied() else {
                issue(
                    &mut issues,
                    &format!("profile {:?}", profile.id),
                    format!("unknown transport {:?}", profile.transport),
                );
                profiles.insert(profile.id.as_str(), profile);
                continue;
            };
            if !envelope_ids.contains(profile.envelope.as_str()) {
                issue(
                    &mut issues,
                    &format!("profile {:?}", profile.id),
                    format!("unknown envelope {:?}", profile.envelope),
                );
            }
            let catalog_id = profile.catalog.to_string();
            if !catalog_ids.contains(&catalog_id) {
                issue(
                    &mut issues,
                    &format!("profile {:?}", profile.id),
                    format!("unknown catalog {catalog_id:?}"),
                );
            }
            let mut profile_signaling = HashSet::new();
            for method in &profile.signaling {
                if !profile_signaling.insert(method.as_str()) {
                    issue(
                        &mut issues,
                        &format!("profile {:?}", profile.id),
                        format!("duplicate signaling reference {method:?}"),
                    );
                }
                match signaling.get(method.as_str()) {
                    None => issue(
                        &mut issues,
                        &format!("profile {:?}", profile.id),
                        format!("unknown signaling method {method:?}"),
                    ),
                    Some(signal) if signal.transport != profile.transport => issue(
                        &mut issues,
                        &format!("profile {:?}", profile.id),
                        format!(
                            "signaling method {method:?} belongs to transport {:?}",
                            signal.transport
                        ),
                    ),
                    Some(_) => {}
                }
            }
            let mut channels = HashSet::new();
            for media in &profile.media {
                if media.channel.trim().is_empty() || !channels.insert(media.channel.as_str()) {
                    issue(
                        &mut issues,
                        &format!("profile {:?}", profile.id),
                        format!("blank or duplicate media channel {:?}", media.channel),
                    );
                }
                if !transport.media_carriers.contains(&media.carrier) {
                    issue(
                        &mut issues,
                        &format!("profile {:?} media {:?}", profile.id, media.channel),
                        format!(
                            "carrier {:?} is not supported by transport {:?}",
                            media.carrier, profile.transport
                        ),
                    );
                }
                if !format_ids.contains(media.wire_format.as_str()) {
                    issue(
                        &mut issues,
                        &format!("profile {:?} media {:?}", profile.id, media.channel),
                        format!("unknown wire format {:?}", media.wire_format),
                    );
                }
                if !format_ids.contains(media.sdk_format.as_str()) {
                    issue(
                        &mut issues,
                        &format!("profile {:?} media {:?}", profile.id, media.channel),
                        format!("unknown SDK format {:?}", media.sdk_format),
                    );
                }
            }
            profiles.insert(profile.id.as_str(), profile);
        }

        let mut preference = HashSet::new();
        for profile in &self.negotiation.server_preference {
            if !preference.insert(profile.as_str()) {
                issue(
                    &mut issues,
                    "negotiation",
                    format!("duplicate server-preference profile {profile:?}"),
                );
            }
            if !profiles.contains_key(profile.as_str()) {
                issue(
                    &mut issues,
                    "negotiation",
                    format!("unknown server-preference profile {profile:?}"),
                );
            }
        }
        for profile in &self.profiles {
            if !preference.contains(profile.id.as_str()) {
                issue(
                    &mut issues,
                    "negotiation",
                    format!("profile {:?} is absent from server preference", profile.id),
                );
            }
        }
        if !profiles.contains_key(self.negotiation.default_profile.as_str()) {
            issue(
                &mut issues,
                "negotiation",
                format!(
                    "default profile {:?} is unknown",
                    self.negotiation.default_profile
                ),
            );
        }
        if let Some(headerless) = &self.negotiation.headerless_profile {
            if !profiles.contains_key(headerless.as_str()) {
                issue(
                    &mut issues,
                    "negotiation",
                    format!("headerless profile {headerless:?} is unknown"),
                );
            }
            if headerless != &self.negotiation.default_profile {
                issue(
                    &mut issues,
                    "negotiation",
                    "headerless profile must equal the default profile to avoid ambiguous fallback",
                );
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(ProfileValidationErrors { issues })
        }
    }
}

fn valid_websocket_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn require_text(issues: &mut Vec<ProfileValidationError>, location: &str, value: &str) {
    if value.trim().is_empty() {
        issue(issues, location, "must be non-empty");
    }
}

fn issue(
    issues: &mut Vec<ProfileValidationError>,
    location: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(ProfileValidationError::Invalid {
        location: location.into(),
        message: message.into(),
    });
}

/// All issues found while validating one profile registry.
#[derive(Debug)]
pub struct ProfileValidationErrors {
    pub issues: Vec<ProfileValidationError>,
}

impl fmt::Display for ProfileValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "profile registry validation failed with {} issue(s):",
            self.issues.len()
        )?;
        for issue in &self.issues {
            writeln!(formatter, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProfileValidationErrors {}

/// One actionable registry validation issue.
#[derive(Debug, Error)]
pub enum ProfileValidationError {
    #[error("{location}: {message}")]
    Invalid { location: String, message: String },
}
