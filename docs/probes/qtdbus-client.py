import sys
import time
import json

from PySide6.QtCore import QCoreApplication, QObject, Slot, QTimer, SLOT
from PySide6.QtDBus import QDBusConnection, QDBusInterface, QDBusMessage, QDBusReply, QDBusArgument

SERVICE = "io.github.ilyasturki.Universe"
PATH = "/io/github/ilyasturki/Universe"
IFACE = "io.github.ilyasturki.Universe.Session1"

events = {
    "SessionStarted": None,
    "SessionEnded": None,
    "PropertiesChanged_1": None,
    "PropertiesChanged_2": None,
}
pc_count = 0


def describe(v, indent=""):
    print(f"{indent}type={type(v).__name__} repr={v!r}")
    if isinstance(v, QDBusArgument):
        print(f"{indent}  -> QDBusArgument, currentType={v.currentType()}")


class Receiver(QObject):
    @Slot(str, str)
    def on_session_started(self, session_id, slug):
        t = time.monotonic()
        print(f"[{t:.3f}] SIGNAL SessionStarted session_id={session_id!r} slug={slug!r}")
        events["SessionStarted"] = t
        maybe_quit()

    @Slot(str, str, "uint")
    def on_session_ended(self, session_id, slug, duration_s):
        t = time.monotonic()
        print(f"[{t:.3f}] SIGNAL SessionEnded session_id={session_id!r} slug={slug!r} duration_s={duration_s!r} (type={type(duration_s).__name__})")
        events["SessionEnded"] = t
        maybe_quit()

    @Slot(QDBusMessage)
    def on_properties_changed(self, msg):
        global pc_count
        t = time.monotonic()
        args = msg.arguments()
        iface_name = args[0]
        changed = args[1]
        invalidated = args[2]
        pc_count += 1
        print(f"[{t:.3f}] SIGNAL PropertiesChanged #{pc_count} iface={iface_name!r} invalidated={invalidated!r}")
        describe(changed, "    changed ")
        if isinstance(changed, dict):
            for k, val in changed.items():
                print(f"      key={k!r}")
                describe(val, "        ")
        if pc_count == 1:
            events["PropertiesChanged_1"] = t
        elif pc_count == 2:
            events["PropertiesChanged_2"] = t
        maybe_quit()


def maybe_quit():
    if all(events.values()):
        print("All 4 events received, quitting early.")
        QTimer.singleShot(50, app.quit)


# NOTE (finding, see report): QDBusArgument in PySide6 6.11 exposes only
# asVariant() for reading -- no asString()/asInt32()/asBool() typed getters
# (unlike C++ QDBusArgument's operator>> overloads). For an a{sv} MapEntryType,
# the key's D-Bus type is plain 's' (not 'v'); calling asVariant() on it does
# NOT raise -- it silently returns None and prints a
# "pointerToPython(): SbkConverter::pointerToPython is null for
# shiboken6.Shiboken.VoidPtr" RuntimeWarning. The C++-style `argument >> key`
# stream extraction IS exposed as __rshift__, but calling it on a dict_entry
# sub-argument aborts the whole process with a libdbus assertion
# ("type dict_entry 101 not a basic type") -- do not use it.


app = QCoreApplication(sys.argv)

bus = QDBusConnection.sessionBus()
if not bus.isConnected():
    print("ERROR: cannot connect to session bus")
    sys.exit(1)

receiver = Receiver()

ok1 = bus.connect(
    SERVICE, PATH, IFACE, "SessionStarted",
    receiver, SLOT("on_session_started(QString,QString)"),
)
print(f"connect SessionStarted -> {ok1}")

ok2 = bus.connect(
    SERVICE, PATH, IFACE, "SessionEnded",
    receiver, SLOT("on_session_ended(QString,QString,uint)"),
)
print(f"connect SessionEnded -> {ok2}")

ok3 = bus.connect(
    SERVICE, PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged",
    receiver, SLOT("on_properties_changed(QDBusMessage)"),
)
print(f"connect PropertiesChanged (QDBusMessage slot) -> {ok3}")

iface = QDBusInterface(SERVICE, PATH, IFACE, bus)
if not iface.isValid():
    print(f"ERROR: interface not valid: {iface.lastError().message()}")
    sys.exit(1)

print("\n=== property() reads (QDBusInterface.property) ===")
version_p = iface.property("Version")
current_p = iface.property("Current")
tags_p = iface.property("Tags")
print("Version:", version_p, type(version_p))
print("Current:", current_p, type(current_p))
print("Tags:", tags_p, type(tags_p))
describe(tags_p, "  tags ")

print("\n=== Properties.Get (org.freedesktop.DBus.Properties) ===")
props_iface = QDBusInterface(SERVICE, PATH, "org.freedesktop.DBus.Properties", bus)
for prop_name in ("Version", "Current", "Tags"):
    reply = props_iface.call("Get", IFACE, prop_name)
    raw = reply.arguments()[0]
    print(f"Get {prop_name} raw={raw!r} type={type(raw).__name__}")
    if hasattr(raw, "variant"):
        unwrapped = raw.variant()
        print(f"  .variant() -> {unwrapped!r} type={type(unwrapped).__name__}")

print("\n=== Info (a{sv}) ===")
reply = iface.call("Info", "the-technomancer")
if reply.type() == QDBusMessage.MessageType.ErrorMessage:
    print("ERROR:", reply.errorMessage())
else:
    args = reply.arguments()
    print("raw arguments():", args)
    info = args[0]
    print("top-level type:", type(info).__name__)
    describe(info, "  info ")
    if isinstance(info, dict):
        for k, v in info.items():
            print(f"  key={k!r} value={v!r} pytype={type(v).__name__}")
            if isinstance(v, QDBusArgument):
                print("    -> nested QDBusArgument, currentType:", v.currentType())
    elif isinstance(info, QDBusArgument):
        print("  Info came back as raw QDBusArgument -- manual demarshalling attempt")
        info.beginMap()
        n = 0
        while not info.atEnd():
            info.beginMapEntry()
            print(f"    entry currentType={info.currentType()} sig={info.currentSignature()!r}")
            k = info.asVariant()  # a{sv} key sub-type is 's', not 'v' -- see note above
            print(f"    key via asVariant() = {k!r} (expected a string; None means broken)")
            info.endMapEntry()
            n += 1
            if n > 6:
                print("    (bailing out, entries not advancing as expected)")
                break
        info.endMap()
        print("  CONCLUSION: manual a{sv} demarshalling via QDBusArgument is not usable"
              " as-is in PySide6 6.11 for the string key of a dict entry.")

print("\n=== InfoJson (s, JSON) ===")
reply = iface.call("InfoJson", "the-technomancer")
raw = reply.arguments()[0]
print("raw:", raw, type(raw))
parsed = json.loads(raw)
print("parsed:", parsed)

print("\n=== Launch (triggers SessionStarted +1s, PropertiesChanged x2, SessionEnded +4s) ===")
t_launch = time.monotonic()
reply = iface.call("Launch", "the-technomancer", "DP-1")
session_id = reply.arguments()[0]
print(f"[{t_launch:.3f}] Launch returned session_id={session_id!r}")

QTimer.singleShot(8000, app.quit)

print("\n=== waiting for signals (up to 8s) ===")
app.exec()

print("\n=== event summary ===")
for k, v in events.items():
    delta = (v - t_launch) if v is not None else None
    print(f"  {k}: t={v} delta_from_launch={delta}")
missing = [k for k, v in events.items() if v is None]
if missing:
    print(f"MISSING EVENTS: {missing}")
else:
    print("ALL 4 EVENTS RECEIVED")

print("\n=== 100x InfoJson round-trip timing ===")
N = 100
t0 = time.perf_counter()
for i in range(N):
    r = iface.call("InfoJson", "the-technomancer")
    _ = r.arguments()[0]
t1 = time.perf_counter()
total_ms = (t1 - t0) * 1000
print(f"total={total_ms:.2f}ms for {N} calls -> {total_ms / N:.3f} ms/call")
