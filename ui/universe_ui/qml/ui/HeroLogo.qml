import QtQuick
import "../core"

// Two layers: reassigning one Image's source blanks it while the new one decodes.
Item {
    id: root

    property var game: null
    property real logoWidth: Theme.dp(480)
    property real logoHeight: Theme.dp(150)
    property real titleWidth: width

    readonly property url logoSource: game && game.assets.logo ? game.assets.logo : ""
    // Null game shows nothing; a game with no logo shows its title.
    readonly property bool titleMode: logoSource == "" && game !== null

    implicitWidth: logoWidth
    implicitHeight: titleMode ? title.height : logoHeight

    property bool showA: true

    onLogoSourceChanged: {
        if (logoSource == "") {
            a.source = "";
            b.source = "";
            return;
        }
        var incoming = showA ? b : a;
        incoming.source = logoSource;
        // A source the layer already holds fires no statusChanged; commit it here.
        if (incoming.status === Image.Ready)
            _commit(incoming);
    }

    function _commit(layer) {
        if (layer.source != logoSource)
            return;
        if ((showA && layer === b) || (!showA && layer === a))
            showA = !showA;
    }

    Image {
        id: a
        width: root.logoWidth
        height: root.logoHeight
        fillMode: Image.PreserveAspectFit
        horizontalAlignment: Image.AlignLeft
        verticalAlignment: Image.AlignBottom
        asynchronous: true
        sourceSize.width: root.logoWidth
        sourceSize.height: root.logoHeight
        opacity: root.showA && !root.titleMode ? 1.0 : 0.0
        onStatusChanged: if (status === Image.Ready) root._commit(a)

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Image {
        id: b
        width: root.logoWidth
        height: root.logoHeight
        fillMode: Image.PreserveAspectFit
        horizontalAlignment: Image.AlignLeft
        verticalAlignment: Image.AlignBottom
        asynchronous: true
        sourceSize.width: root.logoWidth
        sourceSize.height: root.logoHeight
        opacity: !root.showA && !root.titleMode ? 1.0 : 0.0
        onStatusChanged: if (status === Image.Ready) root._commit(b)

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Text {
        id: title

        anchors.bottom: parent.bottom
        width: root.titleWidth
        text: root.game ? root.game.title : ""
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(58)
        elide: Text.ElideRight
        opacity: root.titleMode ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }
}
