import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

// Rounds off whatever is put inside: clip:true ignores radius, so the corners come from a mask.
Item {
    id: root

    property real radius: Theme.dp(Theme.radiusCover)
    default property alias content: inner.data

    // Sized by binding, not anchors: these are parented after they are built.
    readonly property Item inner: Item {
        id: inner
        parent: root
        width: root.width
        height: root.height
        layer.enabled: !Theme.software && root.radius > 0
        layer.smooth: true
        layer.effect: Theme.software ? null : root.maskEffect
    }

    readonly property Component maskEffect: Component {
        OpacityMask { maskSource: root.mask }
    }

    readonly property Rectangle mask: Rectangle {
        parent: root
        width: root.width
        height: root.height
        radius: root.radius
        color: "white"
        antialiasing: true
        visible: false
    }
}
