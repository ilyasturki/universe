import QtQuick

// One line icon of a menu row, drawn on a 24-unit grid in the row's ink.
Canvas {
    id: glyph

    property string kind: ""
    property color tint: "#ffffff"

    onKindChanged: requestPaint()
    onTintChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d");
        ctx.reset();
        var s = width / 24;
        ctx.fillStyle = tint;
        ctx.strokeStyle = tint;
        ctx.lineWidth = 2 * s;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        ctx.beginPath();
        if (kind === "play") {
            ctx.moveTo(6 * s, 3 * s);
            ctx.lineTo(21 * s, 12 * s);
            ctx.lineTo(6 * s, 21 * s);
            ctx.closePath();
            ctx.fill();
        } else if (kind === "info") {
            ctx.arc(12 * s, 12 * s, 10 * s, 0, Math.PI * 2);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(12 * s, 11 * s);
            ctx.lineTo(12 * s, 16.5 * s);
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(12 * s, 7.6 * s, 1.3 * s, 0, Math.PI * 2);
            ctx.fill();
        } else if (kind === "stop") {
            ctx.roundedRect(5 * s, 5 * s, 14 * s, 14 * s, 2 * s, 2 * s);
            ctx.fill();
        } else if (kind === "sliders") {
            [6, 12, 18].forEach(function(y, i) {
                ctx.beginPath();
                ctx.moveTo(3 * s, y * s);
                ctx.lineTo(21 * s, y * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.arc([15, 8, 12][i] * s, y * s, 2.4 * s, 0, Math.PI * 2);
                ctx.fill();
            });
        } else if (kind === "film") {
            ctx.roundedRect(3 * s, 5 * s, 18 * s, 14 * s, 2 * s, 2 * s);
            ctx.stroke();
            [7, 12, 17].forEach(function(x) {
                ctx.beginPath();
                ctx.moveTo(x * s, 5 * s);
                ctx.lineTo(x * s, 19 * s);
                ctx.stroke();
            });
        } else if (kind === "book") {
            ctx.moveTo(12 * s, 6 * s);
            ctx.bezierCurveTo(9 * s, 4 * s, 5 * s, 4 * s, 3 * s, 5 * s);
            ctx.lineTo(3 * s, 19 * s);
            ctx.bezierCurveTo(5 * s, 18 * s, 9 * s, 18 * s, 12 * s, 20 * s);
            ctx.bezierCurveTo(15 * s, 18 * s, 19 * s, 18 * s, 21 * s, 19 * s);
            ctx.lineTo(21 * s, 5 * s);
            ctx.bezierCurveTo(19 * s, 4 * s, 15 * s, 4 * s, 12 * s, 6 * s);
            ctx.closePath();
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(12 * s, 6 * s);
            ctx.lineTo(12 * s, 20 * s);
            ctx.stroke();
        } else if (kind === "download") {
            ctx.moveTo(12 * s, 3 * s);
            ctx.lineTo(12 * s, 15 * s);
            ctx.moveTo(6.5 * s, 10 * s);
            ctx.lineTo(12 * s, 15.5 * s);
            ctx.lineTo(17.5 * s, 10 * s);
            ctx.moveTo(4 * s, 20 * s);
            ctx.lineTo(20 * s, 20 * s);
            ctx.stroke();
        } else if (kind === "refresh") {
            ctx.arc(12 * s, 12 * s, 8 * s, -Math.PI * 0.35, Math.PI * 1.35);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(17 * s, 3.5 * s);
            ctx.lineTo(17.6 * s, 8.3 * s);
            ctx.lineTo(12.8 * s, 7.6 * s);
            ctx.stroke();
        } else if (kind === "trash") {
            ctx.moveTo(4 * s, 7 * s);
            ctx.lineTo(20 * s, 7 * s);
            ctx.moveTo(9 * s, 7 * s);
            ctx.lineTo(9 * s, 4 * s);
            ctx.lineTo(15 * s, 4 * s);
            ctx.lineTo(15 * s, 7 * s);
            ctx.moveTo(6 * s, 7 * s);
            ctx.lineTo(7 * s, 20 * s);
            ctx.lineTo(17 * s, 20 * s);
            ctx.lineTo(18 * s, 7 * s);
            ctx.moveTo(10 * s, 11 * s);
            ctx.lineTo(10 * s, 16.5 * s);
            ctx.moveTo(14 * s, 11 * s);
            ctx.lineTo(14 * s, 16.5 * s);
            ctx.stroke();
        } else if (kind === "eye-off") {
            ctx.moveTo(3 * s, 12 * s);
            ctx.bezierCurveTo(6 * s, 6.5 * s, 18 * s, 6.5 * s, 21 * s, 12 * s);
            ctx.bezierCurveTo(18 * s, 17.5 * s, 6 * s, 17.5 * s, 3 * s, 12 * s);
            ctx.closePath();
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(12 * s, 12 * s, 3 * s, 0, Math.PI * 2);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(5 * s, 20 * s);
            ctx.lineTo(19 * s, 4 * s);
            ctx.stroke();
        } else if (kind === "folder") {
            ctx.moveTo(3 * s, 6 * s);
            ctx.lineTo(9.5 * s, 6 * s);
            ctx.lineTo(11.5 * s, 8.5 * s);
            ctx.lineTo(21 * s, 8.5 * s);
            ctx.lineTo(21 * s, 19 * s);
            ctx.lineTo(3 * s, 19 * s);
            ctx.closePath();
            ctx.stroke();
        } else if (kind === "check") {
            ctx.moveTo(4.5 * s, 12.5 * s);
            ctx.lineTo(10 * s, 18 * s);
            ctx.lineTo(19.5 * s, 6.5 * s);
            ctx.stroke();
        } else if (kind === "keyboard") {
            ctx.roundedRect(3 * s, 6 * s, 18 * s, 12 * s, 2 * s, 2 * s);
            ctx.stroke();
            [[7, 10], [11, 10], [15, 10], [7, 14], [15, 14]].forEach(function(p) {
                ctx.beginPath();
                ctx.arc(p[0] * s, p[1] * s, 0.9 * s, 0, Math.PI * 2);
                ctx.fill();
            });
            ctx.beginPath();
            ctx.moveTo(9.5 * s, 14 * s);
            ctx.lineTo(13.5 * s, 14 * s);
            ctx.stroke();
        } else {
            ctx.moveTo(12 * s, 20 * s);
            ctx.bezierCurveTo(12 * s, 20 * s, 4.5 * s, 15.3 * s, 4.5 * s, 10.4 * s);
            ctx.bezierCurveTo(4.5 * s, 7.2 * s, 9.2 * s, 5.4 * s, 12 * s, 7.6 * s);
            ctx.bezierCurveTo(14.8 * s, 5.4 * s, 19.5 * s, 7.2 * s, 19.5 * s, 10.4 * s);
            ctx.bezierCurveTo(19.5 * s, 15.3 * s, 12 * s, 20 * s, 12 * s, 20 * s);
            ctx.closePath();
            kind === "heart" ? ctx.fill() : ctx.stroke();
        }
    }
}
