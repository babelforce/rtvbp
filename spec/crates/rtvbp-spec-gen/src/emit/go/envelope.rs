use std::fmt::Write as _;
use std::path::PathBuf;

use rtvbp_spec_model::{ControlFrame, EnvelopeSpec, FrameKind, WireError};

use super::{GO_BANNER, GoEmitError};
use crate::emit::GeneratedFile;

const CODEC_TEMPLATE: &str = r#"package __PACKAGE__

import (
	"bytes"
	"encoding/json"
	"fmt"

	"github.com/babelforce/rtvbp/sdk/go"
)

const envelopeName = __ENVELOPE_NAME__

type constantSpec struct {
	name  string
	value string
}

type frameSpec struct {
	kind          rtvbp.Kind
	discriminator string
	id            string
	payload       string
	omitPayload   bool
	error         string
	omitError     bool
}

type errorSpec struct {
	code     string
	message  string
	data     string
	omitData bool
}

var constants = []constantSpec{
__CONSTANTS__}

// Frame order is structural discrimination precedence.
var frames = []frameSpec{
__FRAMES__}

var wireError = errorSpec{
__ERROR_SPEC__}

// Envelope implements the __ENVELOPE_NAME__ envelope codec.
type Envelope struct{}

var _ rtvbp.Envelope = Envelope{}

// Name returns the envelope identifier.
func (Envelope) Name() string { return envelopeName }

// Encode projects one semantic frame into the envelope's ordered wire object.
// Raw payload and error-data JSON is preserved verbatim; fields irrelevant to Kind are ignored.
func (Envelope) Encode(frame rtvbp.ControlFrame) ([]byte, error) {
	shape, ok := frameForKind(frame.Kind)
	if !ok {
		return nil, fmt.Errorf("%s: unknown frame kind %d", envelopeName, frame.Kind)
	}

	fields := make([]wireField, 0, len(constants)+4)
	for _, constant := range constants {
		if err := appendValue(&fields, constant.name, constant.value); err != nil {
			return nil, err
		}
	}

	switch frame.Kind {
	case rtvbp.KindRequest, rtvbp.KindEvent:
		if frame.ID == "" {
			return nil, fmt.Errorf("%s: frame id is required", envelopeName)
		}
		if frame.Method == "" {
			return nil, fmt.Errorf("%s: frame method is required", envelopeName)
		}
		if err := appendValue(&fields, shape.id, frame.ID); err != nil {
			return nil, err
		}
		if err := appendValue(&fields, shape.discriminator, frame.Method); err != nil {
			return nil, err
		}
	case rtvbp.KindResponse:
		if frame.CorrelID == "" {
			return nil, fmt.Errorf("%s: response correlation id is required", envelopeName)
		}
		if err := appendValue(&fields, shape.discriminator, frame.CorrelID); err != nil {
			return nil, err
		}
	}

	if err := appendRaw(&fields, shape.payload, frame.Payload, shape.omitPayload); err != nil {
		return nil, err
	}
	if shape.error != "" {
		if frame.Err == nil {
			if err := appendRaw(&fields, shape.error, nil, shape.omitError); err != nil {
				return nil, err
			}
		} else {
			encoded, err := encodeError(frame.Err)
			if err != nil {
				return nil, err
			}
			if err := appendRaw(&fields, shape.error, encoded, shape.omitError); err != nil {
				return nil, err
			}
		}
	}
	return marshalObject(fields)
}

// Decode projects one envelope wire object into a semantic frame.
func (Envelope) Decode(data []byte) (rtvbp.ControlFrame, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil {
		return rtvbp.ControlFrame{}, fmt.Errorf("%s: invalid JSON: %w", envelopeName, err)
	}
	if object == nil {
		return rtvbp.ControlFrame{}, fmt.Errorf("%s: envelope must be an object", envelopeName)
	}
	for _, constant := range constants {
		value, err := requiredString(object, constant.name)
		if err != nil || value != constant.value {
			return rtvbp.ControlFrame{}, fmt.Errorf("%s: %s must equal %q", envelopeName, constant.name, constant.value)
		}
	}

	for _, shape := range frames {
		discriminator, ok := discriminatorString(object, shape.discriminator)
		if !ok {
			continue
		}
		frame := rtvbp.ControlFrame{Kind: shape.kind}
		if shape.kind == rtvbp.KindResponse {
			frame.CorrelID = discriminator
		} else {
			id, err := requiredString(object, shape.id)
			if err != nil {
				return rtvbp.ControlFrame{}, fmt.Errorf("%s: %w", envelopeName, err)
			}
			frame.ID = id
			frame.Method = discriminator
		}
		frame.Payload = optionalRaw(object, shape.payload)
		if shape.error != "" {
			raw, present := object[shape.error]
			if present && !bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
				decoded, err := decodeError(raw)
				if err != nil {
					return rtvbp.ControlFrame{}, err
				}
				frame.Err = decoded
			}
		}
		return frame, nil
	}
	return rtvbp.ControlFrame{}, fmt.Errorf("%s: envelope has no recognized frame discriminator", envelopeName)
}

type wireField struct {
	name  string
	value json.RawMessage
}

func frameForKind(kind rtvbp.Kind) (frameSpec, bool) {
	for _, frame := range frames {
		if frame.kind == kind {
			return frame, true
		}
	}
	return frameSpec{}, false
}

func appendValue(fields *[]wireField, name string, value any) error {
	encoded, err := json.Marshal(value)
	if err != nil {
		return fmt.Errorf("%s: encode %s: %w", envelopeName, name, err)
	}
	*fields = append(*fields, wireField{name: name, value: encoded})
	return nil
}

func appendRaw(fields *[]wireField, name string, value json.RawMessage, omitWhenNone bool) error {
	if value == nil {
		if omitWhenNone {
			return nil
		}
		value = json.RawMessage("null")
	}
	if len(value) == 0 || !json.Valid(value) {
		return fmt.Errorf("%s: %s contains invalid JSON", envelopeName, name)
	}
	*fields = append(*fields, wireField{name: name, value: value})
	return nil
}

func marshalObject(fields []wireField) ([]byte, error) {
	output := make([]byte, 0, 128)
	output = append(output, '{')
	for index, field := range fields {
		if index != 0 {
			output = append(output, ',')
		}
		name, err := json.Marshal(field.name)
		if err != nil {
			return nil, fmt.Errorf("%s: encode field name: %w", envelopeName, err)
		}
		output = append(output, name...)
		output = append(output, ':')
		output = append(output, field.value...)
	}
	output = append(output, '}')
	return output, nil
}

func encodeError(value *rtvbp.WireError) (json.RawMessage, error) {
	if value.Code == 0 {
		return nil, fmt.Errorf("%s: error code must be non-zero", envelopeName)
	}
	if value.Message == "" {
		return nil, fmt.Errorf("%s: error message is required", envelopeName)
	}
	fields := make([]wireField, 0, 3)
	if err := appendValue(&fields, wireError.code, value.Code); err != nil {
		return nil, err
	}
	if err := appendValue(&fields, wireError.message, value.Message); err != nil {
		return nil, err
	}
	if err := appendRaw(&fields, wireError.data, value.Data, wireError.omitData); err != nil {
		return nil, err
	}
	return marshalObject(fields)
}

func decodeError(data json.RawMessage) (*rtvbp.WireError, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil || object == nil {
		return nil, fmt.Errorf("%s: error must be an object", envelopeName)
	}
	var code int
	encodedCode, ok := object[wireError.code]
	if !ok || json.Unmarshal(encodedCode, &code) != nil || code == 0 {
		return nil, fmt.Errorf("%s: error code must be a non-zero integer", envelopeName)
	}
	message, err := requiredString(object, wireError.message)
	if err != nil {
		return nil, fmt.Errorf("%s: error message is required", envelopeName)
	}
	return &rtvbp.WireError{
		Code:    code,
		Message: message,
		Data:    optionalRaw(object, wireError.data),
	}, nil
}

func requiredString(object map[string]json.RawMessage, name string) (string, error) {
	raw, ok := object[name]
	if !ok {
		return "", fmt.Errorf("%s is required", name)
	}
	var value string
	if err := json.Unmarshal(raw, &value); err != nil || value == "" {
		return "", fmt.Errorf("%s must be a non-empty string", name)
	}
	return value, nil
}

func discriminatorString(object map[string]json.RawMessage, name string) (string, bool) {
	raw, ok := object[name]
	if !ok {
		return "", false
	}
	var value string
	if json.Unmarshal(raw, &value) != nil || value == "" {
		return "", false
	}
	return value, true
}

func optionalRaw(object map[string]json.RawMessage, name string) json.RawMessage {
	raw, ok := object[name]
	if !ok {
		return nil
	}
	return append(json.RawMessage(nil), raw...)
}
"#;

const CONTRACT_TESTS: &str = r#"
func TestDiscriminatorFallbackAndDecodeValidation(t *testing.T) {
	envelope := Envelope{}
	event := testFrame(t, rtvbp.KindEvent)
	request := testFrame(t, rtvbp.KindRequest)
	response := testFrame(t, rtvbp.KindResponse)

	frame, err := envelope.Decode(testWire(t,
		wireField{name: event.discriminator, value: testJSON(t, 7)},
		wireField{name: request.id, value: testJSON(t, "request-1")},
		wireField{name: request.discriminator, value: testJSON(t, "ping")},
		wireField{name: response.discriminator, value: testJSON(t, "ignored")},
	))
	if err != nil {
		t.Fatal(err)
	}
	if frame.Kind != rtvbp.KindRequest || frame.Method != "ping" {
		t.Fatalf("non-string discriminator did not fall through: %#v", frame)
	}

	frame, err = envelope.Decode(testWire(t,
		wireField{name: request.id, value: testJSON(t, "request-1")},
		wireField{name: request.discriminator, value: testJSON(t, "ping")},
		wireField{name: response.discriminator, value: testJSON(t, "ignored")},
	))
	if err != nil || frame.Kind != rtvbp.KindRequest {
		t.Fatalf("request did not beat response: %#v, %v", frame, err)
	}

	invalid := [][]byte{
		[]byte("[]"),
		testWire(t),
		testWire(t, wireField{name: request.discriminator, value: testJSON(t, "ping")}),
	}
	if len(constants) != 0 {
		wrong := testBaseFields(t)
		wrong[0].value = testJSON(t, "wrong")
		invalid = append(invalid, testObject(t, wrong...))
		invalid = append(invalid, testObject(t,
			wireField{name: request.id, value: testJSON(t, "request-1")},
			wireField{name: request.discriminator, value: testJSON(t, "ping")},
		))
	}
	for _, wire := range invalid {
		if _, err := envelope.Decode(wire); err == nil {
			t.Fatalf("invalid envelope decoded: %s", wire)
		}
	}
}

func TestErrorNullDataAndValidation(t *testing.T) {
	envelope := Envelope{}
	response := testFrame(t, rtvbp.KindResponse)
	correlation := wireField{name: response.discriminator, value: testJSON(t, "request-1")}

	if response.omitPayload && response.omitError {
		withNull := testWire(t, correlation, wireField{name: response.error, value: json.RawMessage("null")})
		frame, err := envelope.Decode(withNull)
		if err != nil || frame.Err != nil {
			t.Fatalf("error:null decoded as %#v, %v", frame, err)
		}
		encoded, err := envelope.Encode(frame)
		if err != nil {
			t.Fatal(err)
		}
		if want := testWire(t, correlation); !bytes.Equal(encoded, want) {
			t.Fatalf("error:null normalization mismatch\nwant: %s\n got: %s", want, encoded)
		}
	}

	errorWithNullData := testObject(t,
		wireField{name: wireError.code, value: testJSON(t, 777)},
		wireField{name: wireError.message, value: testJSON(t, "extension")},
		wireField{name: wireError.data, value: json.RawMessage("null")},
	)
	wire := testWire(t, correlation, wireField{name: response.error, value: errorWithNullData})
	frame, err := envelope.Decode(wire)
	if err != nil || frame.Err == nil || frame.Err.Code != 777 || string(frame.Err.Data) != "null" {
		t.Fatalf("unknown error with null data decoded as %#v, %v", frame, err)
	}
	encoded, err := envelope.Encode(frame)
	if err != nil || !bytes.Equal(encoded, wire) {
		t.Fatalf("null error data round-trip mismatch: %s, %v", encoded, err)
	}

	invalidErrors := []json.RawMessage{
		json.RawMessage("[]"),
		testObject(t),
		testObject(t,
			wireField{name: wireError.code, value: testJSON(t, 0)},
			wireField{name: wireError.message, value: testJSON(t, "unset")},
		),
		testObject(t,
			wireField{name: wireError.code, value: testJSON(t, "bad")},
			wireField{name: wireError.message, value: testJSON(t, "failed")},
		),
		testObject(t, wireField{name: wireError.code, value: testJSON(t, 500)}),
		testObject(t,
			wireField{name: wireError.code, value: testJSON(t, 500)},
			wireField{name: wireError.message, value: testJSON(t, "")},
		),
		testObject(t,
			wireField{name: wireError.code, value: testJSON(t, 500)},
			wireField{name: wireError.message, value: testJSON(t, 7)},
		),
	}
	for _, invalidError := range invalidErrors {
		wire := testWire(t, correlation, wireField{name: response.error, value: invalidError})
		if _, err := envelope.Decode(wire); err == nil {
			t.Fatalf("invalid error decoded: %s", wire)
		}
	}
}

func TestEncodeValidationAndReceiveTimestamp(t *testing.T) {
	envelope := Envelope{}
	invalid := []rtvbp.ControlFrame{
		{},
		{Kind: rtvbp.KindRequest, Method: "ping"},
		{Kind: rtvbp.KindRequest, ID: "request-1"},
		{Kind: rtvbp.KindEvent, Method: "ready"},
		{Kind: rtvbp.KindResponse},
		{Kind: rtvbp.KindResponse, CorrelID: "request-1", Err: &rtvbp.WireError{Code: 0, Message: "unset"}},
		{Kind: rtvbp.KindResponse, CorrelID: "request-1", Err: &rtvbp.WireError{Code: 500}},
		{Kind: rtvbp.KindResponse, CorrelID: "request-1", Payload: json.RawMessage{}},
		{Kind: rtvbp.KindResponse, CorrelID: "request-1", Err: &rtvbp.WireError{Code: 500, Message: "failed", Data: json.RawMessage{}}},
	}
	for _, frame := range invalid {
		if _, err := envelope.Encode(frame); err == nil {
			t.Fatalf("invalid frame encoded: %#v", frame)
		}
	}

	if len(goldenFrames) != 0 {
		frame := goldenFrames[0].frame
		frame.ReceivedAt = time.Unix(123, 456)
		encoded, err := envelope.Encode(frame)
		if err != nil || !bytes.Equal(encoded, goldenFrames[0].bytes) {
			t.Fatalf("ReceivedAt changed encoding: %s, %v", encoded, err)
		}
	}
}

func testFrame(t *testing.T, kind rtvbp.Kind) frameSpec {
	t.Helper()
	frame, ok := frameForKind(kind)
	if !ok {
		t.Fatalf("missing frame kind %d", kind)
	}
	return frame
}

func testBaseFields(t *testing.T) []wireField {
	t.Helper()
	fields := make([]wireField, 0, len(constants))
	for _, constant := range constants {
		fields = append(fields, wireField{name: constant.name, value: testJSON(t, constant.value)})
	}
	return fields
}

func testWire(t *testing.T, fields ...wireField) []byte {
	t.Helper()
	all := append(testBaseFields(t), fields...)
	return testObject(t, all...)
}

func testObject(t *testing.T, fields ...wireField) json.RawMessage {
	t.Helper()
	wire, err := marshalObject(fields)
	if err != nil {
		t.Fatal(err)
	}
	return wire
}

func testJSON(t *testing.T, value any) json.RawMessage {
	t.Helper()
	wire, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return wire
}
"#;

/// Emit one generated Go envelope codec and its frozen-wire tests.
pub fn emit_go_envelope(spec: &EnvelopeSpec) -> Result<Vec<GeneratedFile>, GoEmitError> {
    spec.validate().map_err(|error| GoEmitError::Envelope {
        envelope: spec.id.clone(),
        message: error.to_string(),
    })?;
    let package = package_name(&spec.id)?;
    let package_dir = PathBuf::from("envelope").join(&package);
    Ok(vec![
        GeneratedFile {
            path: package_dir.join("zz_generated.codec.go"),
            bytes: render_codec(spec, &package)?.into_bytes(),
        },
        GeneratedFile {
            path: package_dir.join("zz_generated.golden_test.go"),
            bytes: render_tests(spec, &package)?.into_bytes(),
        },
    ])
}

fn render_codec(spec: &EnvelopeSpec, package: &str) -> Result<String, GoEmitError> {
    let mut constants = String::new();
    for constant in &spec.constants {
        writeln!(
            constants,
            "\t{{name: {}, value: {}}},",
            go_string(&constant.name)?,
            go_string(&constant.value)?
        )
        .unwrap();
    }

    let mut frames = String::new();
    for frame in &spec.frames {
        writeln!(
            frames,
            "\t{{kind: {}, discriminator: {}, id: {}, payload: {}, omitPayload: {}, error: {}, omitError: {}}},",
            go_kind(frame.kind),
            go_string(&frame.discriminator.name)?,
            go_string(frame.id.as_ref().map_or("", |field| &field.name))?,
            go_string(&frame.payload.name)?,
            frame.payload.omit_when_none,
            go_string(frame.error.as_ref().map_or("", |field| &field.name))?,
            frame.error.as_ref().is_none_or(|field| field.omit_when_none),
        )
        .unwrap();
    }

    let error_spec = format!(
        "\tcode:     {},\n\tmessage:  {},\n\tdata:     {},\n\tomitData: {},\n",
        go_string(&spec.error.code.name)?,
        go_string(&spec.error.message.name)?,
        go_string(&spec.error.data.name)?,
        spec.error.data.omit_when_none,
    );

    let envelope_name = go_string(&spec.id)?;
    let body = expand_template(
        CODEC_TEMPLATE,
        &[
            ("__PACKAGE__", package),
            ("__ENVELOPE_NAME__", &envelope_name),
            ("__CONSTANTS__", &constants),
            ("__FRAMES__", &frames),
            ("__ERROR_SPEC__", &error_spec),
        ],
    );
    Ok(format!("{GO_BANNER}{body}"))
}

fn render_tests(spec: &EnvelopeSpec, package: &str) -> Result<String, GoEmitError> {
    let mut output = String::from(GO_BANNER);
    writeln!(output, "package {package}\n").unwrap();
    output.push_str(
        "import (\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"reflect\"\n\t\"testing\"\n\t\"time\"\n\n\t\"github.com/babelforce/rtvbp/sdk/go\"\n)\n\n",
    );
    output.push_str("var goldenFrames = []struct {\n\tname  string\n\tbytes []byte\n\tframe rtvbp.ControlFrame\n}{\n");
    let mut fixtures = spec.fixtures.iter().collect::<Vec<_>>();
    fixtures.sort_by(|left, right| left.path.cmp(&right.path));
    for fixture in fixtures {
        let bytes = std::str::from_utf8(&fixture.bytes).map_err(|_| GoEmitError::Envelope {
            envelope: spec.id.clone(),
            message: format!("fixture {:?} is not UTF-8", fixture.path),
        })?;
        writeln!(
            output,
            "\t{{name: {}, bytes: []byte({}), frame: {}}},",
            go_string(&fixture.path)?,
            go_string(bytes)?,
            render_frame(&fixture.frame)?,
        )
        .unwrap();
    }
    output.push_str(
        "}\n\nfunc TestGoldenFrames(t *testing.T) {\n\tenvelope := Envelope{}\n\tif got := envelope.Name(); got != envelopeName {\n\t\tt.Fatalf(\"Name() = %q, want %q\", got, envelopeName)\n\t}\n\tfor _, tc := range goldenFrames {\n\t\tt.Run(tc.name+\"/encode\", func(t *testing.T) {\n\t\t\tactual, err := envelope.Encode(tc.frame)\n\t\t\tif err != nil {\n\t\t\t\tt.Fatal(err)\n\t\t\t}\n\t\t\tif !bytes.Equal(actual, tc.bytes) {\n\t\t\t\tt.Fatalf(\"encode mismatch\\nwant: %s\\n got: %s\", tc.bytes, actual)\n\t\t\t}\n\t\t})\n\t\tt.Run(tc.name+\"/decode\", func(t *testing.T) {\n\t\t\tactual, err := envelope.Decode(tc.bytes)\n\t\t\tif err != nil {\n\t\t\t\tt.Fatal(err)\n\t\t\t}\n\t\t\tif !reflect.DeepEqual(actual, tc.frame) {\n\t\t\t\tt.Fatalf(\"decode mismatch\\nwant: %#v\\n got: %#v\", tc.frame, actual)\n\t\t\t}\n\t\t})\n\t}\n}\n\n",
    );

    let ambiguous = ambiguous_frame(spec)?;
    writeln!(
        output,
        "func TestStructuralPrecedenceAndMalformedInput(t *testing.T) {{\n\tenvelope := Envelope{{}}\n\tframe, err := envelope.Decode([]byte({}))\n\tif err != nil {{\n\t\tt.Fatal(err)\n\t}}\n\tif frame.Kind != {} || frame.Method != {} {{\n\t\tt.Fatalf(\"precedence decoded %#v\", frame)\n\t}}\n\tif _, err := envelope.Decode([]byte(\"not json\")); err == nil {{\n\t\tt.Fatal(\"malformed input decoded without error\")\n\t}}\n}}",
        go_string(&ambiguous.bytes)?,
        go_kind(ambiguous.kind),
        go_string(&ambiguous.discriminator)?,
    )
    .unwrap();
    if let Some(response) = response_with_both(spec)? {
        writeln!(
            output,
            "\nfunc TestResponseAllowsResultAndError(t *testing.T) {{\n\tenvelope := Envelope{{}}\n\twire := []byte({})\n\tframe, err := envelope.Decode(wire)\n\tif err != nil {{\n\t\tt.Fatal(err)\n\t}}\n\tif frame.Kind != rtvbp.KindResponse || string(frame.Payload) != \"{{}}\" || frame.Err == nil || frame.Err.Code != {} {{\n\t\tt.Fatalf(\"decoded %#v\", frame)\n\t}}\n\tencoded, err := envelope.Encode(frame)\n\tif err != nil {{\n\t\tt.Fatal(err)\n\t}}\n\tif !bytes.Equal(encoded, wire) {{\n\t\tt.Fatalf(\"round-trip mismatch\\nwant: %s\\n got: %s\", wire, encoded)\n\t}}\n}}",
            go_string(&response.bytes)?,
            response.code,
        )
        .unwrap();
    }
    output.push_str(CONTRACT_TESTS);
    Ok(output)
}

struct AmbiguousFrame {
    bytes: String,
    kind: FrameKind,
    discriminator: String,
}

struct ResponseWithBoth {
    bytes: String,
    code: i64,
}

fn response_with_both(spec: &EnvelopeSpec) -> Result<Option<ResponseWithBoth>, GoEmitError> {
    let Some(response) = spec
        .frames
        .iter()
        .find(|frame| frame.kind == FrameKind::Response)
    else {
        return Ok(None);
    };
    let Some(error_field) = &response.error else {
        return Ok(None);
    };
    if !response.payload.omit_when_none || !error_field.omit_when_none {
        return Ok(None);
    }
    let code = spec
        .error_codes
        .iter()
        .find(|error| error.code == 500)
        .or_else(|| spec.error_codes.first())
        .map_or(1, |error| error.code);
    let mut object = serde_json::Map::new();
    for constant in &spec.constants {
        object.insert(
            constant.name.clone(),
            serde_json::Value::String(constant.value.clone()),
        );
    }
    object.insert(
        response.discriminator.name.clone(),
        serde_json::Value::String("request-1".into()),
    );
    object.insert(response.payload.name.clone(), serde_json::json!({}));
    let mut error = serde_json::Map::new();
    error.insert(spec.error.code.name.clone(), serde_json::json!(code));
    error.insert(
        spec.error.message.name.clone(),
        serde_json::Value::String("failed".into()),
    );
    object.insert(error_field.name.clone(), serde_json::Value::Object(error));
    Ok(Some(ResponseWithBoth {
        bytes: serde_json::to_string(&object)?,
        code,
    }))
}

fn ambiguous_frame(spec: &EnvelopeSpec) -> Result<AmbiguousFrame, GoEmitError> {
    let first = spec.frames.first().ok_or_else(|| GoEmitError::Envelope {
        envelope: spec.id.clone(),
        message: "has no frame shapes".to_owned(),
    })?;
    let second = spec.frames.get(1).ok_or_else(|| GoEmitError::Envelope {
        envelope: spec.id.clone(),
        message: "needs two frame shapes to prove discrimination precedence".to_owned(),
    })?;
    let mut object = serde_json::Map::new();
    for constant in &spec.constants {
        object.insert(
            constant.name.clone(),
            serde_json::Value::String(constant.value.clone()),
        );
    }
    if let Some(id) = &first.id {
        object.insert(
            id.name.clone(),
            serde_json::Value::String("first-id".into()),
        );
    }
    object.insert(
        first.discriminator.name.clone(),
        serde_json::Value::String("first".into()),
    );
    if let Some(id) = &second.id {
        object
            .entry(id.name.clone())
            .or_insert_with(|| serde_json::Value::String("second-id".into()));
    }
    object.insert(
        second.discriminator.name.clone(),
        serde_json::Value::String("second".into()),
    );
    Ok(AmbiguousFrame {
        bytes: serde_json::to_string(&object)?,
        kind: first.kind,
        discriminator: "first".to_owned(),
    })
}

fn render_frame(frame: &ControlFrame) -> Result<String, GoEmitError> {
    let rendered = match frame {
        ControlFrame::Request { id, method, params } => format!(
            "rtvbp.ControlFrame{{Kind: rtvbp.KindRequest, ID: {}, Method: {}, Payload: {}}}",
            go_string(id)?,
            go_string(method)?,
            render_raw(params.as_ref())?,
        ),
        ControlFrame::Response {
            correlation_id,
            result,
            error,
        } => format!(
            "rtvbp.ControlFrame{{Kind: rtvbp.KindResponse, CorrelID: {}, Payload: {}, Err: {}}}",
            go_string(correlation_id)?,
            render_raw(result.as_ref())?,
            render_error(error.as_ref())?,
        ),
        ControlFrame::Event { id, event, data } => format!(
            "rtvbp.ControlFrame{{Kind: rtvbp.KindEvent, ID: {}, Method: {}, Payload: {}}}",
            go_string(id)?,
            go_string(event)?,
            render_raw(data.as_ref())?,
        ),
    };
    Ok(rendered)
}

fn render_error(error: Option<&WireError>) -> Result<String, GoEmitError> {
    match error {
        None => Ok("nil".to_owned()),
        Some(error) => Ok(format!(
            "&rtvbp.WireError{{Code: {}, Message: {}, Data: {}}}",
            error.code,
            go_string(&error.message)?,
            render_raw(error.data.as_ref())?,
        )),
    }
}

fn render_raw(value: Option<&serde_json::Value>) -> Result<String, GoEmitError> {
    match value {
        None => Ok("nil".to_owned()),
        Some(value) => Ok(format!(
            "json.RawMessage({})",
            go_string(&serde_json::to_string(value)?)?
        )),
    }
}

fn package_name(id: &str) -> Result<String, GoEmitError> {
    let (name, major) = id.rsplit_once(".v").ok_or_else(|| GoEmitError::Envelope {
        envelope: id.to_owned(),
        message: "id must end in .v<major>".to_owned(),
    })?;
    if name.is_empty() || major.is_empty() || !major.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(GoEmitError::Envelope {
            envelope: id.to_owned(),
            message: "id must end in .v<major>".to_owned(),
        });
    }
    let name = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if name.is_empty() {
        return Err(GoEmitError::Envelope {
            envelope: id.to_owned(),
            message: "id has no package-name characters".to_owned(),
        });
    }
    Ok(format!("v{major}{name}"))
}

fn go_kind(kind: FrameKind) -> &'static str {
    match kind {
        FrameKind::Request => "rtvbp.KindRequest",
        FrameKind::Response => "rtvbp.KindResponse",
        FrameKind::Event => "rtvbp.KindEvent",
    }
}

fn go_string(value: &str) -> Result<String, GoEmitError> {
    Ok(serde_json::to_string(value)?)
}

fn expand_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remainder = template;
    while let Some((position, token, value)) = replacements
        .iter()
        .filter_map(|(token, value)| {
            remainder
                .find(token)
                .map(|position| (position, *token, *value))
        })
        .min_by_key(|(position, _, _)| *position)
    {
        output.push_str(&remainder[..position]);
        output.push_str(value);
        remainder = &remainder[position + token.len()..];
    }
    output.push_str(remainder);
    output
}
