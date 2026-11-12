pragma Singleton
import QtQuick
import QtMultimedia

// A player destroyed while it opens its file blocks the GUI thread on the open, whose warning about it needs the GIL a Python caller may hold: a released one goes once its open settled.
QtObject {
    id: clips

    property Component looping: Component {
        MediaPlayer {
            loops: MediaPlayer.Infinite
        }
    }

    property Component recordingPlayer: Component {
        MediaPlayer {
            // With no device, the audio renderer's thread warns into the Python log handler, which waits on the GIL a stop() from Python holds.
            audioOutput: devices.audioOutputs.length > 0 ? sound : null

            readonly property MediaDevices devices: MediaDevices {}
            readonly property AudioOutput sound: AudioOutput {}
        }
    }

    function recording(output) {
        return recordingPlayer.createObject(clips, {
            videoOutput: output
        });
    }

    function open(file, output) {
        const player = looping.createObject(clips, {
            source: file,
            videoOutput: output
        });
        player.play();
        return player;
    }

    function release(player) {
        if (!player)
            return;
        player.videoOutput = null;
        if (player.mediaStatus !== MediaPlayer.LoadingMedia) {
            player.destroy();
            return;
        }
        player.mediaStatusChanged.connect(() => player.destroy());
    }
}
