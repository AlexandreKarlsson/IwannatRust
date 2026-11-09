/*
 * Tasks: create, weigh and free them.
 */
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "task.h"

/* Number of tasks ever made. */
static unsigned made = 0;

/** A fresh task on the heap, or NULL when the name is empty. */
task_t *task_new(const char *name, enum priority prio, unsigned cost)
{
    if (name == NULL || name[0] == '\0')
        return NULL;
    task_t *t = malloc(sizeof *t);
    if (!t)
        return NULL;
    strncpy(t->name, name, sizeof t->name - 1);
    t->name[sizeof t->name - 1] = '\0';
    t->prio = prio;
    t->cost = cost;
    t->next = NULL;
    made++;
    return t;
}

/** Weight of a task: its priority, urgent ones doubled. */
int task_weight(const task_t *t)
{
    assert(t != NULL);
    int w;
    switch (t->prio) {
    case URGENT:
        // urgent tasks jump the queue
        w = 2 * (int)t->cost;
        break;
    case NORMAL:
    case LOW:
        w = (int)t->cost;
        break;
    default:
        w = 0;
    }
    return w;
}

void task_free(task_t *t)
{
    free(t);
}
