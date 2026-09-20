import QtQuick
import "../core"

// Two layers: reassigning one Image's source blanks it while the new one decodes.
Item {
    id: root

    property url source
    property int fillMode: Image.PreserveAspectCrop
    property int horizontalAlignment: Image.AlignHCenter
    property int verticalAlignment: Image.AlignVCenter
    property size sourceSize
    property bool mipmap: false
    property int duration: Theme.durScene

    property bool showA: true

    onSourceChanged: {
        if (source == "") {
            a.source = "";
            b.source = "";
            return;
        }
        (showA ? b : a).source = source;
        // A source the hidden layer already holds fires no statusChanged.
        commit();
    }

    function commit() {
        var incoming = showA ? b : a;
        if (source != "" && incoming.source == source && incoming.status === Image.Ready)
            showA = !showA;
    }

    component Layer: Image {
        anchors.fill: parent
        fillMode: root.fillMode
        horizontalAlignment: root.horizontalAlignment
        verticalAlignment: root.verticalAlignment
        sourceSize: root.sourceSize
        asynchronous: true
        smooth: true
        mipmap: root.mipmap
        onStatusChanged: if (status === Image.Ready)
            root.commit()

        Behavior on opacity {
            Ease {
                duration: root.duration
            }
        }
    }

    Layer {
        id: a
        opacity: root.showA ? 1.0 : 0.0
    }

    Layer {
        id: b
        opacity: root.showA ? 0.0 : 1.0
    }
}
