/**
 * Schedule a few tasks and print them heaviest first.
 */
#include <stdio.h>
#include <stdlib.h>
#include "store.h"

#define NAMES_LEN 3

static const char *names[NAMES_LEN] = { "write", "test", "ship" };

/** Fill the store from the names table; 0 on success. */
static int fill(struct store *s)
{
    int rc = 0;
    for (int i = 0; i < NAMES_LEN; i++) {
        task_t *t = task_new(names[i], i == 2 ? URGENT : NORMAL, 10 + i);
        if (t == NULL) {
            rc = -1;
            goto done;
        }
        if (store_add(s, t) < 0) {
            task_free(t);
            rc = -1;
            goto done;
        }
    }
done:
    return rc;
}

int main(int argc, char **argv)
{
    struct store s = { 0 };
    if (argc > 1) {
        printf("usage: %s\n", argv[0]);
        exit(2);
    }
    if (fill(&s) != 0) {
        store_clear(&s);
        return EXIT_FAILURE;
    }
    task_t *t;
    while ((t = store_pop(&s)) != NULL) {
        printf("%s (%d)\n", t->name, task_weight(t));
        task_free(t);
    }
    return 0;
}
