"""Drives tests/pty.rs under a real pseudo-terminal and checks raw escape bytes and termios.

Usage: python3 tests/pty.py <test binary>. Works for both unwind and `-C panic=abort` builds.
"""

import fcntl, os, re, select, signal, struct, subprocess, sys, termios, time

EXE = sys.argv[1]
ENTER, LEAVE, SHOW = b"\x1b[?1049h", b"\x1b[?1049l", b"\x1b[?25h"
PUSH, POP = b"\x1b[>15u", b"\x1b[<1u"
MOUSE_ON = b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h"
MOUSE_OFF = b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l"
FOCUS_ON, FOCUS_OFF = b"\x1b[?1004h", b"\x1b[?1004l"
QUERY, PANIC = b"\x1b[?u\x1b[c", b"panicked at"
ABORTED = -signal.SIGABRT
failures = []


def run(scenario, kitty=False, stdout=None, env=None):
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    # Mutate VEOF to a non-default value so exact termios restoration is distinguished from reset.
    before = termios.tcgetattr(slave)
    before[6][termios.VEOF] = b"\x05"
    termios.tcsetattr(slave, termios.TCSANOW, before)
    before = termios.tcgetattr(slave)

    child = subprocess.Popen(
        [EXE],
        env={**(os.environ if env is None else env), "PTY_SCENARIO": scenario},
        stdin=slave,
        stdout=slave if stdout is None else stdout,
        stderr=slave,
        start_new_session=True,
        # Adopt the PTY slave as controlling terminal so `/dev/tty` sizes it even when
        # stdout is a pipe and `$TERM`/`tput` are unavailable.
        preexec_fn=lambda: fcntl.ioctl(0, termios.TIOCSCTTY, 0),
    )
    out, snaps, queries, deadline = b"", [], 0, time.monotonic() + 45
    sent_keys = False
    while time.monotonic() < deadline:
        if select.select([master], [], [], 0.05)[0]:
            try:
                chunk = os.read(master, 1 << 16)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
        elif child.poll() is not None:
            while select.select([master], [], [], 0)[0]:
                try:
                    chunk = os.read(master, 1 << 16)
                except OSError:
                    break
                if not chunk:
                    break
                out += chunk
            break
        if not sent_keys and b"<SEND-KEYS>" in out:
            os.write(master, b"a\x1b[Ob\x1b[Ic")
            sent_keys = True
        for _ in range(out.count(QUERY) - queries):
            os.write(master, (b"\x1b[?0u" if kitty else b"") + b"\x1b[?62c")
            queries += 1
        for _ in range(out.count(b"<SNAP>") - len(snaps)):
            snaps.append(termios.tcgetattr(slave))
            os.write(master, b"\n")
    else:
        child.kill()
        failures.append(f"{scenario}: timed out")
    after = termios.tcgetattr(slave)
    os.close(master)
    os.close(slave)
    return out, child.wait(), before, snaps, after


def check(scenario, condition, what):
    if not condition:
        failures.append(f"{scenario}: {what}")


def session(scenario, out, before, after, kitty, mouse, sessions):
    focus = scenario in ("forwarding", "forwarding_panic", "startup_kitty")
    check(scenario, out.count(FOCUS_ON) == out.count(FOCUS_OFF) == int(focus), "focus mismatch")
    teardown = (FOCUS_OFF if focus else b"") + (POP if kitty else b"") + (MOUSE_OFF if mouse else b"") + LEAVE + SHOW
    check(scenario, after == before, "termios not restored to exact baseline")
    check(scenario, out.count(ENTER) == out.count(LEAVE) == sessions, "not one leave per enter")
    check(scenario, out.count(teardown) == sessions, f"teardown not {teardown!r} x{sessions}")
    check(scenario, out.count(PUSH) == out.count(POP) == (sessions if kitty else 0), "kitty mismatch")
    check(scenario, out.count(MOUSE_ON) == out.count(MOUSE_OFF) == (sessions if mouse else 0), "mouse mismatch")
    if sessions == 1:
        for setup in [PUSH] * kitty + [MOUSE_ON] * mouse + [FOCUS_ON] * focus:
            check(scenario, out.find(ENTER) < out.find(setup), f"{setup!r} before enter")
        if PANIC in out:
            panic = out.index(PANIC)
            check(scenario, out.rfind(teardown) + len(teardown) <= panic, "teardown after panic")
            # Ratatui's `Terminal::drop` may emit a harmless `SHOW` during unwind.
            check(
                scenario,
                b"\x1b" not in out[panic:].replace(SHOW, b""),
                f"unexpected escapes after panic: {out[panic:]!r}",
            )


def scenario(name, kitty=False, mouse=False, sessions=1, **run_args):
    mouse = mouse and bool(os.environ.get("PTY_MOUSE", "1"))
    out, rc, before, snaps, after = run(name, kitty, **run_args)
    if os.environ.get("PTY_VERBOSE"):
        print(f"--- {name} (exit {rc})\n{out!r}")
    sessions = 1 if rc == ABORTED else sessions
    session(name, out, before, after, kitty, mouse, sessions)
    return out, rc, before, snaps


out, rc, *_ = scenario("exit", kitty=True, mouse=True)
ran = out.find(b"<RUN-RETURNED>")
check("exit", rc in (101, ABORTED) and -1 < ran < out.find(PANIC), "no post-exit panic")
check("exit", b"\x1b" not in out[ran:], "escapes after app exit")

out, rc, before, snaps = scenario("retained_exit", kitty=True, mouse=True)
alive = out.find(b"<APP-ALIVE>")
check("retained_exit", rc == 0 and snaps == [before], "termios not restored while App alive")
check("retained_exit", alive > out.rfind(LEAVE) and b"\x1b" not in out[alive:], "escapes after AppExit")

out, rc, *_ = scenario("panic", kitty=True, mouse=True)
check("panic", rc in (101, ABORTED), f"unexpected exit {rc}")

out, rc, *_ = scenario("worker_panic", mouse=True)
check("worker_panic", QUERY in out and b"worker thread panic" in out, "missing query or panic")

out, rc, *_ = scenario("abort", kitty=True, mouse=True)
check("abort", rc == ABORTED and out.find(LEAVE) < out.find(b"<USER-HOOK>"), "hook order")

out, rc, *_ = scenario("hook_after")
check("hook_after", b"<LATE-HOOK>" in out and PANIC in out, "late hook not preserved")

if os.environ.get("PTY_KEYBOARD", "1"):
    out, rc, *_ = scenario("focus_unavailable")
    check("focus_unavailable", rc == 0, f"startup failed: {out!r}")
    out, rc, *_ = scenario("forwarding", kitty=True)
    check("forwarding", rc == 0, f"unexpected exit {rc}: {out!r}")
    out, rc, *_ = scenario("forwarding_panic", kitty=True)
    check("forwarding_panic", rc in (101, ABORTED) and b"panic with focus reporting enabled" in out, "panic path")

# A context that exists before `ContextSystems` is kept: no Kitty or mouse modes.
for name in ("preexisting_insert", "preexisting_pre_startup"):
    out, rc, *_ = scenario(name, kitty=False, mouse=False)
    check(name, rc == 0 and QUERY not in out, f"unexpected exit {rc} or Kitty query: {out!r}")

out, rc, *_ = scenario("startup_context", kitty=True, mouse=True)
check("startup_context", rc == 0, f"unexpected exit {rc}: {out!r}")

if os.environ.get("PTY_KEYBOARD", "1"):
    out, rc, *_ = scenario("startup_kitty", kitty=True)
    check("startup_kitty", rc == 0, f"unexpected exit {rc}: {out!r}")

out, rc, *_ = scenario("direct")
check("direct", rc == 0 and out.find(ENTER) < out.find(b"<REFUSED>") < out.find(LEAVE), "refusal")

out, rc, *_ = scenario("resume", kitty=True, mouse=True, sessions=2)
second = out.rfind(ENTER)
stale = out.find(b"<STALE-DROPPED>")
check("resume", rc == 0 and second < stale, "stale marker missing")
check(
    "resume",
    LEAVE not in out[second:stale] and POP not in out[second:stale] and MOUSE_OFF not in out[second:stale],
    "stale drop tore down new session",
)

read_fd, write_fd = os.pipe()
os.close(read_fd)  # `EnterAlternateScreen` fails with EPIPE after raw mode is enabled
# Strip `$TERM` and `PATH` so `tput` cannot hide a broken controlling-terminal size lookup.
no_tput = {k: v for k, v in os.environ.items() if k != "TERM"} | {"PATH": "/"}
out, rc, before, snaps, after = run("rollback", stdout=write_fd, env=no_tput)
os.close(write_fd)
if os.environ.get("PTY_VERBOSE"):
    print(f"--- rollback (exit {rc})\n{out!r}")
check("rollback", rc == 0 and snaps == [before, before], "raw mode not rolled back")
check("rollback", after == before, "termios not restored")
check("rollback", b"\x1b" not in out, "wrote escapes to terminal")

out, rc, *_ = scenario("race", sessions=20)
tokens = {ENTER: "h", LEAVE: "l", PANIC: "P"}
found = sorted((m.start(), c) for t, c in tokens.items() for m in re.finditer(re.escape(t), out))
events = "".join(c for _, c in found)
check("race", re.fullmatch("(hlP)+", events) and (len(events) == 60 or rc == ABORTED), events)

for failure in failures:
    print(f"FAIL {failure}")
print(f"pty: {'FAILED' if failures else 'ok'}")
sys.exit(1 if failures else 0)
