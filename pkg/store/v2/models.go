package v2

import (
	"time"

	"github.com/arthur-debert/nanostore/nanostore"
)

// PadzScratch represents a scratch document with nanostore dimensions
type PadzScratch struct {
	nanostore.Document // Provides UUID, Title, CreatedAt, UpdatedAt

	// Dimensions for ID generation and querying
	Activity string `values:"active,deleted" default:"active"`
	Pinned   string `values:"no,yes" prefix:"yes=p" default:"no"`
	Project  string `dimension:"project,ref"` // For project hierarchy

	// Custom fields (stored in _data)
	Size      int64     `json:"size,omitempty"`
	Checksum  string    `json:"checksum,omitempty"`
	Content   string    `json:"content,omitempty"` // The actual scratch content
	PinnedAt  time.Time `json:"pinned_at,omitempty"`
	DeletedAt time.Time `json:"deleted_at,omitempty"`
}

// ToScratch converts PadzScratch to the legacy Scratch model for compatibility
func (ps *PadzScratch) ToScratch() Scratch {
	return Scratch{
		ID:        ps.UUID,
		Project:   ps.Project,
		Title:     ps.Title,
		CreatedAt: ps.CreatedAt,
		UpdatedAt: ps.UpdatedAt,
		Size:      ps.Size,
		Checksum:  ps.Checksum,
		IsPinned:  ps.Pinned == "yes",
		PinnedAt:  ps.PinnedAt,
		IsDeleted: ps.Activity == "deleted",
		DeletedAt: func() *time.Time {
			if ps.Activity == "deleted" && !ps.DeletedAt.IsZero() {
				return &ps.DeletedAt
			}
			return nil
		}(),
	}
}

// FromScratch creates a PadzScratch from legacy Scratch model
func FromScratch(s Scratch) *PadzScratch {
	ps := &PadzScratch{
		Document: nanostore.Document{
			// Don't set UUID - let nanostore generate it
			Title:     s.Title,
			CreatedAt: s.CreatedAt,
			UpdatedAt: s.UpdatedAt,
		},
		Project:  s.Project,
		Size:     s.Size,
		Checksum: s.Checksum,
		Activity: "active",
		Pinned:   "no",
	}

	if s.IsPinned {
		ps.Pinned = "yes"
		ps.PinnedAt = s.PinnedAt
	}

	if s.IsDeleted {
		ps.Activity = "deleted"
		if s.DeletedAt != nil {
			ps.DeletedAt = *s.DeletedAt
		}
	}

	return ps
}

// Legacy Scratch struct for compatibility during migration
type Scratch struct {
	ID        string     `json:"id"`
	Project   string     `json:"project"`
	Title     string     `json:"title"`
	CreatedAt time.Time  `json:"created_at"`
	UpdatedAt time.Time  `json:"updated_at,omitempty"`
	Size      int64      `json:"size,omitempty"`
	Checksum  string     `json:"checksum,omitempty"`
	IsPinned  bool       `json:"is_pinned,omitempty"`
	PinnedAt  time.Time  `json:"pinned_at,omitempty"`
	IsDeleted bool       `json:"is_deleted,omitempty"`
	DeletedAt *time.Time `json:"deleted_at,omitempty"`
}
