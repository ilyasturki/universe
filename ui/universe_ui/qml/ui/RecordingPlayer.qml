import QtMultimedia

MediaPlayer {
    // With no device, the audio renderer's thread warns into the Python log handler, which waits on the GIL a stop() from Python holds.
    audioOutput: devices.audioOutputs.length > 0 ? sound : null

    readonly property MediaDevices devices: MediaDevices {}
    readonly property AudioOutput sound: AudioOutput {}
}
