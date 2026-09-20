import QtQuick
import "../core"
import "../../ui" as Base

// A source's sign-in link as a QR code, the URL and the flow's status; shown while `api.screens.login` runs for `source`.
Item {
    id: card

    property string source: ""
    readonly property var login: api.screens.login

    height: Theme.dp(330)
    visible: source !== "" && login.source === source && (login.url !== "" || login.status !== "")

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(6)
        color: Theme.card
        border.width: 1
        border.color: Theme.hairline
    }

    Loader {
        id: qr
        x: Theme.dp(24)
        y: Theme.dp(24)
        width: Theme.dp(282)
        height: width
        active: card.login.url !== ""
        sourceComponent: Base.QrCode {
            matrix: card.login.matrix
        }
    }

    Column {
        x: qr.active ? qr.x + qr.width + Theme.dp(30) : Theme.dp(30)
        y: Theme.dp(30)
        width: parent.width - x - Theme.dp(30)
        spacing: Theme.dp(14)

        Label {
            width: parent.width
            text: "Scan to sign in on your phone"
        }

        Label {
            width: parent.width
            text: card.login.url
            color: Theme.accent
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 4
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }

        Label {
            width: parent.width
            text: card.login.status
            color: Theme.textSecondary
            wrapMode: Text.WordWrap
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }
}
