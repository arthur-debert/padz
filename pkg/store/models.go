package store

import "time"

type Scratch struct {
	ID        string    `json:"id"`
	Project   string    `json:"project"`
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"created_at"`
}
