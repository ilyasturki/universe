import QtQuick
import QtMultimedia

QtObject {
    id: scrub

    property var player: null
    property real duration: 0
    property bool active: false

    property bool scrubbing: false
    property real scrubPos: 0
    readonly property real shownPos: scrubbing ? scrubPos : (player ? player.position : 0)
    readonly property real stickX: api.pad.rightX
    // ms per second at full tilt: a dozen seconds across the recording, never under a minute a second.
    readonly property real speed: Math.max(60000, duration / 12)
    readonly property real step: 10000

    signal woke

    function begin() {
        if (!scrubbing) {
            scrubPos = player.position;
            scrubbing = true;
        }
    }

    function seekBy(ms) {
        if (duration <= 0)
            return;
        commitTimer.stop();
        begin();
        scrubPos = Math.max(0, Math.min(duration, scrubPos + ms));
        woke();
    }

    // A click on the bar: straight to that point.
    function seekTo(ms) {
        if (duration <= 0)
            return;
        commitTimer.stop();
        begin();
        scrubPos = Math.max(0, Math.min(duration, ms));
        woke();
        commitSeek();
    }

    function commitSeek() {
        if (!scrubbing)
            return;
        scrubbing = false;
        if (player.playbackState === MediaPlayer.StoppedState) {
            player.play();
            player.pause();
        }
        player.position = scrubPos;
    }

    function release() {
        if (scrubbing && !stickTimer.running)
            commitTimer.restart();
    }

    readonly property Timer commitTimer: Timer {
        interval: 220
        onTriggered: scrub.commitSeek()
    }

    readonly property Timer stickTimer: Timer {
        interval: 16
        repeat: true
        running: scrub.active && scrub.stickX !== 0 && scrub.duration > 0
        onRunningChanged: {
            if (running) {
                scrub.commitTimer.stop();
                scrub.begin();
                scrub.woke();
            } else if (scrub.scrubbing) {
                scrub.commitTimer.restart();
            }
        }
        onTriggered: {
            var x = scrub.stickX;
            var v = (0.12 + 0.88 * x * x) * scrub.speed;
            scrub.scrubPos = Math.max(0, Math.min(scrub.duration, scrub.scrubPos + (x < 0 ? -1 : 1) * v * interval / 1000));
        }
    }
}
