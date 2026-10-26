from datetime import date

from PySide6.QtCore import Signal, Slot

from ..models import file_url
from ..qt import QVARIANT, Property
from .settings import RowsForm, _group, _plural, _row

BEHIND = "Updates available"
UNSURE = "Can't tell"
CHANNELS = {"pc": "Steam", "switch": "titledb"}


def _day(text):
    try:
        d = date.fromisoformat(text)
    except ValueError:
        return text
    return f"{d.day} {d:%b %Y}"


def _confirm(game, version):
    installed = version.get("installed") or ""
    upstream = CHANNELS.get(version.get("channel") or "", "upstream")
    when = _day(version.get("latest_date") or "")
    facts = [f"Your files say {installed}." if installed else "Your files don't say which version they are."]
    if when:
        facts.append(f"{upstream}'s newest build came out on {when}.")
    return {"message": f"Is {game.get('title')} on its latest version?", "detail": " ".join(facts), "yes": "It's the latest", "no": "Not sure"}


def _game_row(section, game, advanced):
    version = game.get("version") or {}
    media = game.get("media") or {}
    art = next((p for p in (media.get("square"), media.get("box_front")) if p), "")
    row = {
        **_row(section, "update_game", str(game.get("title") or game.get("id")), "action", "", advanced=advanced),
        "display": version.get("installed") or "",
        "gameId": str(game.get("id")),
        "image": file_url(art).toString(),
    }
    if version.get("status") == "update" and version.get("how") == "apply":
        row.update(action="Update", how="apply")
    elif version.get("markable"):
        row.update(action="I have the latest version", how="mark", confirm=_confirm(game, version))
    else:
        row.update(type="info", action="", detail="Update it by hand")
    return row


def build_updates(games):
    shown = sorted((g for g in games if not g.get("hidden")), key=lambda g: str(g.get("title") or "").casefold())
    behind = [g for g in shown if (g.get("version") or {}).get("status") == "update"]
    unsure = [g for g in shown if (g.get("version") or {}).get("status") == "unknown" and (g.get("version") or {}).get("markable")]
    rows = [_game_row(BEHIND, g, False) for g in behind] or [{**_row(BEHIND, "none", "No update available", "info", ""), "display": ""}]
    groups = [_group(BEHIND, range(len(rows)), caps=True, meta=_plural(len(behind), "game") if behind else "")]
    if unsure:
        first = len(rows)
        rows.extend(_game_row(UNSURE, g, True) for g in unsure)
        groups.append(_group(UNSURE, range(first, len(rows)), caps=True, advanced=True, meta="No version to compare with upstream's"))
    info = {"name": "Game updates", "meta": f"{_plural(len(behind), 'game')} behind" if behind else "No update available"}
    return info, rows, groups


class UpdatesForm(RowsForm):
    infoChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(client, parent)
        self._info = {}
        self._open = False
        client.libraryChanged.connect(lambda ids: self._refresh() if self._open else None)

    @Slot(str)
    def load(self, _ident=""):
        self._set_show_advanced(False)
        self._open = True
        self._refresh()

    @Slot()
    def close(self):
        self._open = False

    def _refresh(self):
        self._info, rows, groups = build_updates(self._client.list())
        self._set_rows(rows, groups)
        self.infoChanged.emit()

    info = Property(QVARIANT, lambda self: dict(self._info), notify=infoChanged)
