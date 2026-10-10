"""Walks every screen of a look by its keys, on the fake core, and writes what it found: the graph, a shot per state, the findings."""

import hashlib
import json
import re
import sys
import time
import traceback
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path

from looks import Look
from PySide6.QtCore import QElapsedTimer, QObject, QRectF, Qt
from PySide6.QtQml import QQmlExpression
from PySide6.QtQuick import QQuickItem
from PySide6.QtTest import QTest
from uitest import pump, until

from universe_ui import host

KEYS = {
    "Up": Qt.Key.Key_Up,
    "Down": Qt.Key.Key_Down,
    "Left": Qt.Key.Key_Left,
    "Right": Qt.Key.Key_Right,
    "A": Qt.Key.Key_Return,
    "B": Qt.Key.Key_Escape,
}
MOVES = {"Up", "Down", "Left", "Right"}
# Pressed only where the hint bar shows them.
HINTED = {"X": Qt.Key.Key_I, "Y": Qt.Key.Key_F, "Start": Qt.Key.Key_F1, "LB": Qt.Key.Key_Q, "RB": Qt.Key.Key_E}
# B presses from any state to home, at most.
HOME_WITHIN = 6
# Cursor states expanded per region before its moves wait for the rest: a grid's every cell is not a new screen.
PER_REGION = 10
# Suspects of B never getting home walked again, at most: each costs a new window.
CONFIRMED = 15
# Tries of a button in a region that found no page earlier ones had: the button is worked out there.
SATURATED = 6
# Longer text is an excerpt, cut on purpose.
EXCERPT = 60
# A property that names data rather than where the cursor stands.
NOT_CURSOR = {"shown", "guestId", "lastSession", "launchedSession", "openSound", "title", "alt", "addonsShown", "subSource", "lastShown", "settingsLanding"}
# Text typed on a keyboard: whether there is some, not what, or every letter is a new state.
TYPED = {"query", "text", "typed", "search"}
LOOK_DIRS = {"reprise": [".", "pages", "ui"], "switch2": ["switch2", "switch2/pages", "switch2/ui"], "ps5": ["ps5", "ps5/pages", "ps5/ui"]}

SMALL = (160, 90)
TINY = (80, 45)
BUTTONS = {"X", "Y", "Start"}

# Run with the window's content item as scope; %d is the longest label length checked.
TRUNCATED = """(function () {
    var out = [];
    function walk(item, opacity) {
        opacity *= item.opacity;
        if (!item.visible || opacity < 0.05)
            return;
        // A path is elided on purpose: its end or middle is what tells it apart.
        var path = /^[\\/~]/.test(String(item.text));
        if (item.truncated === true && item.elide !== undefined && !path && String(item.text).length <= %d) {
            var at = item.mapToItem(null, 0, 0);
            if (at.x < width && at.y < height && at.x + item.width > 0 && at.y + item.height > 0)
                out.push(String(item.text));
        }
        for (var i = 0; i < item.children.length; i++)
            walk(item.children[i], opacity);
    }
    for (var i = 0; i < children.length; i++)
        walk(children[i], 1);
    return out;
})()"""


@dataclass
class State:
    key: str
    page: str
    chain: list
    cursor: dict
    path: list
    home: bool
    keys: list
    shot: str = ""
    edges: dict = field(default_factory=dict)
    findings: list = field(default_factory=list)
    expanded: bool = False
    picture: object = None
    region: str = ""

    def untried(self):
        return [k for k in self.keys if k not in self.edges]


def _small(window):
    return window.grabWindow().scaled(*SMALL, Qt.AspectRatioMode.IgnoreAspectRatio, Qt.TransformationMode.SmoothTransformation)


def changed_pixels(a, b, tolerance=30):
    """How many pixels of two equal-sized images differ by more than `tolerance` (summed over R, G and B)."""
    pa, pb = bytes(a.constBits()), bytes(b.constBits())
    if pa == pb:
        return 0
    return sum(1 for i in range(0, len(pa), 4) if abs(pa[i] - pb[i]) + abs(pa[i + 1] - pb[i + 1]) + abs(pa[i + 2] - pb[i + 2]) > tolerance)


def _qml_type(item):
    name = item.metaObject().className()
    return name.split("_QMLTYPE_")[0] if "_QMLTYPE_" in name else ""


def _cursor_props(item):
    """The writable int, string and bool properties a QML file declares on `item`: where its cursor stands, what it has open."""
    out = {}
    mo = item.metaObject()
    while mo is not None and "_QML" in mo.className():
        for i in range(mo.propertyOffset(), mo.propertyCount()):
            p = mo.property(i)
            name = p.name()
            if not p.isWritable() or p.typeName() not in ("int", "QString", "bool") or name in NOT_CURSOR:
                continue
            value = item.property(name)
            if name in TYPED:
                value = "typed" if value else ""
            elif isinstance(value, str) and (len(value) > 24 or " " in value or "/" in value):
                continue
            out[name] = value
        mo = mo.superClass()
    if mo is not None and mo.className() in ("QQuickListView", "QQuickGridView", "QQuickPathView"):
        out["currentIndex"] = item.property("currentIndex")
    return out


def page_names(look):
    """The objectName of every page the look opens: the pages/ files its QML names, not the components that sit beside them."""
    files = [p for d in LOOK_DIRS[look] for p in (host.QML_DIR / d).glob("*.qml")]
    pages = {p.stem for p in (host.QML_DIR / LOOK_DIRS[look][1]).glob("*.qml")}
    named = {m for f in files for m in re.findall(r'"(?:pages/)?([A-Z]\w*Page)(?:\.qml)?"', f.read_text())}
    return {m[0].lower() + m[1:] for m in named & pages}


class Explorer:
    def __init__(self, api, fake, name, out, width=1280, height=720, presses=3000, budget_s=1800, log=None):
        assert type(fake.core).__name__ == "FakeCore", "the explorer presses A on games: never on the real core"
        self.api = api
        self.fake = fake
        self.name = name
        self.out = Path(out)
        self.size = (width, height)
        self.presses = presses
        self.budget_s = budget_s
        self.log = log or list
        self.states = {}
        self.findings = []
        self.steps = 0
        self.resets = 0
        self.look = None
        self.current = None
        self.started = 0.0
        self.alias = {}
        self.retired = []
        self.mismatches = []
        self.discovered = {}
        self.by_region = {}
        self.active = {}
        self.last = None
        self._slot_errors = []
        self._quits = []
        self._launches = []
        fake.launchRequested.connect(self._launches.append)

    # The window ------------------------------------------------------------------------------------------------

    def open(self):
        if self.look is not None:
            # Kept, not dropped: an engine torn down mid-walk deletes objects PySide still wraps.
            self.look.window.close()
            self.retired.append(self.look)
        self.look = Look(self.api, self.name, *self.size)
        self.look.engine.quit.connect(lambda: self._quits.append(True))
        self.settle()

    def settle(self, max_ms=2500):
        """Until the window stops drawing, or its picture stops changing where something always moves."""
        window = self.look.window
        timer = QElapsedTimer()
        timer.start()
        drawn = [0]

        def swapped():
            drawn[0] = timer.elapsed()

        window.frameSwapped.connect(swapped)
        prev, quiet, sampled = _small(window), 0, 0
        try:
            while timer.elapsed() < max_ms:
                pump(20)
                # A press can start its transition a frame or two late: 150 ms before calling a still window settled.
                if timer.elapsed() >= 150 and timer.elapsed() - drawn[0] >= 100:
                    return True
                if timer.elapsed() - sampled >= 60:
                    sampled = timer.elapsed()
                    cur = _small(window)
                    quiet = quiet + 1 if changed_pixels(prev, cur) < 3 else 0
                    prev = cur
                    if quiet >= 3:
                        return True
            return False
        finally:
            window.frameSwapped.disconnect(swapped)

    def press(self, key):
        QTest.keyClick(self.look.window, HINTED.get(key) or KEYS[key])
        self.steps += 1
        self.settle()
        # A launch plays its overlay before it asks the core.
        until(
            lambda: self._launches or self.api.theme.current != self.name or not self.look.root.property("launching"),
            "the launch overlay never ended",
            10000,
        )

    # What the window shows -------------------------------------------------------------------------------------

    def hints(self):
        root = self.look.root
        if self.look.stacked:
            hints = root.property("hints")
        else:
            bar = self.look.window.findChild(QObject, "hintBar")
            hints = bar.property("hints") if bar is not None else []
        hints = hints.toVariant() if hasattr(hints, "toVariant") else hints
        glyphs = set()
        for h in hints or []:
            glyphs.update(str(h.get("glyph", "")).split())
        return glyphs

    def is_home(self):
        root = self.look.root
        if self.look.stacked:
            return bool(root.property("onHome")) and not root.property("modal")
        overlays = [self.look.window.findChild(QObject, n) for n in ("confirm", "prompts")]
        page = root.property("activePage")
        return (
            root.property("tabIndex") == 0
            and not any(root.property(p) for p in ("subOpen", "detailOpen", "searchOpen", "menuOpen", "launching"))
            and not any(o is not None and o.property("open") for o in overlays)
            and not (page is not None and page.property("modal"))
        )

    def observe(self):
        """The state on screen: the named items the focus sits in, and their cursors."""
        window = self.look.window
        focus = window.activeFocusItem()
        chain, cursor = [], {}
        item = focus
        # The look's own root holds global state, not a cursor.
        while isinstance(item, QQuickItem) and isinstance(item.parentItem(), QQuickItem) and item.parentItem().objectName() != "look":
            if item.objectName():
                chain.append(item.objectName())
            for k, v in _cursor_props(item).items():
                cursor.setdefault(f"{item.objectName() or _qml_type(item) or 'item'}.{k}", v)
            item = item.parentItem()
        chain.reverse()
        if focus is not None and not focus.objectName() and _qml_type(focus):
            chain.append(_qml_type(focus))
        page = next((n for n in chain if n.endswith("Page")), chain[0] if chain else "")
        if self.look.stacked and page == "bottomBar":
            page = "homePage"
        key = "/".join(chain) + " " + " ".join(f"{k}={v}" for k, v in sorted(cursor.items()))
        return key, page, chain, cursor, focus

    def checks(self, state, focus):
        window = self.look.window
        found = []
        if focus is None:
            found.append(("no-focus", "no item has the active focus"))
        elif not state.chain:
            found.append(("no-focus", "no page or overlay holds the focus: it sits on the look itself"))
        else:
            rect = focus.mapRectToScene(QRectF(0, 0, focus.width(), focus.height()))
            screen = QRectF(0, 0, window.width(), window.height())
            if not focus.isVisible():
                found.append(("focus-hidden", f"the focused {state.chain[-1] if state.chain else 'item'} is not visible"))
            elif focus.width() > 0 and focus.height() > 0 and not rect.intersects(screen):
                found.append(("focus-off-screen", f"the focused {state.chain[-1] if state.chain else 'item'} is off screen at {rect.x():.0f},{rect.y():.0f}"))
        found.extend(("truncated", text) for text in self.truncated())
        return found

    def truncated(self):
        """The cut short labels on screen. Walked in QML: a Python wrapper per item would outlive the items a page unloads."""
        found = QQmlExpression(self.look.engine.rootContext(), self.look.window.contentItem(), TRUNCATED % EXCERPT).evaluate()[0]
        found = found.toVariant() if hasattr(found, "toVariant") else found
        return [str(t) for t in found or []]

    # The walk ----------------------------------------------------------------------------------------------------

    def record(self, path, move=None):
        """The state on screen, a known one when only a cursor the screen does not show tells them apart."""
        key, page, chain, cursor, focus = self.observe()
        # A region is the named items and the zone-like strings; its cursors are the values moves within it change.
        region = "/".join(chain) + " " + " ".join(f"{k}={v}" for k, v in sorted(cursor.items()) if isinstance(v, str))
        last, self.last = self.last, (region, cursor)
        if move in MOVES and last is not None and last[0] == region:
            self.active.setdefault(region, set()).update(k for k in cursor.keys() | last[1].keys() if cursor.get(k) != last[1].get(k))
        if key in self.alias:
            return self.states[self.alias[key]]
        active = self.active.get(region, set())
        picture = _small(self.look.window).scaled(*TINY)
        for known in self.by_region.get(region, ()):
            if all(known.cursor.get(k) == cursor.get(k) for k in active) or changed_pixels(known.picture, picture) <= 2:
                self.alias[key] = known.key
                return known
        hinted = self.hints()
        # A screen that hints nothing may still answer them: the PS5's home hides its strip.
        keys = list(KEYS) + sorted(k for k in HINTED if k in hinted or (not hinted and k in BUTTONS))
        state = State(key, page, chain, cursor, list(path), self.is_home(), keys)
        state.picture = picture
        state.region = region
        self.by_region.setdefault(state.region, []).append(state)
        self.states[key] = state
        if page not in self.discovered:
            self.discovered[page] = [self.steps, round(time.monotonic() - self.started)]
        self.alias[key] = key
        state.findings = self.checks(state, focus)
        shot = hashlib.sha1(f"{self.name} {key}".encode()).hexdigest()[:12] + ".png"
        self.look.window.grabWindow().save(str(self.out / "shots" / shot))
        state.shot = shot
        return state

    def step(self, key):
        before = self.current
        errors = len(self._slot_errors)
        self._launches.clear()
        self.press(key)
        if self._launches:
            # The walk stops at a launch and goes on from a new window.
            before.edges[key] = "launch:" + self._launches[0]
            self.open()
            self.current = self.record([])
        elif self.api.theme.current != self.name:
            # Picked another look: that one gets its own walk.
            before.edges[key] = "look:" + self.api.theme.current
            self.open()
            self.current = self.record([])
        elif self._quits:
            self._quits.clear()
            before.edges[key] = "quit"
            self.open()
            self.current = self.record([])
        else:
            self.current = self.record([*before.path, key], key)
            before.edges[key] = self.current.key
        for w in [*self.log(), *self._slot_errors[errors:]]:
            before.findings.append(("warning", f"{key}: {w}"))

    def home(self):
        """Back to home with B, as a player would; a new window when B never gets there."""
        self.resets += 1
        for _ in range(HOME_WITHIN + 2):
            if self.current.home:
                return
            self.step("B")
        self.open()
        self.current = self.record([])

    def b_home(self, state):
        """B pressed from `state`, reached again from a new window, until home: the presses it took, None when home never came."""
        self.open()
        self.current = self.record([])
        for k in state.path:
            self.step(k)
        if self.current.page != state.page:
            return 0
        for presses in range(HOME_WITHIN + 1):
            if self.current.home:
                return presses
            self.step("B")
        return None

    def routes(self, start):
        """The shortest run of keys known to lead from `start` to each state it reaches."""
        seen = {start.key: []}
        queue = deque([start])
        while queue:
            s = queue.popleft()
            for k, target in s.edges.items():
                if target in self.states and target not in seen:
                    seen[target] = [*seen[s.key], k]
                    queue.append(self.states[target])
        return seen

    def go(self, goal):
        """Walks to `goal` along known edges, planning again after each press: a page that remembers its cursor lands elsewhere."""
        shortest = len(self.depth.get(goal.key, goal.path))
        budget = 2 * shortest + 12
        homed = opened = False
        while self.current.key != goal.key and budget > 0:
            keys = self.routes(self.current).get(goal.key)
            if keys is None or (not homed and len(keys) > shortest + 2):
                if not homed:
                    homed = True
                    self.home()
                elif not opened:
                    opened = True
                    self.open()
                    self.current = self.record([])
                else:
                    return False
                continue
            expected = self.current.edges.get(keys[0])
            self.step(keys[0])
            budget -= 1
            if self.current.key != expected:
                self.mismatches.append({"key": keys[0], "expected": expected, "got": self.current.key})
        return self.current.key == goal.key

    def outcome(self, state, key):
        """What `key` did from `state`: the page it led to, or the launch, look or quit it set off."""
        target = state.edges.get(key, "")
        known = self.states.get(target)
        return known.page if known is not None else target.partition(":")[0]

    def saturated(self, state, key):
        """Whether `key` in this state's region stopped finding pages: its last tries led only where earlier ones had."""
        tries = [self.outcome(s, key) for s in self.by_region.get(state.region, ()) if key in s.edges]
        return len(tries) >= SATURATED and set(tries[:-SATURATED]) >= set(tries[-SATURATED:]) and bool(tries[:-SATURATED])

    def next_goal(self, failed):
        """The next state and key: a button the region never tried, then moves and buttons that still find pages, the nearest first."""
        self.depth = self.routes(self.origin)
        near = self.routes(self.current)
        expanded = {}
        for s in self.states.values():
            if s.expanded:
                expanded[s.region] = expanded.get(s.region, 0) + 1
        best = None
        for s in self.states.values():
            untried = s.untried()
            if not untried or failed.get(s.key, 0) >= 3:
                continue
            far = len(near[s.key]) if s.key in near else 99
            crowded = not s.expanded and expanded.get(s.region, 0) >= PER_REGION
            for key in untried:
                if key in MOVES:
                    rank = 9 if crowded else 1
                elif not any(key in t.edges for t in self.by_region[s.region]):
                    rank = 0
                else:
                    rank = 9 if crowded or self.saturated(s, key) else 1
                # Nearest first: a button the region never tried is worth a walk of four presses; at a tie, a button before a move.
                order = (4 * rank + min(far, 20), key in MOVES, len(self.depth.get(s.key, s.path)))
                if best is None or order < best[0]:
                    best = (order, s, key)
        return best[1:] if best else (None, None)

    def crawl(self):
        (self.out / "shots").mkdir(parents=True, exist_ok=True)
        previous_hook = sys.excepthook

        def hook(*exc):
            self._slot_errors.append("".join(traceback.format_exception(*exc)).strip().splitlines()[-1])
            previous_hook(*exc)

        sys.excepthook = hook
        self.started = time.monotonic()
        try:
            self.open()
            self.current = self.origin = self.record([])
            self.depth = {}
            failed = {}
            while self.steps < self.presses and time.monotonic() - self.started < self.budget_s:
                goal, key = self.next_goal(failed)
                if goal is None:
                    break
                if not self.go(goal):
                    failed[goal.key] = failed.get(goal.key, 0) + 1
                    continue
                goal.expanded = True
                if key in goal.untried():
                    self.step(key)
            self.complete = goal is None if self.states else False
            self.judge()
        finally:
            sys.excepthook = previous_hook
            if self.look is not None:
                self.look.close()
            for look in self.retired:
                look.engine = None
        self.write()
        return self.findings

    # What it found -------------------------------------------------------------------------------------------------

    def judge(self):
        states = self.states
        shortest = self.routes(self.origin)
        for s in states.values():
            s.path = shortest.get(s.key, s.path)
        for s in states.values():
            for kind, detail in s.findings:
                self.findings.append({"kind": kind, "state": s.key, "page": s.page, "path": s.path, "detail": detail})
        # B, pressed again and again, reaches home; any state home cannot be reached from is a trap. The graph only suspects:
        # a page that remembers its cursor makes cycles of states it merged, so each suspect is walked again from a new window.
        back = {k: set() for k in states}
        for s in states.values():
            for target in s.edges.values():
                if target in back:
                    back[target].add(s.key)
        reach, queue = set(), deque(k for k, s in states.items() if s.home)
        reach.update(queue)
        while queue:
            k = queue.popleft()
            for prev in back[k] - reach:
                reach.add(prev)
                queue.append(prev)
        suspects = {}
        for s in states.values():
            if s.edges and not s.untried() and all(t == s.key for t in s.edges.values()):
                self.findings.append({"kind": "dead-end", "state": s.key, "page": s.page, "path": s.path, "detail": "no key leaves it"})
                continue
            # B on a keyboard deletes what was typed first.
            if s.home or "B" not in s.edges or "typed" in s.cursor.values():
                continue
            at, presses, seen = s, 0, set()
            while not at.home and presses <= HOME_WITHIN and at.key not in seen:
                seen.add(at.key)
                target = at.edges.get("B")
                if target not in states:
                    break
                at, presses = states[target], presses + 1
            cycles = not at.home and (presses > HOME_WITHIN or at.key in seen or at.edges.get("B") in states)
            if cycles or (not s.untried() and s.key not in reach):
                suspects.setdefault(s.region, s)
        for s in list(suspects.values())[:CONFIRMED]:
            if self.b_home(s) is None:
                self.findings.append(
                    {"kind": "b-never-home", "state": s.key, "page": s.page, "path": s.path, "detail": f"B {HOME_WITHIN} times from here never reaches home"}
                )
        reached = {s.page for s in states.values()}
        for page in sorted(page_names(self.name) - reached):
            self.findings.append({"kind": "unreached", "state": "", "page": page, "path": [], "detail": f"no key path reached {page}"})

    def write(self):
        graph = {
            "look": self.name,
            "size": list(self.size),
            "steps": self.steps,
            "resets": self.resets,
            "seconds": round(time.monotonic() - self.started, 1),
            "complete": getattr(self, "complete", False),
            "mismatches": self.mismatches,
            "discovered": self.discovered,
            "states": [
                {"key": s.key, "page": s.page, "home": s.home, "path": s.path, "shot": s.shot, "edges": s.edges, "findings": s.findings, "expanded": s.expanded}
                for s in self.states.values()
            ],
            "findings": self.findings,
        }
        (self.out / "graph.json").write_text(json.dumps(graph, indent=1))
