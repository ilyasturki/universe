import html
import logging
import re
from datetime import date

from PySide6.QtCore import QObject, QUrl, Signal, Slot

from ..qt import QVARIANT, Property

log = logging.getLogger(__name__)

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
        "sections": [
            {"title": str(s.get("title") or ""), "items": [{"text": styled(str(i.get("text") or "")), "media": i.get("media")} for i in s.get("items") or []]}
            for s in r.get("sections") or []
        ],
    }


class Changelog(QObject):
    pendingChanged = Signal()
    opensChanged = Signal()
    filesChanged = Signal()

    def __init__(self, client, parent=None):
        super().__init__(parent)
        self._client = client
        self._releases = None
        self._files = {}
        self._asked = set()
        # Built once per start: the core records this version as seen whether the page shows or not.
        self._pending = [_release(r) for r in client.whatsNew()]
        client.configWritten.connect(self._written)

    def _written(self, key):
        if key == "desktop.whats_new":
            self.opensChanged.emit()

    def _all(self):
        if self._releases is None:
            self._releases = [_release(r) for r in self._client.changelog()]
        return self._releases

    @Slot(str)
    def fetch(self, url):
        """Downloads a highlight's media once; offline, its entry stays text."""
        if url in self._asked:
            return
        self._asked.add(url)

        def landed(path):
            self._files = {**self._files, url: QUrl.fromLocalFile(path).toString()}
            self.filesChanged.emit()

        def failed(e):
            self._asked.discard(url)
            log.info("%s: %s", url, e.message)

        self._client.changelogMedia(url, landed, failed)

    @Slot()
    def dismiss(self):
        if self._pending:
            self._pending = []
            self.pendingChanged.emit()

    def _opens(self):
        return bool((self._client.config().get("desktop") or {}).get("whats_new"))

    def _set_opens(self, on):
        if bool(on) != self._opens():
            self._client.setConfig("desktop.whats_new", "true" if on else "false")

    releases = Property(QVARIANT, _all, constant=True)
    pending = Property(QVARIANT, lambda self: self._pending, notify=pendingChanged)
    files = Property(QVARIANT, lambda self: self._files, notify=filesChanged)
    # config.toml's desktop.whats_new: the page opens by itself on the first start after an update.
    opensAfterUpdate = Property(bool, _opens, _set_opens, notify=opensChanged)
