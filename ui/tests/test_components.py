from conftest import settle, wait_for


def loaded(api):
    form = api.screens.components
    form.load()
    settle(form)
    return form


def row_of(form, ident, key="component"):
    return next(i for i, r in enumerate(form.rows) if r.get("component") == ident and r["key"] == key)


def labels(items):
    return [i["label"] for i in items]


def needs_rpcs3(fake):
    return fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")


def test_components_list_by_kind_with_what_needs_doing_first(api, fake):
    needs_rpcs3(fake)
    form = loaded(api)
    assert [g["title"] for g in form.groups] == ["Recently updated", "Proton", "Wine", "Emulators", "Tools", "System", "No download"]
    emulators = next(g for g in form.groups if g["title"] == "Emulators")
    assert [form.rows[i]["component"] for i in emulators["rows"]] == ["rpcs3", "xemu", "dolphin", "eden"], "what needs doing, then by the games on it"
    assert emulators["meta"] == "3 of 4 installed"
    rows = {r["component"]: r for r in form.rows if r["key"] == "component"}
    assert rows["xemu"]["tag"] == "Update 0.8.136" and rows["xemu"]["accent"] is True and rows["xemu"]["display"] == "0.8.135 · Universe"
    assert rows["wine"]["tag"] == "11.18 available" and rows["wine"]["display"] == "11.8 · Nix" and rows["wine"]["size"]
    assert rows["rpcs3"]["tag"] == "Needed" and rows["rpcs3"]["display"] == "Not installed"
    assert rows["dolphin"]["display"] == "2606a · Nix" and rows["dolphin"]["tag"] == "", "no build to offer: nothing to do"
    assert rows["mame"]["display"] == "No download" and next(g for g in form.groups if g["title"] == "No download")["off"] is True
    assert rows["eden"]["icon"] == "assets/runners/eden.svg" and rows["umu-run"]["icon"] == "terminal"
    assert form.pending == 4, "the update and the three proposals: the sidebar's badge"


def test_the_options_put_the_useful_one_first_and_ask_before_what_costs(api, fake):
    form = loaded(api)
    assert labels(form.actions(row_of(form, "xemu"))) == ["Update to 0.8.136", "Uninstall · 61.0 MB", "Runner settings"]
    assert labels(form.actions(row_of(form, "wine")))[:2] == ["Install 11.18 and use it", "Install another version…"]
    assert labels(form.versionActions(row_of(form, "wine")))[0].startswith("11.17 · 58")
    ge = row_of(form, "ge-proton")
    assert labels(form.actions(ge)) == [
        "Use GE-Proton11-6 · Universe",
        "Install another version…",
        "Roll back GE-Proton11-7",
        "Remove GE-Proton11-6 · 1.1 GB",
        "Uninstall · 2.2 GB",
    ]
    assert form.actions(row_of(form, "umu-run"))[0]["label"] == "Install 1.4.4", "a tool: no build to switch to, the system's always wins"
    ask = form.confirm(row_of(form, "rpcs3"), "install")
    assert ask["message"] == "Install RPCS3 0.0.42-20069-3fa07db7?" and "to download" in ask["detail"] and ask["yes"] == "Install"
    assert form.confirm(ge, "rollback")["yes"] == "Roll back"
    assert form.confirm(ge, "use:GE-Proton11-6") is None, "a switch is undone as easily: nothing to ask"


def test_an_install_runs_as_a_job_and_a_proposed_newer_build_becomes_the_one_used(api, fake):
    form = loaded(api)
    assert form.act(row_of(form, "wine"), "install") is True
    assert form.job["label"] == "Installing Wine (staging) 11.18"
    assert form.rows[row_of(form, "wine")]["tag"] == "Installing…"
    job, ok, _text = wait_for(fake.jobFinished, 5000)
    assert ok is True and job == form.job["id"]
    settle(form)
    rows = {r["component"]: r for r in form.rows if r["key"] == "component"}
    assert rows["wine"]["display"] == "11.18 · Universe" and rows["wine"]["tag"] == "", "accepted from its proposal: Universe's build runs"
    assert form.job["ok"] is True and form.job["message"] == "Installed Wine (staging) 11.18"


def test_a_cancel_stops_the_job(api, fake):
    form = loaded(api)
    form.act(row_of(form, "rpcs3"), "install")
    assert labels(form.actions(row_of(form, "rpcs3"))) == ["Cancel"]
    assert form.act(row_of(form, "rpcs3"), "cancel") is True
    _job, ok, _text = wait_for(fake.jobFinished, 5000)
    assert ok is False and form.job["message"] == "Stopped installing RPCS3 0.0.42-20069-3fa07db7"


def test_a_rollback_removes_the_update_and_skips_it(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    assert form.act(row_of(form, "ge-proton"), "rollback") is True
    settle(form)
    settle(form)
    assert messages[-1] == "Rolled back GE-Proton"
    assert form.rows[row_of(form, "ge-proton")]["display"] == "GE-Proton11-6 · Universe"
    assert form.groups[0]["title"] == "Proton", "the update rolled back is no longer recent"


def test_an_uninstall_takes_every_build_the_one_in_use_too(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    xemu = row_of(form, "xemu")
    ask = form.confirm(xemu, "uninstall")
    assert (ask["message"], ask["yes"], ask["no"]) == ("Uninstall xemu?", "Uninstall", "Keep it")
    assert ask["detail"] == "Universe removes 0.8.135 (61.0 MB). Universe can install it again later.", "its one build, in use"
    ge = form.confirm(row_of(form, "ge-proton"), "uninstall")["detail"]
    assert ge == "Universe removes GE-Proton11-7 and GE-Proton11-6 (2.2 GB). 3 games on it won't start until it is installed again."
    assert form.act(xemu, "uninstall") is True
    settle(form)
    settle(form)
    assert messages[-1] == "Uninstalled xemu"
    assert form.rows[row_of(form, "xemu")]["display"] == "Not installed"
    assert not [a for a in form.actions(row_of(form, "xemu")) if a["action"] == "uninstall"], "nothing of Universe's left"
    assert not [a for a in form.actions(row_of(form, "eden")) if a["action"] == "uninstall"], "the system's build is not Universe's to take"


def test_a_launch_missing_its_runner_offers_the_install_then_launches(api, fake):
    game = needs_rpcs3(fake)
    form = loaded(api)
    proposed, ready = [], []
    form.installProposed.connect(lambda *args: proposed.append(args))
    form.readyToLaunch.connect(ready.append)
    fake.launchFailed.emit(game, f"{game}: RPCS3 not found (install it or set runners.rpcs3.exe)")
    assert proposed == [(game, "rpcs3", "RPCS3", "0.0.42-20069-3fa07db7")]
    assert form.question("rpcs3")["message"] == "Install RPCS3 0.0.42-20069-3fa07db7?"
    assert form.installFor(game, "rpcs3") is True
    wait_for(fake.jobFinished, 5000)
    assert ready == [game]
    fake.launchFailed.emit(game, f"{game}: the prefix is locked")
    assert len(proposed) == 1, "another failure proposes nothing"


def test_the_doctor_offers_the_install_that_fixes_a_check(api, fake):
    needs_rpcs3(fake)
    doctor = api.screens.modules
    doctor.loadDoctor()
    wait_for(doctor.doctorChanged, 3000)
    row = next(r for r in doctor.doctor if r["label"] == "RPCS3")
    assert row["value"] is False and row["component"] == "rpcs3"
    assert all(r["component"] == "" for r in doctor.doctor if r["label"] != "RPCS3")


def test_the_daily_update_runs_quietly_and_says_what_it_updated(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    form._auto_update()
    assert form.job is None, "no bar for the daily run"
    wait_for(fake.jobFinished, 5000)
    assert messages == ["Updated xemu 0.8.136"]
    fake.setConfig("components.auto_update", "false")
    form._auto_update()
    assert form.job is None and messages == ["Updated xemu 0.8.136"], "turned off: nothing runs"


def test_a_system_tool_installs_from_the_distribution_through_packagekit(api, fake):
    form = loaded(api)
    system = next(g for g in form.groups if g["title"] == "System")
    assert [form.rows[i]["component"] for i in system["rows"]] == ["gpu-screen-recorder", "gamescope"]
    row = form.rows[row_of(form, "gpu-screen-recorder")]
    assert row["tag"] == "Needed" and row["display"] == "Not installed" and "gpu-screen-recorder from your distribution" in row["detail"]
    assert labels(form.actions(row_of(form, "gpu-screen-recorder"))) == ["Install from the distribution"]
    assert form.actions(row_of(form, "gamescope")) == [], "installed by the distribution: nothing for Universe to do"
    ask = form.confirm(row_of(form, "gpu-screen-recorder"), "install")
    assert ask["detail"] == "gpu-screen-recorder from your distribution's packages. It asks for your password."
    assert form.act(row_of(form, "gpu-screen-recorder"), "install") is True
    wait_for(fake.jobFinished, 5000)
    settle(form)
    assert form.rows[row_of(form, "gpu-screen-recorder")]["display"] == "1.0 · system"


def test_setup_offers_the_runners_the_library_needs(api, fake):
    needs_rpcs3(fake)
    form = api.screens.onboarding
    form.load()
    settle(form)
    settle(api.screens.components)
    needed = [(i, r) for i, r in enumerate(form.rows) if r.get("via") == "component"]
    assert [(r["label"], r["action"], r["section"]) for _, r in needed] == [("RPCS3", "Install", "Runners your games need")]
    assert form.runImport(needed[0][0]) is True
    assert next(r for r in form.rows if r.get("via") == "component")["display"] == "Installing…"
    wait_for(fake.jobFinished, 5000)
    settle(api.screens.components)
    assert not [r for r in form.rows if r.get("via") == "component"], "installed: nothing left to offer"
