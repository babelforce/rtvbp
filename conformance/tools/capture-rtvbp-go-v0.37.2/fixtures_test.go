package main

import (
	"bytes"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

var commonFixturePaths = []string{
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
	"variants/events/call.hangup-no-reason.json",
	"variants/payloads/application.move.request-empty.json",
	"variants/payloads/application.move.response-no-next.json",
	"variants/payloads/ping.request-no-optionals.json",
	"variants/payloads/ping.response-no-data.json",
	"variants/payloads/recording.start.request-no-tags.json",
}

var notInV0372FixturePaths = []string{
	"events/agent.tool.call.json",
	"events/audio.info.json",
	"events/input.transcript.json",
	"events/output.transcript.delta.json",
	"events/output.transcript.done.json",
	"variants/events/audio.info-nonzero.json",
	"variants/events/output.transcript.done-text-empty.json",
	"variants/events/output.transcript.done-text.json",
}

func TestCommonInventoryIsExplicitAndComplete(t *testing.T) {
	assertSortedUnique(t, commonFixturePaths)
	assertSortedUnique(t, notInV0372FixturePaths)

	var captured []string
	for _, item := range fixtures() {
		captured = append(captured, item.path)
	}
	sort.Strings(captured)
	if !equalStrings(captured, commonFixturePaths) {
		t.Fatalf("v0.37.2 capture inventory mismatch\nwant: %q\n got: %q", commonFixturePaths, captured)
	}

	wantGolden := append(append([]string{}, commonFixturePaths...), notInV0372FixturePaths...)
	sort.Strings(wantGolden)
	gotGolden := jsonInventory(t, goldenRoot(t))
	if !equalStrings(gotGolden, wantGolden) {
		t.Fatalf("golden inventory has an unclassified or missing fixture\nwant: %q\n got: %q", wantGolden, gotGolden)
	}
}

func TestV0372MatchesV0400GoldenCommonBytes(t *testing.T) {
	generated := t.TempDir()
	if err := capture(generated); err != nil {
		t.Fatal(err)
	}

	golden := goldenRoot(t)
	for _, name := range commonFixturePaths {
		t.Run(name, func(t *testing.T) {
			want, err := os.ReadFile(filepath.Join(golden, filepath.FromSlash(name)))
			if err != nil {
				t.Fatal(err)
			}
			got, err := os.ReadFile(filepath.Join(generated, filepath.FromSlash(name)))
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(got, want) {
				t.Fatalf("v0.37.2 differs from v0.40.0 authority\nwant: %s\n got: %s", want, got)
			}
		})
	}
}

func goldenRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", "babelforce.v1", "golden"))
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func jsonInventory(t *testing.T, root string) []string {
	t.Helper()
	var inventory []string
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
		inventory = append(inventory, filepath.ToSlash(rel))
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	sort.Strings(inventory)
	return inventory
}

func assertSortedUnique(t *testing.T, values []string) {
	t.Helper()
	for index := 1; index < len(values); index++ {
		if values[index-1] >= values[index] {
			t.Fatalf("inventory must be sorted and unique: %q then %q", values[index-1], values[index])
		}
	}
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for index := range a {
		if a[index] != b[index] {
			return false
		}
	}
	return true
}
