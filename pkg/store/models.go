package store

import "time"

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
