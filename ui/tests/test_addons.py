import pytest
from looks import invoke, read
from PySide6.QtCore import QObject, Qt
from uitest import until

KINDS = {"module": "modules", "source": "sources"}


def form_of(api, kind):
    return api.screens.modules if kind == "module" else api.screens.sourceList


def more_row(api, kind):
    """The list's last row, once the index is in: "Get more…"."""
    form = form_of(api, kind)
    until(lambda: form.rows and form.rows[-1]["key"] == "more" and api.screens.addons.listing is not None, "the list ends on Get more…")
    return len(form.rows) - 1, form.rows[-1]


def opened(look, page, kind):
    """The "Get more…" row picked: the add-ons of `kind`, in the look's menu."""
    index, row = more_row(look.api, kind)
    invoke(page, "activate", index, {"key": "more", "addons": row["addons"]})
    menu = page.findChild(QObject, "settingsMenu") if not look.stacked else look.menu()
    until(lambda: menu.property("open") is True, "the add-ons list opens")
    return menu


def dialog_of(look, page):
    return page.findChild(QObject, "dialog") if not look.stacked else look.dialog()


def answered(look, page, ask):
    """The confirmation `ask` is shown, then accepted."""
    dialog = dialog_of(look, page)
    until(lambda: dialog.property("open") is True, "a confirmation first")
    assert read(dialog, "message" if look.stacked else "title") == ask["message"]
    look.press(Qt.Key.Key_Return)
    until(lambda: dialog.property("open") is False)


def shown(menu):
    items = read(menu, "choices" if menu.objectName() == "picker" else "items")
    return len(items)


@pytest.mark.slow
@pytest.mark.parametrize("kind", ["module", "source"])
def test_get_more_lists_the_index_installs_after_a_confirmation_and_the_list_takes_it(look, api, fake, kind):
    addons = api.screens.addons
    page = look.settings(KINDS[kind])
    menu = opened(look, page, kind)
    listed = [r for r in addons.listing["extensions"] if r["kind"] == kind]
    assert shown(menu) == len(listed), "a row per add-on of the kind"
    at, first = next((i, r["id"]) for i, r in enumerate(listed) if not r["incompatible"])
    ask = addons.confirm(first, "install")
    assert (ask["kind"], ask["origin"], ask["runsAsYou"]) == (kind, "registry", True)
    look.press(Qt.Key.Key_Down, at)
    look.press(Qt.Key.Key_Return)
    answered(look, page, ask)
    form = form_of(api, kind)
    until(lambda: any(r["module"] == first for r in form.rows), "the installed add-on joins the list")
    until(lambda: next(r for r in addons.listing["extensions"] if r["id"] == first)["installed"], "and the add-ons list says it is installed")


@pytest.mark.slow
def test_an_unlisted_add_on_is_tagged_and_removes_from_the_list(look, api, fake):
    addons = api.screens.addons
    fake.core.extension_install("/home/me/lights", True)
    page = look.settings("modules")
    form = form_of(api, "module")
    until(lambda: any(r["module"] == "lights" and r["tag"] == "Unlisted" for r in form.rows), "an unlisted add-on says so")
    menu = opened(look, page, "module")
    lights = next(i for i, r in enumerate(addons.items("module")) if r["action"] == "lights")
    assert addons.actions("lights") == [{"icon": "trash", "label": "Remove", "action": "remove", "danger": True}], "only removal, from the UI"
    look.press(Qt.Key.Key_Down, lights)
    look.press(Qt.Key.Key_Return)
    answered(look, page, addons.confirm("lights", "remove"))
    until(lambda: not any(r["module"] == "lights" for r in form.rows), "removed, it leaves the list")
    assert menu.property("open") is False


def test_an_add_on_for_another_universe_is_listed_and_offers_nothing(api, fake):
    addons = api.screens.addons
    addons.load()
    until(lambda: addons.listing is not None)
    assert any(i["action"] == "arcade-vault" for i in addons.items("source"))
    assert next(r for r in addons.listing["extensions"] if r["id"] == "arcade-vault")["incompatible"]
    assert addons.actions("arcade-vault") == [], "it cannot be installed"
