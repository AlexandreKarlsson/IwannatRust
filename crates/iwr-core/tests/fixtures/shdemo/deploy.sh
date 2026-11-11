#!/usr/bin/env bash
# Deploy the site: build it, then copy it to every host.
#
# usage: deploy.sh [-n] HOST...

set -euo pipefail

source "$(dirname "$0")/lib/log.sh"

readonly TARGET_DIR=/var/www/site
DRY_RUN=0

# Copy the build to one host; 1 when the host is down.
copy_to() {
    local host=$1
    if ! ping -c 1 "$host" > /dev/null; then
        warn "$host is down"
        return 1
    fi
    # the real work
    [ "$DRY_RUN" -eq 1 ] || rsync -a build/ "$host:$TARGET_DIR"
    info "copied to $host"
}

usage() {
    echo "usage: deploy.sh [-n] HOST..." >&2
    exit 2
}

case "${1:-}" in
    -n) DRY_RUN=1; shift ;;
    -h|--help) usage ;;
    *) ;;
esac

[ $# -gt 0 ] || usage

./build.sh || die "build failed"

failed=0
for host in "$@"; do
    copy_to "$host" || failed=$((failed + 1))
done

while [ "$failed" -gt 3 ]; do
    warn "too many hosts down"
    break
done

if [ "$failed" -gt 0 ]; then
    die "$failed hosts failed"
elif [ "$DRY_RUN" -eq 1 ]; then
    info "dry run"
else
    info "done"
fi
