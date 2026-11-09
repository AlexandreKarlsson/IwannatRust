/** Keep tasks sorted by weight. */
#include <stdio.h>
#include <stdlib.h>
#include "store.h"

/** Put a task in its place; -1 when the store is full. */
int store_add(struct store *s, task_t *t)
{
    if (s->count >= MAX_TASKS) {
        fprintf(stderr, "store full\n");
        return -1;
    }
    int w = task_weight(t);
    task_t **slot = &s->head;
    // walk to the first lighter task
    while (*slot && task_weight(*slot) >= w)
        slot = &(*slot)->next;
    t->next = *slot;
    *slot = t;
    s->count++;
    return 0;
}

/** Take the heaviest task out, NULL when empty. */
task_t *store_pop(struct store *s)
{
    task_t *t = s->head;
    if (t == NULL)
        return NULL;
    s->head = t->next;
    s->count--;
    return t;
}

void store_clear(struct store *s)
{
    task_t *t;
    do {
        t = store_pop(s);
        if (t)
            task_free(t);
    } while (t != NULL);
}
