/** Tasks: one unit of work with a priority. */
#ifndef TASK_H
#define TASK_H

#include <stddef.h>

/** How many tasks a store holds at most. */
#define MAX_TASKS 64

/** Priority of a task, low to urgent. */
enum priority { LOW, NORMAL, URGENT };

/** One unit of work, with a priority and a cost. */
typedef struct task {
    char name[32];        /**< shown in the schedule */
    enum priority prio;   /**< 1 (low) to 9 (urgent) */
    unsigned cost;        /**< minutes it takes */
    struct task *next;    /**< the task after this one */
} task_t;

/* Compare two tasks, like strcmp. */
typedef int (*task_cmp)(const task_t *, const task_t *);

task_t *task_new(const char *name, enum priority prio, unsigned cost);
int task_weight(const task_t *t);
void task_free(task_t *t);

#endif
