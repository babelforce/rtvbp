use rtvbp_spec_model::{
    Catalog, CatalogId, ControlCarrier, EnvelopeSpec, MediaCarrier, MediaFormatSpec,
    NegotiationSpec, NegotiationTransport, ProfileMediaSpec, ProfileRegistry, ProfileSpec,
    SignalingSpec, TransportSpec,
};

fn catalog() -> Catalog {
    Catalog::new("demo", 1)
}

fn envelope() -> EnvelopeSpec {
    rtvbp_spec_model::EnvelopeSpec {
        id: "compact.v1".to_owned(),
        constants: Vec::new(),
        frames: Vec::new(),
        error: rtvbp_spec_model::ErrorSpec {
            code: rtvbp_spec_model::FieldSpec::required("code"),
            message: rtvbp_spec_model::FieldSpec::required("message"),
            data: rtvbp_spec_model::FieldSpec::optional("data"),
        },
        error_codes: Vec::new(),
        fixtures: Vec::new(),
    }
}

fn registry() -> ProfileRegistry {
    ProfileRegistry {
        transports: vec![TransportSpec {
            id: "memory.v1".to_owned(),
            description: "In-memory semantic control and media.".to_owned(),
            control: ControlCarrier::Memory,
            media_carriers: vec![MediaCarrier::Memory],
        }],
        media_formats: vec![MediaFormatSpec {
            id: "l16-8k-mono-20ms".to_owned(),
            encoding: "L16".to_owned(),
            sample_rate: 8_000,
            bit_depth: 16,
            channels: 1,
            packet_time_ms: 20,
        }],
        signaling: vec![SignalingSpec {
            method: "transport.memory.open".to_owned(),
            transport: "memory.v1".to_owned(),
            description: "Open the synthetic media path.".to_owned(),
        }],
        profiles: vec![ProfileSpec {
            id: "rtvbp.memory.v1".to_owned(),
            negotiation_token: "rtvbp.memory.v1".to_owned(),
            transport: "memory.v1".to_owned(),
            envelope: "compact.v1".to_owned(),
            catalog: CatalogId::new("demo", 1),
            signaling: vec!["transport.memory.open".to_owned()],
            media: vec![ProfileMediaSpec {
                channel: "audio".to_owned(),
                carrier: MediaCarrier::Memory,
                wire_format: "l16-8k-mono-20ms".to_owned(),
                sdk_format: "l16-8k-mono-20ms".to_owned(),
            }],
        }],
        negotiation: NegotiationSpec {
            transport: NegotiationTransport::WebSocketSubprotocol,
            server_preference: vec!["rtvbp.memory.v1".to_owned()],
            default_profile: "rtvbp.memory.v1".to_owned(),
            headerless_profile: Some("rtvbp.memory.v1".to_owned()),
        },
    }
}

#[test]
fn a_complete_registry_validates_all_references() {
    registry()
        .validate(&[catalog()], &[envelope()])
        .expect("synthetic registry should validate");
}

#[test]
fn registry_rejects_collisions_dangling_references_and_ambiguous_defaults() {
    let catalogs = [catalog()];
    let envelopes = [envelope()];

    let mut duplicate = registry();
    duplicate.profiles.push(duplicate.profiles[0].clone());
    let error = duplicate
        .validate(&catalogs, &envelopes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate profile"), "{error}");

    let mut dangling = registry();
    dangling.profiles[0].catalog = CatalogId::new("missing", 1);
    let error = dangling
        .validate(&catalogs, &envelopes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown catalog"), "{error}");

    let mut ambiguous = registry();
    ambiguous.negotiation.headerless_profile = Some("missing.profile".to_owned());
    let error = ambiguous
        .validate(&catalogs, &envelopes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("headerless profile"), "{error}");
}

#[test]
fn reserved_signaling_and_media_constraints_are_validated() {
    let catalogs = [catalog()];
    let envelopes = [envelope()];

    let mut invalid_signal = registry();
    invalid_signal.signaling[0].method = "catalog.lookalike".to_owned();
    let error = invalid_signal
        .validate(&catalogs, &envelopes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("reserved transport.*"), "{error}");

    let mut invalid_media = registry();
    invalid_media.profiles[0].media[0].wire_format = "missing".to_owned();
    let error = invalid_media
        .validate(&catalogs, &envelopes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown wire format"), "{error}");
}
