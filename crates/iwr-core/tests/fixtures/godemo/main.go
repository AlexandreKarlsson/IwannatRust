// Command godemo schedules a few tasks and runs them by priority.
package main

import (
	"errors"
	"fmt"
	"os"

	"example.com/godemo/store"
)

// maxTasks is how many tasks one run takes.
const maxTasks = 8

var verbose = false

// fill puts the named tasks into the store.
func fill(s *store.Store, names []string) error {
	for i, name := range names {
		if i >= maxTasks {
			break
		}
		t := store.NewTask(name, i)
		// a full store is the caller's problem
		if err := s.Add(t); err != nil {
			return err
		}
	}
	return nil
}

func run(s *store.Store) (int, error) {
	done := 0
	for s.Len() > 0 {
		t, err := s.Pop()
		if err != nil {
			return done, fmt.Errorf("pop: %w", err)
		}
		switch t.Priority {
		case store.High:
			fmt.Println("urgent", t.Name)
		case store.Low, store.Normal:
			fmt.Println(t.Name)
		default:
			return done, errors.New("unknown priority")
		}
		done++
	}
	return done, nil
}

func main() {
	s := store.New(4)
	defer s.Clear()
	if err := fill(s, os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	n, err := run(s)
	if err != nil {
		panic(err)
	}
	if verbose {
		go fmt.Println("ran", n)
	}
}
