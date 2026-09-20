import QtQuick
import "../core"

// A source's sign-in link as a QR code, the URL and the flow's status; shown while `api.screens.login` runs for `source`.
Item {
    id: card

    property string source: ""
    readonly property var login: api.screens.login

    height: Math.max(Theme.dp(74), head.height) + body.height + Theme.dp(8) * 2 + 2
    visible: source !== "" && login.source === source && (login.url !== "" || login.status !== "")

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(24)
        color: Qt.rgba(1, 1, 1, 0.04)
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.10)
    }

    Text {
        id: head
        x: Theme.dp(8) + 1 + Theme.dp(18)
        y: Theme.dp(8) + 1
        height: Theme.dp(74)
        verticalAlignment: Text.AlignVCenter
        text: "Sign-in link"
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(27)
    }

    Row {
        id: body

        x: Theme.dp(8) + 1 + Theme.dp(16)
        y: head.y + head.height
        width: parent.width - x * 2
        height: Math.max(qr.height, text.height) + Theme.dp(36)
        spacing: Theme.dp(28)

        QrCode {
            id: qr
            y: Theme.dp(18)
            width: Theme.dp(300)
            height: width
            matrix: card.login.matrix
            visible: card.login.url !== ""
        }

        Column {
            id: text
            y: Theme.dp(18)
            width: parent.width - (qr.visible ? qr.width + parent.spacing : 0)
            spacing: Theme.dp(14)

            Text {
                width: parent.width
                text: "Scan to sign in on your phone"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(22)
            }

            Text {
                width: parent.width
                text: card.login.url
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(18)
                lineHeight: 1.3
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 5
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: card.login.status
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(20)
                wrapMode: Text.WordWrap
            }
        }
    }
}
