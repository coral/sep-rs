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

func TestInvalidJSONIsCategorized(t *testing.T) {
	_, err := ValidateArtifactJson("{")
	if !errors.Is(err, ErrSepToolsErrorInvalidRequest) {
		t.Fatalf("expected invalid-request error, got %v", err)
	}
}
