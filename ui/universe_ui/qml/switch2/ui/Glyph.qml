import QtQuick
import "../core"

Canvas {
    id: glyph

    property string kind: ""
    property color tint: Theme.text
    property real stroke: 1.9

    onKindChanged: requestPaint()
    onTintChanged: requestPaint()
    onWidthChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d");
        ctx.reset();
        var s = width / 24;
        ctx.fillStyle = tint;
        ctx.strokeStyle = tint;
        ctx.lineWidth = stroke * s;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        ctx.beginPath();
        function rr(x, y, w, h, r) {
            ctx.beginPath();
            ctx.roundedRect(x * s, y * s, w * s, h * s, r * s, r * s);
        }
        function line(a, b, c, d) {
            ctx.beginPath();
            ctx.moveTo(a * s, b * s);
            ctx.lineTo(c * s, d * s);
            ctx.stroke();
        }
        function dot(x, y, r) {
            ctx.beginPath();
            ctx.arc(x * s, y * s, r * s, 0, Math.PI * 2);
            ctx.fill();
        }
        if (kind === "news") {
            rr(3.5, 4, 17, 16, 2.5);
            ctx.stroke();
            rr(6.5, 7, 5, 4, 0.8);
            ctx.stroke();
            line(14, 8, 17.5, 8);
            line(14, 10.5, 17.5, 10.5);
            line(6.5, 14, 17.5, 14);
            line(6.5, 17, 17.5, 17);
        } else if (kind === "shop") {
            ctx.moveTo(5 * s, 9 * s);
            ctx.lineTo(19 * s, 9 * s);
            ctx.lineTo(20 * s, 20 * s);
            ctx.lineTo(4 * s, 20 * s);
            ctx.closePath();
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(8.5 * s, 9 * s);
            ctx.lineTo(8.5 * s, 7 * s);
            ctx.arc(12 * s, 7 * s, 3.5 * s, Math.PI, 0);
            ctx.lineTo(15.5 * s, 9 * s);
            ctx.stroke();
        } else if (kind === "album") {
            rr(3, 5, 18, 14, 2.5);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(6 * s, 16 * s);
            ctx.lineTo(10 * s, 11 * s);
            ctx.lineTo(13 * s, 14 * s);
            ctx.lineTo(15 * s, 12 * s);
            ctx.lineTo(18 * s, 16 * s);
            ctx.stroke();
            dot(15.5, 8.5, 1.3);
        } else if (kind === "controllers") {
            ctx.save();
            ctx.translate(12 * s, 12 * s);
            ctx.rotate(-Math.PI / 5);
            ctx.translate(-12 * s, -12 * s);
            rr(9, 2.5, 6, 19, 3);
            ctx.stroke();
            dot(12, 8, 1.4);
            line(10.8, 15, 13.2, 15);
            line(12, 13.8, 12, 16.2);
            ctx.restore();
        } else if (kind === "settings") {
            dot(12, 12, 3.2);
            ctx.beginPath();
            ctx.arc(12 * s, 12 * s, 3.2 * s, 0, Math.PI * 2);
            ctx.stroke();
            for (var i = 0; i < 8; i++) {
                var a = i * Math.PI / 4, r0 = 6.2, r1 = 9.5;
                line(12 + Math.cos(a) * r0, 12 + Math.sin(a) * r0, 12 + Math.cos(a) * r1, 12 + Math.sin(a) * r1);
            }
        } else if (kind === "power") {
            ctx.beginPath();
            ctx.arc(12 * s, 13 * s, 8 * s, -Math.PI * 0.32, Math.PI * 1.32);
            ctx.stroke();
            line(12, 3.5, 12, 11.5);
        } else if (kind === "grid") {
            rr(4, 4, 6.5, 6.5, 1.2);
            ctx.stroke();
            rr(13.5, 4, 6.5, 6.5, 1.2);
            ctx.stroke();
            rr(4, 13.5, 6.5, 6.5, 1.2);
            ctx.stroke();
            rr(13.5, 13.5, 6.5, 6.5, 1.2);
            ctx.stroke();
        } else if (kind === "search") {
            ctx.beginPath();
            ctx.arc(10.5 * s, 10.5 * s, 6.5 * s, 0, Math.PI * 2);
            ctx.stroke();
            line(15.5, 15.5, 20.5, 20.5);
        } else if (kind === "filter") {
            ctx.moveTo(4 * s, 5 * s);
            ctx.lineTo(20 * s, 5 * s);
            ctx.lineTo(14 * s, 12.5 * s);
            ctx.lineTo(14 * s, 19 * s);
            ctx.lineTo(10 * s, 17 * s);
            ctx.lineTo(10 * s, 12.5 * s);
            ctx.closePath();
            ctx.stroke();
        } else if (kind === "sort") {
            line(8, 4, 8, 20);
            line(4.5, 16.5, 8, 20);
            line(11.5, 16.5, 8, 20);
            line(16, 20, 16, 4);
            line(12.5, 7.5, 16, 4);
            line(19.5, 7.5, 16, 4);
        } else if (kind === "check") {
            ctx.moveTo(5 * s, 12.5 * s);
            ctx.lineTo(10 * s, 17.5 * s);
            ctx.lineTo(19 * s, 7 * s);
            ctx.stroke();
        } else if (kind === "cross") {
            line(5, 5, 19, 19);
            line(19, 5, 5, 19);
        } else if (kind === "plus") {
            line(12, 4.5, 12, 19.5);
            line(4.5, 12, 19.5, 12);
        } else if (kind === "chevron-left") {
            ctx.moveTo(14.5 * s, 6 * s);
            ctx.lineTo(9 * s, 12 * s);
            ctx.lineTo(14.5 * s, 18 * s);
            ctx.stroke();
        } else if (kind === "film") {
            rr(3.5, 5, 17, 14, 2);
            ctx.stroke();
            line(8, 5, 8, 19);
            line(16, 5, 16, 19);
            line(3.5, 12, 8, 12);
            line(16, 12, 20.5, 12);
        } else if (kind === "play") {
            ctx.moveTo(7 * s, 4 * s);
            ctx.lineTo(20 * s, 12 * s);
            ctx.lineTo(7 * s, 20 * s);
            ctx.closePath();
            ctx.fill();
        } else if (kind === "refresh") {
            ctx.beginPath();
            ctx.arc(12 * s, 12 * s, 7.5 * s, -Math.PI * 0.35, Math.PI * 1.35);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(16.5 * s, 4 * s);
            ctx.lineTo(17.2 * s, 8.6 * s);
            ctx.lineTo(12.6 * s, 8 * s);
            ctx.stroke();
        } else if (kind === "key") {
            ctx.beginPath();
            ctx.arc(8.5 * s, 12 * s, 4 * s, 0, Math.PI * 2);
            ctx.stroke();
            line(12.5, 12, 20.5, 12);
            line(17.5, 12, 17.5, 15);
            line(20.5, 12, 20.5, 14.5);
        } else if (kind === "folder") {
            ctx.moveTo(3.5 * s, 6.5 * s);
            ctx.lineTo(9.5 * s, 6.5 * s);
            ctx.lineTo(11.5 * s, 9 * s);
            ctx.lineTo(20.5 * s, 9 * s);
            ctx.lineTo(20.5 * s, 19 * s);
            ctx.lineTo(3.5 * s, 19 * s);
            ctx.closePath();
            ctx.stroke();
        } else if (kind === "gamepad") {
            ctx.moveTo(7 * s, 6.5 * s);
            ctx.lineTo(17 * s, 6.5 * s);
            ctx.bezierCurveTo(21 * s, 6.5 * s, 22.5 * s, 10 * s, 22 * s, 15 * s);
            ctx.bezierCurveTo(21.6 * s, 19 * s, 18 * s, 19.5 * s, 16.5 * s, 16.5 * s);
            ctx.lineTo(15.5 * s, 14.5 * s);
            ctx.lineTo(8.5 * s, 14.5 * s);
            ctx.lineTo(7.5 * s, 16.5 * s);
            ctx.bezierCurveTo(6 * s, 19.5 * s, 2.4 * s, 19 * s, 2 * s, 15 * s);
            ctx.bezierCurveTo(1.5 * s, 10 * s, 3 * s, 6.5 * s, 7 * s, 6.5 * s);
            ctx.closePath();
            ctx.fill();
        }
    }
}
