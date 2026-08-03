package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

var expectedFixtures = []string{
	"envelope/classic.v1/event.json",
	"envelope/classic.v1/request-with-params.json",
	"envelope/classic.v1/request.json",
	"envelope/classic.v1/response-error-internal.json",
	"envelope/classic.v1/response-error-not-implemented.json",
	"envelope/classic.v1/response-error-unknown.json",
	"envelope/classic.v1/response-error.json",
	"envelope/classic.v1/response-ok-no-result.json",
	"envelope/classic.v1/response-ok-null-result.json",
	"envelope/classic.v1/response-ok.json",
	"events/audio.info.json",
	"events/audio.speech.started.json",
	"events/call.hangup.json",
	"events/dtmf.json",
	"events/session.updated.json",
	"payloads/application.move.request.json",
	"payloads/application.move.response.json",
	"payloads/audio.buffer.clear.request.json",
	"payloads/audio.buffer.clear.response.json",
	"payloads/call.hangup.request.json",
	"payloads/call.hangup.response.json",
	"payloads/ping.request.json",
	"payloads/ping.response.json",
	"payloads/recording.start.request.json",
	"payloads/recording.start.response.json",
	"payloads/recording.stop.request.json",
	"payloads/recording.stop.response.json",
	"payloads/session.get.request.json",
	"payloads/session.get.response.json",
	"payloads/session.initialize.request.json",
	"payloads/session.initialize.response.json",
	"payloads/session.set.request.json",
	"payloads/session.set.response.json",
	"payloads/session.terminate.request.json",
	"payloads/session.terminate.response.json",
	"variants/events/audio.info-nonzero.json",
	"variants/events/call.hangup-no-reason.json",
	"variants/payloads/application.move.request-empty.json",
	"variants/payloads/application.move.response-no-next.json",
	"variants/payloads/ping.request-no-optionals.json",
	"variants/payloads/ping.response-no-data.json",
	"variants/payloads/recording.start.request-no-tags.json",
}

var additiveEventFixtures = map[string]struct{}{
	"events/agent.tool.call.json":                            {},
	"events/input.transcript.json":                           {},
	"events/output.transcript.delta.json":                    {},
	"events/output.transcript.done.json":                     {},
	"variants/events/output.transcript.done-text-empty.json": {},
	"variants/events/output.transcript.done-text.json":       {},
}

func goldenRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", "babelforce.v1", "golden"))
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func TestFixtureInventory(t *testing.T) {
	root := goldenRoot(t)
	var got []string
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() || filepath.Ext(path) != ".json" {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		name := filepath.ToSlash(rel)
		if _, additive := additiveEventFixtures[name]; !additive {
			got = append(got, name)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	sort.Strings(got)
	if !equalStrings(got, expectedFixtures) {
		t.Fatalf("fixture inventory mismatch\nwant: %q\n got: %q", expectedFixtures, got)
	}
}

func TestPinnedWireQuirks(t *testing.T) {
	root := goldenRoot(t)
	cases := map[string][]byte{
		"payloads/session.initialize.request.json":                []byte(`{"application":{"id":"app-1"},"call":{"id":"call-1","session_id":"session-1","from":"+12025550100","to":"+12025550101"},"audio_codec_offerings":[{"id":"L16/8000/1","name":"L16","sample_rate":8000,"bit_depth":16,"channels":1}],"metadata":null}`),
		"payloads/session.initialize.response.json":               []byte(`{"audio_codec":null}`),
		"payloads/session.get.response.json":                      []byte(`{"attempt":2,"customer":"Ada"}`),
		"envelope/classic.v1/request.json":                        []byte(`{"version":"1","id":"request-1","method":"session.get"}`),
		"envelope/classic.v1/request-with-params.json":            []byte(`{"version":"1","id":"request-terminate-1","method":"session.terminate","params":{"reason":"completed"}}`),
		"envelope/classic.v1/response-error.json":                 []byte(`{"version":"1","response":"request-1","error":{"code":400,"message":"invalid request","any":{"field":"reason","retryable":false}}}`),
		"envelope/classic.v1/response-error-not-implemented.json": []byte(`{"version":"1","response":"request-terminate-1","error":{"code":501,"message":"session.terminate is not supported. please use application.move or call.hangup instead"}}`),
		"envelope/classic.v1/response-ok-no-result.json":          []byte(`{"version":"1","response":"request-1"}`),
		"envelope/classic.v1/response-ok-null-result.json":        []byte(`{"version":"1","response":"request-1","result":null}`),
		"envelope/classic.v1/event.json":                          []byte(`{"version":"1","id":"event-1","event":"dtmf","data":{"seq":7,"pressed_at":1700000000000,"released_at":1700000000120,"digit":"5"}}`),
		"variants/events/audio.info-nonzero.json":                 []byte(`{"read":{"bytes":1280,"bytes_per_second":12800,"bytes_total":6400},"write":{"bytes":32,"bytes_per_second":106.66666666666667,"bytes_total":96}}`),
	}
	for name, want := range cases {
		t.Run(name, func(t *testing.T) {
			got, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(name)))
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(got, want) {
				t.Fatalf("wire bytes differ\nwant: %s\n got: %s", want, got)
			}
			if bytes.HasSuffix(got, []byte("\n")) {
				t.Fatal("fixture must not end with a newline")
			}
		})
	}
}

func TestCaptureReproducesEveryGoldenByte(t *testing.T) {
	generated := t.TempDir()
	if err := capture(generated); err != nil {
		t.Fatal(err)
	}

	golden := goldenRoot(t)
	for _, name := range expectedFixtures {
		t.Run(name, func(t *testing.T) {
			want, err := os.ReadFile(filepath.Join(golden, filepath.FromSlash(name)))
			if err != nil {
				t.Fatal(err)
			}
			got, err := os.ReadFile(filepath.Join(generated, filepath.FromSlash(name)))
			if err != nil {
				t.Fatal(err)
			}
			if !json.Valid(got) {
				t.Fatalf("generated fixture is not valid JSON: %s", got)
			}
			if bytes.HasSuffix(got, []byte("\n")) {
				t.Fatal("fixture must not end with a newline")
			}
			if !bytes.Equal(got, want) {
				t.Fatalf("capture drifted from committed fixture\nwant: %s\n got: %s", want, got)
			}
		})
	}

	if err := capture(generated); err != nil {
		t.Fatal(err)
	}
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
