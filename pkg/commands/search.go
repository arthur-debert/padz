package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"regexp"
)

func Search(s *store.Store, all, global bool, project, term string) ([]store.Scratch, error) {
	scratches := Ls(s, all, global, project)

	re, err := regexp.Compile(term)
	if err != nil {
		return nil, err
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return nil, err
		}
		if re.Match(content) {
			filtered = append(filtered, scratch)
		}
	}

	return filtered, nil
}
