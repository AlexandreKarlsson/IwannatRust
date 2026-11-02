#!/usr/bin/env bash
# Play the codecast of this repository with IwannatRust (`codecast`): a spoken, interactive tour of
# IwannatRust itself. Two tours: codecast/ (the project, from parsing to the command line) and
# codecast-algorithms/ (every algorithm of iwr-core, explained like you are five).
#
#   scripts/codecast.sh                 serve the tour on http://127.0.0.1:4321 and open the browser
#   scripts/codecast.sh algo [CMD]      same commands, on the algorithms tour (codecast-algorithms/)
#   scripts/codecast.sh check           validate every ref in the tour against the code (exit 1 on a broken ref)
#   scripts/codecast.sh speak           print the tour as the voice will read it
#   scripts/codecast.sh voice [ARGS]    record the tour with Kokoro TTS (NN-part.ogg + .times)
#   scripts/codecast.sh export [DIR]    static site (default: iwr-site/, gitignored)
#   scripts/codecast.sh brief [--fn P]  the compact code brief codecast gives a writer; `--guide` adds the writing guide
#
# Environment:
#   IWR    path to the codecast binary. Default: target/release/codecast (scripts/iwr build), else `codecast` on PATH
#   PORT   serve port (default 4321); a codecast already serving on it is stopped first
#
# codecast/*.md and glossary.md are re-read on every page reload; restart the serve after
# changing Rust sources or rebuilding.
set -euo pipefail

ROOT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
SCRIPT_DIR="$ROOT/codecast"
PORT="${PORT:-4321}"

case "${1:-}" in
    algo|algorithms) SCRIPT_DIR="$ROOT/codecast-algorithms"; shift ;;
esac

# The repository's own build first: it is the codecast the tour is about.
find_iwr() {
    if [ -n "${IWR:-}" ]; then echo "$IWR"; return; fi
    local cand="$ROOT/target/release/codecast"
    [ -x "$cand" ] && { echo "$cand"; return; }
    if command -v codecast >/dev/null 2>&1; then command -v codecast; return; fi
    echo "codecast not found: scripts/iwr build, put codecast on PATH or set IWR=/path/to/codecast" >&2
    exit 1
}

# Stop a codecast still serving on $PORT (an older build would keep serving the old UI and script);
# refuse when another program holds the port.
stop_running() {
    local pid
    pid="$(ss -ltnpH "sport = :$PORT" 2>/dev/null | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2 || true)"
    [ -n "$pid" ] || return 0
    if [ "$(cat "/proc/$pid/comm" 2>/dev/null)" != codecast ]; then
        echo "port $PORT is used by $(cat "/proc/$pid/comm" 2>/dev/null || echo "pid $pid"); set PORT=<free port>" >&2
        exit 1
    fi
    echo "stopping codecast already serving on port $PORT (pid $pid)" >&2
    kill "$pid"
    for _ in $(seq 50); do kill -0 "$pid" 2>/dev/null || return 0; sleep 0.1; done
    kill -9 "$pid" 2>/dev/null || true
}

IWR_BIN="$(find_iwr)"
cmd="${1:-serve}"; shift || true
case "$cmd" in
    serve)  stop_running; exec "$IWR_BIN" serve "$ROOT" --script "$SCRIPT_DIR" -p "$PORT" "$@" ;;
    check)  exec "$IWR_BIN" check "$SCRIPT_DIR" --path "$ROOT" ;;
    speak)  exec "$IWR_BIN" speak "$SCRIPT_DIR" --path "$ROOT" ;;
    voice)  exec "$IWR_BIN" voice "$SCRIPT_DIR" "$@" ;;
    export) out="${1:-$ROOT/iwr-site}"; shift || true
            exec "$IWR_BIN" export "$ROOT" --script "$SCRIPT_DIR" -o "$out" "$@" ;;
    brief)  exec "$IWR_BIN" brief "$ROOT" "$@" ;;
    -h|--help|help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//' ;;
    *) echo "unknown command: $cmd (serve|check|speak|voice|export|brief; prefix algo for the algorithms tour)" >&2; exit 1 ;;
esac
