package septools

import (
	"encoding/json"
	"errors"
	"testing"
)

func TestModelProfiles(t *testing.T) {
	value, err := ModelProfilesJson()
	if err != nil {
		t.Fatalf("ModelProfilesJson: %v", err)
	}

	var profiles []map[string]any
	if err := json.Unmarshal([]byte(value), &profiles); err != nil {
		t.Fatalf("decode profiles: %v", err)
	}
	if len(profiles) == 0 {
		t.Fatal("expected at least one model profile")
	}
}

func TestOptions(t *testing.T) {
	device := "device"
	value, err := OptionsJson(&device)
	if err != nil {
		t.Fatalf("OptionsJson: %v", err)
	}

	var catalog struct {
		SchemaVersion int `json:"schema_version"`
		Targets       []struct {
			Target string `json:"target"`
		} `json:"targets"`
		Choices struct {
			ModelProfiles []map[string]any `json:"model_profiles"`
		} `json:"choices"`
		Schema map[string]any `json:"schema"`
	}
	if err := json.Unmarshal([]byte(value), &catalog); err != nil {
		t.Fatalf("decode options: %v", err)
	}
	if catalog.SchemaVersion != 1 {
		t.Fatalf("expected schema version 1, got %d", catalog.SchemaVersion)
	}
	if len(catalog.Targets) != 1 || catalog.Targets[0].Target != device {
		t.Fatalf("expected only device target, got %#v", catalog.Targets)
	}
	if len(catalog.Choices.ModelProfiles) == 0 || catalog.Schema["$defs"] == nil {
		t.Fatal("expected model choices and shared schema definitions")
	}
}

func TestInvalidJSONIsCategorized(t *testing.T) {
	_, err := ValidateArtifactJson("{")
	if !errors.Is(err, ErrSepToolsErrorInvalidRequest) {
		t.Fatalf("expected invalid-request error, got %v", err)
	}
}

func TestInvalidOptionsTargetIsCategorized(t *testing.T) {
	target := "wat"
	_, err := OptionsJson(&target)
	if !errors.Is(err, ErrSepToolsErrorInvalidRequest) {
		t.Fatalf("expected invalid-request error, got %v", err)
	}
}
