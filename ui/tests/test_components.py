from conftest import record, settle, until


def loaded(api):
    form = api.screens.components
    form.load()
    settle(form)
    return form


def component(form, ident):
    return next(c for c in form.listing()["components"] if c["id"] == ident)


def row_of(form, ident):
    return form.row(component(form, ident))


def acts(items):
    return [i["action"] for i in items]


def needs_rpcs3(fake):
    return fake.addGame("rpcs3", "/games/Demons Souls/PS3_GAME/USRDIR/EBOOT.BIN", "Demons Souls")


def test_a_component_row_says_what_to_do_about_it(api, fake):
    needs_rpcs3(fake)
    form = loaded(api)
    xemu, wine, rpcs3, dolphin = (row_of(form, ident) for ident in ("xemu", "wine", "rpcs3", "dolphin"))
    assert (xemu["key"], xemu["component"], xemu["action"], xemu["accent"]) == ("component", "xemu", "Options", True), "an update waits"
    assert wine["accent"] is True and wine["size"], "a newer build than the system's, with its download's size"
    assert rpcs3["accent"] is True and rpcs3["size"], "a game waits on it"
    assert dolphin["accent"] is False and dolphin["size"] == "", "no build to offer: nothing to do"
    assert row_of(form, "eden")["icon"] == "assets/runners/eden.svg" and row_of(form, "umu-run")["icon"] == "terminal"
    assert form.pending == 4, "the update and the three proposals: Runners' badge"


def test_the_options_put_the_useful_one_first_and_ask_before_what_costs(api, fake):
    form = loaded(api)
    assert acts(form.actions("xemu")) == ["update", "uninstall"]
    assert acts(form.actions("wine"))[:2] == ["install", "versions"]
    assert acts(form.versionActions("wine")) == ["install:11.17"]
    ge = "ge-proton"
    assert acts(form.actions(ge)) == ["use:GE-Proton11-6", "versions", "rollback", "remove:GE-Proton11-6", "uninstall"]
    assert acts(form.actions("umu-run")) == ["install"], "a tool: no build to switch to, the system's always wins"
    ask = form.confirm("rpcs3", "install")
    assert ask["message"] == "Install RPCS3 0.0.42-20069-3fa07db7?" and "to download" in ask["detail"] and ask["yes"] == "Install"
    assert form.confirm(ge, "rollback")["yes"] == "Roll back"
    assert form.confirm(ge, "use:GE-Proton11-6") is None, "a switch is undone as easily: nothing to ask"
    assert form.act(ge, "use:GE-Proton11-6") is True
    settle(form)
    assert "use:latest" in acts(form.actions(ge)), "held on one build: the newest is a pick away"
    fake.componentUse("xemu", "0.8.135")
    form.load()
    settle(form)
    assert "use:latest" in acts(form.actions("xemu")), "an emulator held on one build too"


def test_an_install_runs_as_a_job_and_a_proposed_newer_build_becomes_the_one_used(api, fake):
    form = loaded(api)
    finished = record(fake.jobFinished)
    assert form.act("wine", "install") is True
    assert form.job["label"] == "Installing Wine (staging) 11.18"
    assert row_of(form, "wine")["action"] == "Cancel"
    job, ok, _text = until(lambda: finished)[0]
    assert ok is True and job == form.job["id"]
    settle(form)
    wine = component(form, "wine")
    assert wine["in_use"]["version"] == "11.18" and not row_of(form, "wine")["accent"], "accepted from its proposal: Universe's build runs"
    assert form.job["ok"] is True and form.job["message"] == "Installed Wine (staging) 11.18"


def test_a_cancel_stops_the_job(api, fake):
    form = loaded(api)
    finished = record(fake.jobFinished)
    form.act("rpcs3", "install")
    assert acts(form.actions("rpcs3")) == ["cancel"]
    assert form.act("rpcs3", "cancel") is True
    _job, ok, _text = until(lambda: finished)[0]
    assert ok is False and form.job["message"] == "Stopped installing RPCS3 0.0.42-20069-3fa07db7"


def test_a_rollback_removes_the_update_and_skips_it(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    assert form.act("ge-proton", "rollback") is True
    settle(form)
    settle(form)
    assert messages[-1] == "Rolled back GE-Proton"
    assert component(form, "ge-proton")["in_use"]["version"] == "GE-Proton11-6"
    assert component(form, "ge-proton")["recent"] is None, "the update rolled back is no longer recent"


def test_an_uninstall_takes_every_build_the_one_in_use_too(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    xemu = "xemu"
    ask = form.confirm(xemu, "uninstall")
    assert (ask["message"], ask["yes"], ask["no"]) == ("Uninstall xemu?", "Uninstall", "Keep it")
    assert ask["detail"] == "Universe removes 0.8.135 (61.0 MB). Universe can install it again later.", "its one build, in use"
    ge = form.confirm("ge-proton", "uninstall")["detail"]
    assert ge == "Universe removes GE-Proton11-7 and GE-Proton11-6 (2.2 GB). 3 games on it won't start until it is installed again."
    assert form.act(xemu, "uninstall") is True
    settle(form)
    settle(form)
    assert messages[-1] == "Uninstalled xemu"
    assert component(form, "xemu")["in_use"] is None
    assert not [a for a in form.actions("xemu") if a["action"] == "uninstall"], "nothing of Universe's left"
    assert not [a for a in form.actions("eden") if a["action"] == "uninstall"], "the system's build is not Universe's to take"


def test_hiding_the_bar_leaves_the_job_its_row_and_its_toast(api, fake):
    form = loaded(api)
    messages, progress = [], []
    form.message.connect(messages.append)
    form.listingChanged.connect(lambda: progress.append(row_of(form, "wine")["progress"]))
    finished = record(fake.jobFinished)
    form.act("wine", "install")
    form.hideJob()
    assert form.job is None, "the bar goes"
    assert row_of(form, "wine")["action"] == "Cancel" and acts(form.actions("wine")) == ["cancel"]
    until(lambda: finished)
    assert messages[-1] == "Installed Wine (staging) 11.18" and form.job is None, "the end still toasts, the bar stays away"
    assert [p for p in progress if p] == [0.25, 0.5, 0.75, 1.0], "the row's own progress moves meanwhile"
    settle(form)
    form.act("rpcs3", "install")
    assert form.job["label"] == "Installing RPCS3 0.0.42-20069-3fa07db7", "the next job has its bar"
    until(lambda: len(finished) == 2)
    assert form.job["ok"] is True
    form.hideJob()
    assert form.job is None, "a finished bar goes the same way"


def test_a_launch_missing_its_runner_offers_the_install_then_launches(api, fake):
    game = needs_rpcs3(fake)
    form = loaded(api)
    proposed, ready = [], []
    form.installProposed.connect(lambda *args: proposed.append(args))
    form.readyToLaunch.connect(ready.append)
    fake.launchFailed.emit(game, f"{game}: RPCS3 not found (install it or set runners.rpcs3.exe)")
    assert proposed == [(game, "rpcs3", "RPCS3", "0.0.42-20069-3fa07db7")]
    assert form.question("rpcs3")["message"] == "Install RPCS3 0.0.42-20069-3fa07db7?"
    finished = record(fake.jobFinished)
    assert form.installFor(game, "rpcs3") is True
    until(lambda: finished)
    assert ready == [game]
    fake.launchFailed.emit(game, f"{game}: the prefix is locked")
    assert len(proposed) == 1, "another failure proposes nothing"


def test_the_doctor_offers_the_install_that_fixes_a_check(api, fake):
    needs_rpcs3(fake)
    doctor = api.screens.modules
    changed = record(doctor.doctorChanged)
    doctor.loadDoctor()
    until(lambda: changed)
    row = next(r for r in doctor.doctor if r["label"] == "RPCS3")
    assert row["value"] is False and row["component"] == "rpcs3"
    assert all(r["component"] == "" for r in doctor.doctor if r["label"] != "RPCS3")


def test_the_daily_update_runs_quietly_and_says_what_it_updated(api, fake):
    form = loaded(api)
    messages = []
    form.message.connect(messages.append)
    finished = record(fake.jobFinished)
    form._auto_update()
    assert form.job is None, "no bar for the daily run"
    until(lambda: finished)
    assert messages == ["Updated xemu 0.8.136"]
    fake.setConfig("components.auto_update", "false")
    form._auto_update()
    assert form.job is None and messages == ["Updated xemu 0.8.136"], "turned off: nothing runs"


def test_a_system_tool_installs_from_the_distribution_through_packagekit(api, fake):
    form = loaded(api)
    row = row_of(form, "gpu-screen-recorder")
    assert row["accent"] is True and "gpu-screen-recorder from your distribution" in row["detail"]
    assert acts(form.actions("gpu-screen-recorder")) == ["install"]
    assert form.actions("gamescope") == [], "installed by the distribution: nothing for Universe to do"
    ask = form.confirm("gpu-screen-recorder", "install")
    assert ask["detail"] == "gpu-screen-recorder from your distribution's packages. It asks for your password."
    finished = record(fake.jobFinished)
    assert form.act("gpu-screen-recorder", "install") is True
    until(lambda: finished)
    settle(form)
    assert component(form, "gpu-screen-recorder")["in_use"]["version"] == "1.0"


def test_setup_offers_the_runners_the_library_needs(api, fake):
    needs_rpcs3(fake)
    form = api.screens.onboarding
    form.load()
    settle(form)
    settle(api.screens.components)
    needed = [(i, r) for i, r in enumerate(form.rows) if r.get("via") == "component"]
    assert [(r["label"], r["action"], r["section"]) for _, r in needed] == [("RPCS3", "Install", "Runners your games need")]
    finished = record(fake.jobFinished)
    assert form.runImport(needed[0][0]) is True
    assert next(r for r in form.rows if r.get("via") == "component")["display"] == "Installing…"
    until(lambda: finished)
    settle(api.screens.components)
    assert not [r for r in form.rows if r.get("via") == "component"], "installed: nothing left to offer"


def test_a_notice_is_put_to_the_user_before_every_install_of_its_component(api, fake):
    fake._core._component("eden")["builds"] = []
    notice = fake._core._component("eden")["notice"]
    form = loaded(api)
    assert form.confirm("eden", "install")["notice"] == notice, "Eden's page"
    assert form.question("eden")["notice"] == notice, "the launch offer and the doctor's install"
    assert form.confirm("rpcs3", "install")["notice"] == ""
    proposed = []
    form.installProposed.connect(lambda *args: proposed.append(args))
    setup = api.screens.onboarding
    setup.load()
    settle(setup)
    settle(form)
    index = next(i for i, r in enumerate(setup.rows) if r.get("component") == "eden")
    assert setup.rows[index]["detail"] == notice
    assert setup.runImport(index) is True
    assert proposed == [("", "eden", "Eden", "0.2.1")] and form.job is None, "setup asks first, nothing installs yet"
    finished = record(fake.jobFinished)
    assert form.installFor("", "eden") is True
    _job, ok, _text = until(lambda: finished)[0]
    assert ok is True


def test_the_core_refuses_an_install_whose_notice_was_not_accepted(api, fake):
    finished = record(fake.jobFinished)
    fake.componentInstall("eden", "")
    _job, ok, _text = until(lambda: finished)[0]
    assert ok is False
    fake.componentInstall("eden", "", True)
    until(lambda: len(finished) == 2)
    _job, ok, _text = finished[1]
    assert ok is True
