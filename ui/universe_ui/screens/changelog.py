import html
import re
from datetime import date

from PySide6.QtCore import QObject, Signal, Slot

from ..qt import QVARIANT, Property

INLINE = re.compile(r"`(?P<code>[^`]+)`|\*\*(?P<strong>.+?)\*\*|\[(?P<text>[^\]]+)\]\([^)]+\)")
MONTHS = ("January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December")


def _date_text(value):
    try:
        day = date.fromisoformat(str(value))
    except ValueError:
        return ""
    return f"{day.day} {MONTHS[day.month - 1]} {day.year}"


def styled(item):
    """An entry's Markdown as Qt's RichText: its code keeps the line's size, which MarkdownText shrinks."""
    out, at = [], 0
    for m in INLINE.finditer(item):
        out.append(html.escape(item[at : m.start()], quote=False))
        if m["code"] is not None:
            out.append(f'<span style="font-family: monospace">{html.escape(m["code"], quote=False)}</span>')
        elif m["strong"] is not None:
            out.append(f"<b>{html.escape(m['strong'], quote=False)}</b>")
        else:
            out.append(html.escape(m["text"], quote=False))
        at = m.end()
    out.append(html.escape(item[at:], quote=False))
    return "".join(out)


def _release(r):
    return {
        "version": str(r.get("version") or ""),
        "dateText": _date_text(r.get("date")),
        "sections": [{"title": str(s.get("title") or ""), "items": [styled(str(i)) for i in s.get("items") or []]} for s in r.get("sections") or []],
    }


class Changelog(QObject):
    pendingChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._releases = None
        # Built once per start: the core records this version as seen whether the page shows or not.
        self._pending = [_release(r) for r in client.whatsNew()]

    def _all(self):
        if self._releases is None:
            self._releases = [_release(r) for r in self._client.changelog()]
        return self._releases

    @Slot()
    def dismiss(self):
        if self._pending:
            self._pending = []
            self.pendingChanged.emit()

    releases = Property(QVARIANT, _all, constant=True)
    pending = Property(QVARIANT, lambda self: self._pending, notify=pendingChanged)
