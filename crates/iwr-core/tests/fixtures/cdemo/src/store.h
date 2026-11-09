/** A store of tasks: a linked list, heaviest first. */
#ifndef STORE_H
#define STORE_H
#include "task.h"

struct store {
    task_t *head;
    size_t count;
};

int store_add(struct store *s, task_t *t);
task_t *store_pop(struct store *s);
void store_clear(struct store *s);

#endif
