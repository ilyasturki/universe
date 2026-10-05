.pragma library

// Wi-Fi and Bluetooth as the console words them: the rows Settings › Network and Accessories show, and what A on one does.
// `wifi` is api.screens.network, `bt` api.screens.bluetooth, `shell` the theme, `play` Sound.play.

var SECURITY = { open: "Open", owe: "Open", wep: "WEP", psk: "WPA2/WPA3", sae: "WPA3", enterprise: "WPA2 Enterprise" };

function networkDisplay(wifi, n) {
    if (n.active)
        return "Connected";
    if (wifi.connecting === n.ssid)
        return "Connecting…";
    return n.saved ? "Saved" : SECURITY[n.security] || "";
}

function statusText(wifi) {
    if (wifi.link === "wired")
        return "Connected by cable";
    if (wifi.link === "wifi")
        return "Connected to " + wifi.ssid;
    return "Not connected";
}

function connectivityText(c) {
    return c === "full" ? "Internet connection: OK." : c === "limited" ? "Connected, but the internet does not answer." : c === "portal" ? "The network asks you to sign in from a browser." : c === "none" ? "No internet connection." : "The connection could not be tested.";
}

function networkContent(wifi) {
    var out = [
        {
            key: "wifi",
            label: "Connect to the Internet",
            type: "bool",
            value: wifi.enabled,
            action: "wifi",
            display: "",
            detail: "Wi-Fi on or off. A cable stays connected either way."
        },
        {
            heading: true,
            part: true,
            label: "Set Up Internet Connection",
            display: ""
        }
    ];
    var nets = wifi.networks;
    nets.forEach(function (n) {
        out.push(Object.assign({}, n, {
            key: "network",
            label: n.ssid,
            type: "action",
            action: "network",
            icon: "wifi" + Math.max(1, n.bars),
            display: networkDisplay(wifi, n),
            detail: n.joinable ? "" : "Universe does not set up " + (SECURITY[n.security] || n.security) + " networks.",
            dim: !n.joinable
        }));
    });
    if (nets.length === 0)
        out.push({
            key: "",
            label: wifi.enabled ? "Searching for networks…" : "Wi-Fi is off",
            type: "static",
            display: "",
            detail: ""
        });
    out.push({
        heading: true,
        part: true,
        label: "Connection Status",
        display: ""
    }, {
        key: "status",
        label: "Network",
        type: "static",
        display: statusText(wifi),
        detail: ""
    }, {
        key: "check",
        label: "Test Internet Connection",
        type: "action",
        action: "check",
        display: wifi.checking ? "Testing…" : "",
        detail: ""
    });
    return out;
}

function askPassword(shell, wifi, ssid) {
    shell.prompt({
        title: "Enter the password for " + ssid,
        value: "",
        secret: true,
        max: 63
    }, function (v) {
        if (v !== null && v !== "")
            wifi.join(ssid, v);
    });
}

function joinNetwork(shell, wifi, row, play) {
    if (!row.active && !row.joinable) {
        play("edge");
        shell.showToast(row.ssid + " uses " + (SECURITY[row.security] || row.security) + " security, which Universe does not set up.");
        return;
    }
    if (row.active) {
        play("ok");
        shell.dialogAsk({
            message: "Forget " + row.ssid + "?",
            detail: "This machine will stop joining it. Joining it again takes its password.",
            buttons: ["Cancel", "Forget"],
            danger: 1
        }, function (i) {
            if (i === 1)
                wifi.forget(row.ssid);
        });
        return;
    }
    if (wifi.needsPassword(row.ssid)) {
        play("ok");
        askPassword(shell, wifi, row.ssid);
        return;
    }
    play(wifi.join(row.ssid, "") ? "ok" : "edge");
}

var KIND_ICON = { pad: "gamepad", audio: "headphones", keyboard: "keyboard", mouse: "cursor", phone: "screen", other: "bluetooth" };

function deviceRow(bt, d) {
    var display = bt.pairing === d.address ? "Pairing…" : d.connected ? (d.battery !== null && d.battery !== undefined ? "Connected · " + d.battery + "%" : "Connected") : d.paired ? "Not Connected" : "";
    return Object.assign({}, d, {
        key: "device",
        label: d.name,
        type: "action",
        action: "device",
        icon: KIND_ICON[d.kind] || "bluetooth",
        display: display,
        detail: ""
    });
}

function deviceContent(bt, pads) {
    var keep = function (d) {
        return !pads || d.kind === "pad";
    };
    var paired = bt.paired.filter(keep), found = bt.found.filter(keep);
    var out = [
        {
            key: "power",
            label: "Bluetooth",
            type: "bool",
            value: bt.powered,
            action: "bt-power",
            display: "",
            detail: ""
        },
        {
            heading: true,
            label: "Registered Accessories",
            display: ""
        }
    ];
    paired.forEach(function (d) {
        out.push(deviceRow(bt, d));
    });
    if (paired.length === 0)
        out.push({
            key: "",
            label: "None yet",
            type: "static",
            display: "",
            detail: ""
        });
    out.push({
        heading: true,
        label: "Found Accessories",
        display: bt.discovering ? "Searching…" : ""
    });
    found.forEach(function (d) {
        out.push(deviceRow(bt, d));
    });
    if (found.length === 0)
        out.push({
            key: "",
            label: !bt.powered ? "Bluetooth is off" : pads ? "Searching for controllers…" : "Searching for accessories…",
            type: "static",
            wraps: bt.powered,
            display: "",
            detail: !bt.powered ? "" : pads ? "Hold the controller's pairing button until its light blinks: PS and Create together on a DualSense. No controller connected yet? Connect one with a USB cable first." : "Put the accessory in pairing mode."
        });
    return out;
}

function device(shell, bt, row, play) {
    if (!row.paired) {
        play(bt.pairing === "" && bt.pair(row.address) ? "ok" : "edge");
        return;
    }
    play("ok");
    var link = row.connected ? "Disconnect" : "Connect";
    shell.dialogAsk({
        message: row.name,
        detail: row.connected ? "Connected." : "Registered, not connected.",
        buttons: ["Cancel", "Forget", link],
        danger: 1
    }, function (i) {
        if (i === 1)
            bt.forget(row.address);
        else if (i === 2)
            row.connected ? bt.disconnectDevice(row.address) : bt.connectDevice(row.address);
    });
}
