package store

import "testing"

func TestAdd(t *testing.T) {
	s := New(1)
	if err := s.Add(NewTask("a", 0)); err != nil {
		t.Fatal(err)
	}
}
