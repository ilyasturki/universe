import QtQuick
import "../core"

// The home's backdrop: the focused tile's world at full bleed, cross-faded once the incoming picture is decoded.
// `target`: { source, cropped } for a game's art, { scene: "welcome" | "system" } for the console's own.
Item {
    id: root

    property var target: ({
            scene: "system"
        })
    property int duration: Theme.durBackdrop
    // Past the hero, the world goes dark under the hub's strips.
    property real dim: 0
    property bool shaded: true
    // Held still while a game covers it.
    readonly property bool live: visible && !Theme.covered

    property bool showA: true

    function keyOf(t) {
        return t ? (t.scene || "") + "|" + String(t.source || "") : "";
    }

    onTargetChanged: {
        var incoming = showA ? b : a;
        var shown = showA ? a : b;
        if (keyOf(shown.spec) === keyOf(target))
            return;
        incoming.spec = target;
        commit();
    }

    function commit() {
        var incoming = showA ? b : a;
        if (keyOf(incoming.spec) === keyOf(target) && incoming.ready)
            showA = !showA;
    }

    component Layer: Item {
        id: layer

        property var spec: ({
                scene: ""
            })
        readonly property string scene: spec && spec.scene ? spec.scene : ""
        readonly property url source: spec && !spec.scene && spec.source ? spec.source : ""
        readonly property bool ready: scene !== "" || String(source) === "" || picture.status === Image.Ready || picture.status === Image.Error

        anchors.fill: parent
        visible: opacity > 0.001
        onReadyChanged: if (ready)
            root.commit()

        Behavior on opacity {
            NumberAnimation {
                duration: root.duration
                easing.type: Easing.InOutQuad
            }
        }

        Backdrop {
            anchors.fill: parent
            visible: layer.scene === "system" || (layer.scene === "" && (String(layer.source) === "" || picture.status === Image.Error))
        }

        Loader {
            anchors.fill: parent
            active: layer.scene === "welcome"
            sourceComponent: WelcomeScene {
                running: root.live && layer.opacity > 0.5
            }
        }

        Image {
            id: picture
            anchors.fill: parent
            source: layer.source
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            smooth: true
            cache: true
            sourceSize.width: 1920
            visible: layer.scene === "" && status === Image.Ready
        }

        Rectangle {
            anchors.fill: parent
            visible: layer.spec && layer.spec.cropped === true
            color: Qt.rgba(0.03, 0.035, 0.05, 0.5)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Layer {
        id: a
        opacity: root.showA ? 1.0 : 0.0
    }

    Layer {
        id: b
        opacity: root.showA ? 0.0 : 1.0
    }

    Item {
        anchors.fill: parent
        visible: root.shaded

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0, 0, 0, 0.14)
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            height: parent.height * 0.36
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0.55)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
            }
        }

        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: parent.width * 0.6
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0.5)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.height * 0.42
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0.72)
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#05060a"
        opacity: root.dim
    }
}
