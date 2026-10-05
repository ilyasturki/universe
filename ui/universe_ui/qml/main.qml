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

    // Outside the Loader, so a theme switch never replays it.
    Boot {
        anchors.fill: parent
    }
}
