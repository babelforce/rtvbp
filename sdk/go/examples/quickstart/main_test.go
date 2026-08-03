package main

import (
	"testing"

	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

func TestSelectCodecFindsDeployedProfileOffering(t *testing.T) {
	want := v1bridge.AudioCodecL16_8kHzMono
	offerings := []v1.AudioCodec{
		{ID: "other", Name: "L16", SampleRate: 16_000, BitDepth: 16, Channels: 1},
		want,
	}
	got, err := selectCodec(offerings)
	if err != nil {
		t.Fatal(err)
	}
	if *got != want {
		t.Fatalf("selected codec = %#v, want %#v", *got, want)
	}
}

func TestSelectCodecRejectsMissingDeployedProfileOffering(t *testing.T) {
	if _, err := selectCodec(nil); err == nil {
		t.Fatal("selectCodec accepted an empty offering list")
	}
}
