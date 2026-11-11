# Messages on stderr, with a level in front.

# Print an information line.
info() {
    echo "[info] $*" >&2
}

warn() {
    echo "[warn] $*" >&2
}

# Print the message and stop the script.
die() {
    echo "[fatal] $*" >&2
    exit 1
}
