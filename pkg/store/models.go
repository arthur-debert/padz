package store

import (
	"time"

	"github.com/arthur-debert/nanostore/nanostore"
)

type Scratch struct {
	ID        string     `json:"id"`
	Project   string     `json:"project"`
	Title     string     `json:"title"`
	Content   string     `json:"-"` // Not stored in JSON metadata, but in nanostore Body field
	CreatedAt time.Time  `json:"created_at"`
	UpdatedAt time.Time  `json:"updated_at,omitempty"`
	Size      int64      `json:"size,omitempty"`
	Checksum  string     `json:"checksum,omitempty"`
	IsPinned  bool       `json:"is_pinned,omitempty"`
	PinnedAt  time.Time  `json:"pinned_at,omitempty"`
	IsDeleted bool       `json:"is_deleted,omitempty"`
	DeletedAt *time.Time `json:"deleted_at,omitempty"`
}

// TypedScratch represents a scratch using the TypedAPI with struct tags for dimensions
type TypedScratch struct {
	nanostore.Document // Required embedding for TypedAPI

	// Enumerated dimensions - these map to our current nanostore dimensions
	Activity string `values:"active,deleted" default:"active"`    // activity dimension
	Pinned   string `values:"no,yes" prefix:"yes=p" default:"no"` // pinned dimension with "p" prefix

	// Data fields - stored as _data.* in nanostore
	Project   string // _data.project
	Size      int64  // _data.size
	Checksum  string // _data.checksum
	PinnedAt  string // _data.pinned_at - RFC3339 string, empty when not pinned
	DeletedAt string // _data.deleted_at - RFC3339 string, empty when not deleted
}

// ToScratch converts a TypedScratch to the legacy Scratch struct
func (ts *TypedScratch) ToScratch() Scratch {
	var pinnedAt time.Time
	if ts.Pinned == "yes" && ts.PinnedAt != "" {
		if parsed, err := time.Parse(time.RFC3339, ts.PinnedAt); err == nil {
			pinnedAt = parsed
		}
	}

	var deletedAt *time.Time
	if ts.DeletedAt != "" {
		if parsed, err := time.Parse(time.RFC3339, ts.DeletedAt); err == nil {
			deletedAt = &parsed
		}
	}

	return Scratch{
		ID:        ts.SimpleID,
		Project:   ts.Project,
		Title:     ts.Title,
		Content:   ts.Body,
		CreatedAt: ts.CreatedAt,
		UpdatedAt: ts.UpdatedAt,
		Size:      ts.Size,
		Checksum:  ts.Checksum,
		IsPinned:  ts.Pinned == "yes",
		PinnedAt:  pinnedAt,
		IsDeleted: ts.Activity == "deleted",
		DeletedAt: deletedAt,
	}
}

// FromScratch converts a legacy Scratch to TypedScratch
func FromScratch(s Scratch) *TypedScratch {
	activity := "active"
	if s.IsDeleted {
		activity = "deleted"
	}

	pinned := "no"
	if s.IsPinned {
		pinned = "yes"
	}

	// Convert time fields to RFC3339 strings
	var pinnedAt string
	if s.IsPinned && !s.PinnedAt.IsZero() {
		pinnedAt = s.PinnedAt.Format(time.RFC3339)
	}

	var deletedAt string
	if s.DeletedAt != nil {
		deletedAt = s.DeletedAt.Format(time.RFC3339)
	}

	return &TypedScratch{
		Document: nanostore.Document{
			SimpleID:  s.ID,
			Title:     s.Title,
			Body:      s.Content,
			CreatedAt: s.CreatedAt,
			UpdatedAt: s.UpdatedAt,
		},
		Activity:  activity,
		Pinned:    pinned,
		Project:   s.Project,
		Size:      s.Size,
		Checksum:  s.Checksum,
		PinnedAt:  pinnedAt,
		DeletedAt: deletedAt,
	}
}

// IndexEntry represents minimal scratch info for the master index
type IndexEntry struct {
	Project   string    `json:"project"`
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"created_at"`
}

// Index represents the master index structure
type Index struct {
	Version   string                `json:"version"`
	UpdatedAt time.Time             `json:"updated_at"`
	Scratches map[string]IndexEntry `json:"scratches"`
}
