import QtQuick
import QtQuick.Window
import "core"
import "ui"

Window {
    id: window

    width: 1920
    height: 1080
    visibility: api.fullscreen ? Window.FullScreen : Window.Windowed
    color: api.theme.ground
    title: "Universe"

    Loader {
        objectName: "look"
        anchors.fill: parent
        source: api.theme.entry
        focus: true

        onLoaded: {
            var notice = api.theme.takeNotice();
            if (notice)
                Notices.fail(notice);
        }
        // Out of the status change: the fall back changes this very source.
        onStatusChanged: {
            if (status === Loader.Error)
                Qt.callLater(api.theme.failed);
        }
    }

    // Out here, an action's result outlives the page that started it.
    Connections {
        target: api.screens.gameData
        function onFinished(key, ok, message) {
            if (ok)
                Notices.show(message);
            else
                Notices.fail(message);
        }
    }

    Connections {
        target: api.screens.storage
        function onFinished(key, ok, message) {
            if (ok)
                Notices.show(message);
            else
                Notices.fail(message);
        }
    }

    Connections {
        target: api.screens.add
        function onMessage(text) {
            Notices.show(text);
        }
        function onFailed(text) {
            Notices.fail(text);
        }
    }

    // Outside the Loader, so a theme switch never replays it.
    Boot {
        anchors.fill: parent
    }
}
