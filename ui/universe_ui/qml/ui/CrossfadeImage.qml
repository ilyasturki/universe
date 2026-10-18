import QtQuick
import "../core"

// Two layers: reassigning one Image's source blanks it while the new one decodes.
Item {
    id: root

    property url source
    property int fillMode: Image.PreserveAspectCrop
    property int horizontalAlignment: Image.AlignHCenter
    property int verticalAlignment: Image.AlignVCenter
    // Taken along with each source by the hidden layer: the shown one never reloads at a new size.
    property size sourceSize
    property bool mipmap: false
    property int duration: Theme.durScene
    // The shown layer stays whole under the incoming one until that one is in, so nothing behind shows through midway.
    property bool opaque: false

    property bool showA: true
    // The incoming layer fades in on top; one already whole underneath (a swap undone midway) stays there as the top fades out.
    property bool aOnTop: false
    readonly property int status: (showA ? a : b).status
    // For the one swap that shows a first picture the cache already held: there at once, as a lone Image's would be.
    property bool instant: false

    onSourceChanged: loader.restart()
    onSourceSizeChanged: loader.restart()
    Component.onCompleted: load()

    // An unset dimension reads back as the picture's own once it is loaded.
    function sized(layer) {
        return (sourceSize.width <= 0 || layer.sourceSize.width === sourceSize.width) && (sourceSize.height <= 0 || layer.sourceSize.height === sourceSize.height);
    }

    function load() {
        if (source == "") {
            a.source = "";
            b.source = "";
            return;
        }
        var shown = showA ? a : b, incoming = showA ? b : a;
        if (shown.source == source && sized(shown))
            return;
        if (!sized(incoming)) {
            incoming.source = "";
            incoming.sourceSize = sourceSize;
        }
        // A cache hit is Ready within the assignment.
        instant = shown.source == "";
        incoming.source = source;
        // A source the hidden layer already holds fires no statusChanged.
        commit();
        instant = false;
    }

    // A source that fails is shown too, as nothing: `status` says so.
    function commit() {
        var incoming = showA ? b : a;
        if (source == "" || incoming.source != source || !sized(incoming) || (incoming.status !== Image.Ready && incoming.status !== Image.Error))
            return;
        if (incoming.opacity < 1)
            aOnTop = incoming === a;
        showA = !showA;
    }

    // One load once a source and its size have both settled, whichever binding ran first.
    Timer {
        id: loader
        interval: 0
        onTriggered: root.load()
    }

    component Layer: Image {
        readonly property bool ready: status === Image.Ready

        anchors.fill: parent
        fillMode: root.fillMode
        horizontalAlignment: root.horizontalAlignment
        verticalAlignment: root.verticalAlignment
        asynchronous: true
        retainWhileLoading: true
        smooth: true
        mipmap: root.mipmap
        onStatusChanged: if (status === Image.Ready || status === Image.Error)
            root.commit()

        Behavior on opacity {
            enabled: !root.instant
            Ease {
                duration: root.duration
            }
        }
    }

    Layer {
        id: a
        z: root.aOnTop ? 1 : 0
        opacity: root.showA || (root.opaque && b.opacity < 1) ? 1.0 : 0.0
    }

    Layer {
        id: b
        z: root.aOnTop ? 0 : 1
        opacity: !root.showA || (root.opaque && a.opacity < 1) ? 1.0 : 0.0
    }
}
