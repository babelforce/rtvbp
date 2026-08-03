package main

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestCaptureReproducesGoFloat64BoundaryAuthority(t *testing.T) {
	generated := filepath.Join(t.TempDir(), "go-float64-boundaries.json")
	if err := captureFloat64Boundaries(generated); err != nil {
		t.Fatal(err)
	}

	want, err := os.ReadFile(defaultFloatBoundaryOutput())
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(generated)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("Go float64 boundary authority drifted\nwant: %s\n got: %s", want, got)
	}
}
