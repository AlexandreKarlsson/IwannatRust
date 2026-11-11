// Package store keeps the tasks waiting to run.
package store

import "errors"

// ErrFull is returned when the store has no room left.
var ErrFull = errors.New("store full")

// Store holds tasks, heaviest first.
type Store struct {
	tasks []*Task
	limit int
}

// New makes a store that holds up to limit tasks.
func New(limit int) *Store {
	return &Store{limit: limit}
}

// Len is the number of tasks waiting.
func (s *Store) Len() int {
	return len(s.tasks)
}

// Add puts a task in, keeping the heaviest first.
func (s *Store) Add(t *Task) error {
	if len(s.tasks) >= s.limit {
		return ErrFull
	}
	s.tasks = append(s.tasks, t)
	for i := len(s.tasks) - 1; i > 0; i-- {
		if heavier(s.tasks[i], s.tasks[i-1]) {
			s.tasks[i], s.tasks[i-1] = s.tasks[i-1], s.tasks[i]
		}
	}
	return nil
}

// Pop takes the heaviest task out.
func (s *Store) Pop() (*Task, error) {
	if len(s.tasks) == 0 {
		return nil, errors.New("empty")
	}
	t := s.tasks[0]
	s.tasks = s.tasks[1:]
	return t, nil
}

// Clear drops every task.
func (s *Store) Clear() {
	s.tasks = nil
}

func heavier(a, b Weigher) bool {
	return a.Weight() > b.Weight()
}
