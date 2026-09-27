import QtQuick
import "../core"

SequentialAnimation {
    property bool entering: true

    PauseAnimation {
        duration: entering ? Theme.durLeave : 0
    }
    NumberAnimation {
        duration: entering ? Theme.durArrive : Theme.durLeave
        easing.type: Easing.InOutQuad
    }
}
