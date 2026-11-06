import QtQuick
import QtQuick.Window
import "core"
import "ui"

// Shown with no core: nothing here may read `api`, so the look's own buttons, glyphs and sounds stay out.
Window {
    id: window

    component Action: Rectangle {
        id: action

        property alias label: name.text

        signal picked

        width: name.width + Theme.dp(92)
        height: Theme.dp(78)
        radius: height / 2
        color: activeFocus ? Theme.text : Theme.surface
        border.width: Math.max(1, Theme.dp(2))
        border.color: Theme.surfaceBorder

        Keys.onPressed: function (event) {
            if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                event.accepted = true;
                action.picked();
            }
        }

        Loader {
            anchors.fill: parent
            active: action.activeFocus
            sourceComponent: FocusRing {
                cornerRadius: action.radius
                gapWidth: Theme.dp(Theme.ringGap)
            }
        }

        Text {
            id: name
            anchors.centerIn: parent
            color: action.activeFocus ? Theme.onLight : Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(27)
        }

        MouseArea {
            anchors.fill: parent
            onClicked: action.picked()
        }
    }

    width: 1920
    height: 1080
    visibility: failure.fullscreen ? Window.FullScreen : Window.Windowed
    color: Theme.ground
    title: "Universe"

    Binding {
        target: Theme
        property: "vscale"
        value: window.height / 1080
    }

    Column {
        anchors.centerIn: parent
        width: Math.min(parent.width - Theme.dp(Theme.edgeMargin) * 2, Theme.dp(1200))
        spacing: Theme.dp(32)

        Keys.onPressed: function (event) {
            if ((event.key === Qt.Key_Escape || event.key === Qt.Key_Backspace) && !event.isAutoRepeat) {
                event.accepted = true;
                failure.leave();
            }
        }

        Text {
            width: parent.width
            text: failure.retried ? "Universe still can't start" : "Universe can't start"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(56)
            wrapMode: Text.Wrap
        }

        Text {
            objectName: "reason"
            width: parent.width
            text: failure.reason
            textFormat: Text.PlainText
            color: Theme.textSecondary
            font.family: "monospace"
            font.pixelSize: Theme.dp(26)
            wrapMode: Text.Wrap
            maximumLineCount: 10
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            text: "Fix it from a terminal or another computer, then try again. universe doctor shows what's wrong."
            color: Theme.textHint
            font.family: Theme.sans
            font.pixelSize: Theme.dp(28)
            wrapMode: Text.Wrap
        }

        Row {
            spacing: Theme.dp(36)
            topPadding: Theme.dp(24)

            Action {
                id: retry
                objectName: "retry"
                label: "Try again"
                focus: true
                KeyNavigation.right: leave
                KeyNavigation.down: leave
                onPicked: failure.retry()
            }

            Action {
                id: leave
                objectName: "leave"
                label: failure.session ? "Log out" : "Quit Universe"
                KeyNavigation.left: retry
                KeyNavigation.up: retry
                onPicked: failure.leave()
            }
        }
    }
}
