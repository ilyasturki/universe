import QtQuick
import "../core"

MenuGlyph {
    id: mark

    objectName: "journalMark"
    width: Theme.dp(24)
    height: width
    visible: api.screens.pendingJournals.count > 0
    kind: "book"
    tint: Theme.textSecondary

    SequentialAnimation on opacity {
        running: mark.visible && !Theme.covered
        loops: Animation.Infinite
        NumberAnimation {
            to: 0.35
            duration: 900
            easing.type: Easing.InOutQuad
        }
        NumberAnimation {
            to: 1.0
            duration: 900
            easing.type: Easing.InOutQuad
        }
    }
}
