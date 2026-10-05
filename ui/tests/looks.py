from PySide6.QtCore import Q_ARG, Q_RETURN_ARG, QMetaObject, QObject, Qt, QUrl
from PySide6.QtGui import QColor
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuick import QQuickWindow  # noqa: F401  (rootObjects() down-cast, for grabWindow)
from PySide6.QtTest import QTest
from uitest import until

from universe_ui import host

LOOKS = ["reprise", "switch2", "ps5"]
# What each look calls its question and its list of choices.
DIALOG = {"reprise": "confirm", "switch2": "dialog", "ps5": "dialog"}
MENU = {"reprise": "gameMenu", "switch2": "picker", "ps5": "popup"}
# On the text sheet: the key that gives the text (Enter under Reprise's keyboard mode, where F types an f; + and Options on the
# consoles'), and the one that shows a password.
SUBMIT = {"reprise": Qt.Key.Key_Return, "switch2": Qt.Key.Key_F1, "ps5": Qt.Key.Key_F1}
REVEAL = {"reprise": Qt.Key.Key_F1, "switch2": Qt.Key.Key_E, "ps5": Qt.Key.Key_E}
TABS = {"homePage", "mediaPage", "settingsPage", "libraryPage"}


def lit_fraction(image, ground):
    small = image.scaled(96, 54)
    r, g, b = QColor(ground).getRgb()[:3]
    lit = 0
    for y in range(small.height()):
        for x in range(small.width()):
            c = small.pixelColor(x, y)
            if abs(c.red() - r) + abs(c.green() - g) + abs(c.blue() - b) > 60:
                lit += 1
    return lit / (small.width() * small.height())


def settle(window):
    """Until the window draws the scene as it stands; an animation may still be running."""
    drawn = []

    def swapped():
        drawn.append(True)

    window.frameSwapped.connect(swapped)
    window.update()
    until(lambda: drawn, "the window drew no frame")
    window.frameSwapped.disconnect(swapped)


def render(api, width=1280, height=720, activate=False):
    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("api", api)
    engine.load(QUrl.fromLocalFile(str(host.QML_DIR / "main.qml")))
    assert engine.rootObjects(), "main.qml failed to load"
    window = engine.rootObjects()[0]
    api.attachWindow(window)
    window.setWidth(width)
    window.setHeight(height)
    if activate:
        window.requestActivate()
    settle(window)
    return engine, window


def root_of(window):
    return until(lambda: window.findChild(QObject, "look").property("item"), "no look loaded")


def read(obj, name):
    value = obj.property(name)
    return value.toVariant() if hasattr(value, "toVariant") else value


def invoke(obj, method, *args):
    return QMetaObject.invokeMethod(obj, method, *(Q_ARG("QVariant", a) for a in args))


def call(obj, method, *args):
    value = QMetaObject.invokeMethod(obj, method, Qt.DirectConnection, Q_RETURN_ARG("QVariant"), *(Q_ARG("QVariant", a) for a in args))
    return value.toVariant() if hasattr(value, "toVariant") else value


def content_rows(page):
    """A Settings page's rows for its open section: Reprise's `content` holds them with its groups, the others' is the list."""
    content = read(page, "content")
    return content["rows"] if isinstance(content, dict) else content


def activate(page, rows, **want):
    """A on the first of `rows` whose keys hold `want`, through the page's own `activate(index, row)`."""
    i = next(i for i, r in enumerate(rows) if all(r.get(k) == v for k, v in want.items()))
    invoke(page, "activate", i, {**rows[i], "form": i})


def current_row(page):
    """The row under a form page's cursor: Reprise's pages call it `row`, the others' `currentRow`."""
    return read(page, "row") or read(page, "currentRow") or {}


def theme_rows(page):
    """The Themes section's rows as the look builds them: Reprise's `content` carries its cards beside them."""
    content = read(page, "content") or []
    return content.get("rows", []) if isinstance(content, dict) else content


def page_name(source):
    stem = source.rpartition("/")[2].removesuffix(".qml")
    return stem[0].lower() + stem[1:]


class Look:
    """A window on one look, and the steps that differ between them: Reprise opens its pages over its tabs, the others push them on a stack."""

    def __init__(self, api, name, width=1280, height=720, activate=True):
        self.api = api
        self.name = name
        self._root = None
        api.theme.set(name)
        api.theme.takeLanding()
        self.engine, self.window = render(api, width, height, activate)

    # Held: PySide invalidates what findChild returned once the wrapper it searched from is freed.
    @property
    def root(self):
        if self._root is None:
            self._root = root_of(self.window)
        return self._root

    @property
    def stacked(self):
        return self.name != "reprise"

    def find(self, name):
        return until(lambda: self.window.findChild(QObject, name), f"no {name}")

    def dialog(self):
        return self.find(DIALOG[self.name])

    def menu(self):
        return self.find(MENU[self.name])

    def page(self, name):
        """The page `name` once it is the one shown."""
        root = self.root
        # A Repeater's pages (Reprise's tabs, the others' stack) are no QObject's children: findChild reaches only Reprise's sub-pages.
        if not self.stacked and name not in TABS:
            return until(lambda: root.findChild(QObject, name), f"{name} is not shown")
        slot = "topPage" if self.stacked else "activePage"
        return until(lambda: (p := root.property(slot)) is not None and p.objectName() == name and p, f"{name} is not shown")

    def open(self, source, args=None):
        invoke(self.root, "push" if self.stacked else "openSub", source, args or {})
        return self.page(page_name(source))

    def home(self):
        return self.page("homePage") if not self.stacked else self.find("homePage")

    def launch(self, game_id):
        game = self.api.allGames.byId(game_id)
        if self.name == "switch2":
            invoke(self.root, "launch", game, None)
        else:
            invoke(self.root, "launch" if self.stacked else "launchGame", game)

    def game(self, game_id):
        """A game page's argument for `game_id`: Reprise takes the game, the others its id."""
        return {"gameId": game_id} if self.stacked else {"game": self.api.allGames.byId(game_id)}

    def settings(self, section):
        if self.stacked:
            return self.open("pages/SettingsPage.qml", {"section": section})
        root = self.root
        root.goToTab(root.property("settingsTab"))
        page = self.page("settingsPage")
        invoke(page, "land", section)
        return page

    def switch(self, name):
        """A live switch: the new look lands on its Themes section."""
        self.api.theme.set(name)
        self._root = None
        self.name = name
        settle(self.window)

    def press(self, key, times=1):
        for _ in range(times):
            QTest.keyClick(self.window, key)

    def close(self):
        """Closes the window and drops the engine now: a failed test's frame would keep it for a later collection to tear down mid-test."""
        settle(self.window)
        self.window.close()
        self._root = None
        self.engine = None
