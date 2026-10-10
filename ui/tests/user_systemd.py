"""The user's systemd as the launcher drives it, a python-dbusmock template: a game's transient unit runs, a stop SIGTERMs its
process group, ExecStopPost closes the session, and every start and stop-post lands in the `log` parameter's JSON lines."""

import contextlib
import ctypes
import json
import os
import signal
import subprocess

import dbus
from dbusmock import mockobject

BUS_NAME = "org.freedesktop.systemd1"
MAIN_OBJ = "/org/freedesktop/systemd1"
MAIN_IFACE = "org.freedesktop.systemd1.Manager"
UNIT_IFACE = "org.freedesktop.systemd1.Unit"
SERVICE_IFACE = "org.freedesktop.systemd1.Service"
SYSTEM_BUS = False
NO_SUCH_UNIT = "org.freedesktop.systemd1.NoSuchUnit"
GAME = "universe-game-"
POLL_MS = 20

# GLib through ctypes, as dbusmock's own main loop: the test environment has no PyGObject.
GLIB = ctypes.CDLL("libglib-2.0.so.0")
SOURCE_FUNC = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p)
GLIB.g_timeout_add.argtypes = [ctypes.c_uint, SOURCE_FUNC, ctypes.c_void_p]
# Kept for the process's life: GLib may still be inside one when it says it is done.
CALLBACKS = []


def later(ms, fn, *args):
    """`fn(*args)` on the mock's main loop in `ms`, and again every `ms` while it returns True."""
    callback = SOURCE_FUNC(lambda _data: bool(fn(*args)))
    CALLBACKS.append(callback)
    GLIB.g_timeout_add(ms, callback, None)


def load(mock, parameters):
    mock.units_log = str(parameters["log"])
    mock.next_job = 1
    mock.units = {}
    mock.running = {}
    mock.AddProperties(MAIN_IFACE, {"Version": "258"})


def _write(mock, entry):
    with open(mock.units_log, "a") as f:
        f.write(json.dumps(entry) + "\n")


def _job(mock):
    job_id = mock.next_job
    mock.next_job += 1
    return job_id, dbus.ObjectPath(f"{MAIN_OBJ}/job/{job_id}")


def _job_done(mock, job_id, job, name):
    mock.EmitSignal(MAIN_IFACE, "JobRemoved", "uoss", [dbus.UInt32(job_id), job, name, "done"])


def _argv(props, key):
    return [str(a) for a in props[key][0][1]] if props.get(key) else []


def _no_such_unit(name):
    return dbus.exceptions.DBusException(f"Unit {name} not loaded.", name=NO_SUCH_UNIT)


@dbus.service.method(MAIN_IFACE, in_signature="", out_signature="")
def Subscribe(self):
    pass


@dbus.service.method(MAIN_IFACE, in_signature="ssa(sv)a(sa(sv))", out_signature="o")
def StartTransientUnit(self, name, _mode, properties, _aux):
    name, props = str(name), {str(k): v for k, v in properties}
    env = dict(str(kv).split("=", 1) for kv in props.get("Environment", []))
    for key in props.get("UnsetEnvironment", []):
        env.pop(str(key), None)
    cwd = str(props.get("WorkingDirectory") or "") or None
    argv = _argv(props, "ExecStart")
    job_id, job = _job(self)
    pid = None
    if name.startswith(GAME) and argv:
        # Its own process group, as a unit's own cgroup: a stop reaches the shell's children too.
        proc = subprocess.Popen(argv, env=env, cwd=cwd, start_new_session=True, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        pid = proc.pid
        path = f"{MAIN_OBJ}/unit/{job_id}"
        self.AddObject(path, UNIT_IFACE, {"Id": name, "ActiveState": "active"}, [])
        mockobject.objects[path].AddProperties(SERVICE_IFACE, {"ControlGroup": ""})
        self.units[name] = path
        self.running[name] = {"proc": proc, "post": _argv(props, "ExecStopPost"), "env": env, "stop": None}
        later(POLL_MS, _watch, self, name)
    _write(self, {"unit": name, "argv": argv, "env": env, "cwd": cwd, "pid": pid})
    later(1, _job_done, self, job_id, job, name)
    return job


def _watch(mock, name):
    unit = mock.running[name]
    code = unit["proc"].poll()
    if code is None:
        return True
    end = ("killed", signal.Signals(-code).name.removeprefix("SIG")) if code < 0 else ("exited", str(code))
    mockobject.objects[mock.units[name]].UpdateProperties(UNIT_IFACE, {"ActiveState": "deactivating"})
    if unit["post"]:
        unit["stderr"] = f"{mock.units_log}.{name}.stderr"
        env = {**unit["env"], "EXIT_CODE": end[0], "EXIT_STATUS": end[1]}
        with open(unit["stderr"], "wb") as err:
            unit["post_proc"] = subprocess.Popen(unit["post"], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=err)
    later(POLL_MS, _closing, mock, name)
    return False


def _closing(mock, name):
    unit = mock.running[name]
    post = unit.get("post_proc")
    if post is not None:
        if post.poll() is None:
            return True
        with open(unit["stderr"], "rb") as err:
            _write(mock, {"unit": name, "stop_post": post.returncode, "stderr": err.read().decode(errors="replace")[-4000:]})
    del mock.running[name]
    mock.RemoveObject(mock.units.pop(name))
    if unit["stop"]:
        _job_done(mock, *unit["stop"], name)
    return False


@dbus.service.method(MAIN_IFACE, in_signature="s", out_signature="o")
def GetUnit(self, name):
    if str(name) not in self.units:
        raise _no_such_unit(name)
    return self.units[str(name)]


@dbus.service.method(MAIN_IFACE, in_signature="ss", out_signature="o")
def StopUnit(self, name, _mode):
    unit = self.running.get(str(name))
    if unit is None:
        raise _no_such_unit(name)
    job_id, job = _job(self)
    unit["stop"] = (job_id, job)
    mockobject.objects[self.units[str(name)]].UpdateProperties(UNIT_IFACE, {"ActiveState": "deactivating"})
    with contextlib.suppress(ProcessLookupError):
        os.killpg(unit["proc"].pid, signal.SIGTERM)
    return job


@dbus.service.method(MAIN_IFACE, in_signature="s", out_signature="")
def FreezeUnit(self, _name):
    pass


@dbus.service.method(MAIN_IFACE, in_signature="s", out_signature="")
def ThawUnit(self, _name):
    pass
