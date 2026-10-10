import QtQuick
import "../core"

// A PillButton's round sibling: one glyph, no label.
Rectangle {
    id: root

    property string icon: "more"
    property bool focused: false
    property bool dimmed: false

    signal picked

    width: Theme.dp(78)
    height: Theme.dp(78)
    radius: width / 2
    color: Theme.surface
    border.width: Math.max(1, Theme.dp(2))
    border.color: Theme.surfaceBorder
    opacity: root.dimmed ? 0.5 : 1.0
    scale: root.focused ? 1.04 : 1.0

    Behavior on opacity {
        Ease {
            duration: Theme.durQuick
        }
    }
    Behavior on scale {
        Ease {
            easing.type: Easing.OutQuint
        }
    }

    Loader {
        anchors.fill: parent
        active: root.focused
        sourceComponent: FocusRing {
            cornerRadius: root.radius
            gapWidth: Theme.dp(Theme.ringGap)
        }
    }

    MenuGlyph {
        anchors.centerIn: parent
        width: Theme.dp(28)
        height: Theme.dp(28)
        kind: root.icon
        tint: "#f2f3f5"
    }

    Pointer {
        current: root.focused
        direct: true
        radius: root.radius
        onPicked: root.picked()
    }
}
