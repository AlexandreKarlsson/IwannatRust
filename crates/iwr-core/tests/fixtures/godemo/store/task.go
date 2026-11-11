package store

// Priority orders the tasks.
type Priority int

const (
	Low Priority = iota
	Normal
	High
)

// Task is one thing to do.
type Task struct {
	Name     string
	Priority Priority
	weight   int
}

// Weigher is anything that can be ordered.
type Weigher interface {
	// Weight is higher for what goes first.
	Weight() int
}

// NewTask makes a task; every third one is urgent.
func NewTask(name string, i int) *Task {
	p := Normal
	if i%3 == 0 {
		p = High
	}
	return &Task{Name: name, Priority: p, weight: i}
}

// Weight is the priority, then the age.
func (t *Task) Weight() int {
	return int(t.Priority)*100 + t.weight
}
