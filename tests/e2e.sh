#!/usr/bin/env bash
# End-to-end lifecycle tests: real tmux on a private socket, stub `claude`
# and a stub Discord API — no real Claude session, no real network.
#
#   tests/e2e.sh [path-to-dcbot-binary]
#
# Scenarios: healthy start (greet happens only after the gateway line),
# start-while-running, zombie-pane recycle, missing claude, instant-exit
# claude, `dcbot list` dead-state, stop.
set -u

DCBOT_BIN="${1:-$(pwd)/target/debug/dcbot}"
[ -x "$DCBOT_BIN" ] || { echo "dcbot binary not found: $DCBOT_BIN — cargo build first" >&2; exit 2; }

ROOT="$(mktemp -d /tmp/dcbot-e2e.XXXXXX)"
export HOME="$ROOT/home"
mkdir -p "$HOME/.claude/plugins"
export DCBOT_LANG=en DCBOT_LIVE_TIMEOUT_SECS=8
REAL_TMUX="$(command -v tmux)"; [ -n "$REAL_TMUX" ] || { echo "tmux required" >&2; exit 2; }
SOCK="dcbot-e2e-$$"
API_PORT=18991
API_LOG="$ROOT/api.log"

pass=0; fail=0
ok()  { echo "  PASS $1"; pass=$((pass+1)); }
bad() { echo "  FAIL $1"; fail=$((fail+1)); }
say() { echo "== $1"; }

cleanup() {
    "$REAL_TMUX" -L "$SOCK" kill-server 2>/dev/null
    [ -n "${API_PID:-}" ] && kill "$API_PID" 2>/dev/null
    pkill -f "sleep 600" 2>/dev/null
    [ $fail -eq 0 ] && rm -rf "$ROOT" || echo "kept $ROOT for debugging"
}
trap cleanup EXIT

# --- stubs -----------------------------------------------------------------

BIN="$ROOT/bin"; BIN_NOCLAUDE="$ROOT/bin-noclaude"
mkdir -p "$BIN" "$BIN_NOCLAUDE"
for d in "$BIN" "$BIN_NOCLAUDE"; do
    cat > "$d/tmux" <<EOF
#!/bin/sh
exec $REAL_TMUX -L $SOCK "\$@"
EOF
    chmod +x "$d/tmux"
done

mkclaude() {  # $1 = healthy|instant-exit
    cat > "$BIN/claude" <<EOF
#!/bin/sh
if [ "\${1:-}" = "plugin" ]; then
  mkdir -p "\$HOME/.claude/plugins"
  cat > "\$HOME/.claude/plugins/installed_plugins.json" <<'JSON'
{"version":2,"plugins":{"discord@claude-plugins-official":[{"scope":"user","installPath":"/x","version":"0.0.4","installedAt":"2026-01-01T00:00:00Z","lastUpdated":"2026-01-01T00:00:00Z"}]}}
JSON
  exit 0
fi
if [ "$1" = "instant-exit" ]; then
  echo "stub claude: error crashing on boot" >&2
  exit 1
fi
echo "discord channel: gateway connected as @stub" >&2
exec sleep 600
EOF
    chmod +x "$BIN/claude"
}
mkclaude healthy

# Plugin pre-seeded Ready so `plugin install` never runs.
cat > "$HOME/.claude/plugins/installed_plugins.json" <<'JSON'
{"version":2,"plugins":{"discord@claude-plugins-official":[{"scope":"user","installPath":"/x","version":"0.0.4","installedAt":"2026-01-01T00:00:00Z","lastUpdated":"2026-01-01T00:00:00Z"}]}}
JSON

cat > "$ROOT/discord_stub.py" <<'PY'
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer
logf, port = sys.argv[1], int(sys.argv[2])
class H(BaseHTTPRequestHandler):
    def _reply(self, body):
        b = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def _log(self):
        with open(logf, "a") as f:
            f.write(f"{self.command} {self.path}\n")
    def do_GET(self):
        self._log()
        if self.path == "/users/@me":
            self._reply({"id": "1552555836134391838", "username": "stubbot", "discriminator": "0"})
        elif self.path == "/applications/@me":
            self._reply({"id": "1552555836134391838", "flags": 524288})
        else:
            self._reply({})
    def do_POST(self):
        self._log()
        if self.path == "/users/@me/channels":
            self._reply({"id": "999"})
        elif self.path.startswith("/channels/"):
            self._reply({"id": "1"})
        else:
            self._reply({})
    def log_message(self, *a): pass
HTTPServer(("127.0.0.1", port), H).serve_forever()
PY
python3 "$ROOT/discord_stub.py" "$API_LOG" "$API_PORT" &
API_PID=$!
sleep 0.4

export PATH="$BIN:$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin"
export DCBOT_API_BASE="http://127.0.0.1:$API_PORT"
DCBOT() { "$DCBOT_BIN" "$@"; }

wait_dead() {  # poll until pane shows the dead-shell marker
    for _ in $(seq 1 20); do
        "$REAL_TMUX" -L "$SOCK" capture-pane -pt "dcbot-$1" -S -60 2>/dev/null \
            | grep -q "run.sh exited — shell below" && return 0
        sleep 0.3
    done
    return 1
}
posts() { grep -c "POST /channels/999/messages" "$API_LOG" 2>/dev/null || true; }

# --- 1. healthy new --start: greet only after gateway line -------------------

say "healthy: dcbot new --start greets only once live"
OUT="$(DCBOT new e2ebot --yes --dir "$ROOT/bot" --token FAKETOKEN \
        --owner 111111111111111111 --start 2>&1)"
RC=$?
[ $RC -eq 0 ]                                          && ok "exit 0"            || { bad "exit $RC"; echo "$OUT"; }
echo "$OUT" | grep -q "session is live"                && ok "live confirmed"    || { bad "no live line"; echo "$OUT"; }
[ "$(posts)" -ge 1 ]                                   && ok "greeting POSTed"   || bad "no greeting POST"
"$REAL_TMUX" -L "$SOCK" has-session -t dcbot-e2ebot    && ok "tmux session up"   || bad "no tmux session"
[ -f "$ROOT/bot/logs/lifecycle.log" ]                  && ok "lifecycle.log"     || bad "no lifecycle.log"
grep -q '\.local/bin' "$ROOT/bot/run.sh"               && ok "run.sh hardened"   || bad "run.sh not hardened"

# --- 2. second start while actually running ---------------------------------

say "healthy: second start bails already_running"
OUT="$(DCBOT start e2ebot 2>&1)"; RC=$?
[ $RC -ne 0 ] && echo "$OUT" | grep -q "already running" && ok "already_running" || { bad "rc=$RC"; echo "$OUT"; }

# --- 3. claude dies → dead pane → list shows dead → start recycles -----------

say "zombie: kill claude, list shows dead, start recycles"
pkill -f "sleep 600"; sleep 0.5
wait_dead e2ebot                                       && ok "pane fell to shell" || bad "pane not dead"
OUT="$(DCBOT list 2>&1)"
echo "$OUT" | grep -q "dead"                           && ok "list shows dead"   || { bad "list not dead"; echo "$OUT"; }
OUT="$(DCBOT start e2ebot 2>&1)"; RC=$?
[ $RC -eq 0 ] && echo "$OUT" | grep -q "session is live" && ok "recycled + live" || { bad "rc=$RC"; echo "$OUT"; }
[ "$(posts)" -ge 2 ]                                   && ok "second greeting"   || bad "no second greeting"

DCBOT stop e2ebot >/dev/null 2>&1
"$REAL_TMUX" -L "$SOCK" has-session -t dcbot-e2ebot 2>/dev/null \
                                                       && bad "session still up"  || ok "stop kills session"

# --- 4. claude missing on PATH: no greet, diagnostics saved ------------------

say "missing claude: start fails loudly, no greeting"
BEFORE="$(posts)"
DCBOT new e2ebot2 --yes --dir "$ROOT/bot2" --token FAKETOKEN \
      --owner 111111111111111111 >/dev/null 2>&1
OUT="$(env PATH="$BIN_NOCLAUDE:$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin" \
        "$DCBOT_BIN" start e2ebot2 2>&1)"; RC=$?
[ $RC -ne 0 ]                                          && ok "start failed"      || { bad "rc=$RC"; echo "$OUT"; }
echo "$OUT" | grep -q "command not found\|not found on PATH" \
                                                       && ok "reason shown"      || { bad "no reason"; echo "$OUT"; }
ls "$ROOT/bot2/logs/"failed-start-*.log >/dev/null 2>&1 && ok "pane captured"    || bad "no failed-start log"
[ "$(posts)" = "$BEFORE" ]                             && ok "no greeting"       || bad "greeting sent anyway"
DCBOT list >/dev/null 2>&1  # warm nothing; session auto-recycles next test

# --- 5. instant-exit claude --------------------------------------------------

say "instant-exit: crash on boot is reported, not greeted"
mkclaude instant-exit
DCBOT new e2ebot3 --yes --dir "$ROOT/bot3" --token FAKETOKEN \
      --owner 111111111111111111 >/dev/null 2>&1
OUT="$(DCBOT start e2ebot3 2>&1)"; RC=$?
[ $RC -ne 0 ] && echo "$OUT" | grep -q "exited\|didn't come up\|dead\|shell" \
                                                       && ok "dead reported"     || { bad "rc=$RC"; echo "$OUT"; }

echo
echo "== $pass passed, $fail failed =="
[ $fail -eq 0 ]
