import QtQuick
import "../core"

// The console's line icons on a 24-unit grid: `stroke` paths drawn with the pen, `fill` paths filled.
Canvas {
    id: glyph

    property string kind: ""
    property color tint: Theme.text
    property real stroke: 1.8

    readonly property var glyphs: ({
            "search": {
                stroke: "M10.5 3.8A6.7 6.7 0 1 1 10.5 17.2A6.7 6.7 0 1 1 10.5 3.8ZM15.4 15.4L20.6 20.6",
                weight: 2.5
            },
            "home": {
                stroke: "M3.2 11.2L12 3.8L20.8 11.2M5.8 9.4V20.2H10.2V14.6H13.8V20.2H18.2V9.4"
            },
            "power": {
                stroke: "M12 3.2V11.4M7 6.3A7.6 7.6 0 1 0 17 6.3",
                weight: 2.1
            },
            "sound": {
                stroke: "M16 9.2A4 4 0 0 1 16 14.8M18.6 6.6A7.6 7.6 0 0 1 18.6 17.4",
                fill: "M3.5 9H7.5L12.5 4.8V19.2L7.5 15H3.5Z"
            },
            "mute": {
                stroke: "M16.2 9.6L20.8 14.2M20.8 9.6L16.2 14.2",
                fill: "M3.5 9H7.5L12.5 4.8V19.2L7.5 15H3.5Z"
            },
            "bell": {
                stroke: "M6 16.4V11.2A6 6 0 0 1 18 11.2V16.4L19.6 18.2H4.4ZM10 20.6A2 2 0 0 0 14 20.6"
            },
            "trophy": {
                stroke: "M7 4H17V9A5 5 0 0 1 7 9ZM7 6H4V7.5A3.5 3.5 0 0 0 7.5 11M17 6H20V7.5A3.5 3.5 0 0 1 16.5 11M12 14V17.5M8 20.2H16"
            },
            "capture": {
                stroke: "M3.2 8V5.2A2 2 0 0 1 5.2 3.2H8M16 3.2H18.8A2 2 0 0 1 20.8 5.2V8M20.8 16V18.8A2 2 0 0 1 18.8 20.8H16M8 20.8H5.2A2 2 0 0 1 3.2 18.8V16",
                fill: "M7.6 10H9.6L10.6 8.6H13.4L14.4 10H16.4V16H7.6ZM13.6 13A1.6 1.6 0 1 0 10.4 13A1.6 1.6 0 1 0 13.6 13Z",
                evenOdd: true
            },
            "gallery": {
                stroke: "M3.2 4.2H18.8V14.2M3.2 4.2V16.8H10.6",
                fill: "M12.4 13.2H14.8L15.8 11.6H18.6L19.6 13.2H22V20.4H12.4ZM18.8 16.8A1.6 1.6 0 1 0 15.6 16.8A1.6 1.6 0 1 0 18.8 16.8Z",
                evenOdd: true
            },
            "store": {
                stroke: "M5.2 8.2H18.8L17.8 20.8H6.2ZM9 8.2V6.6A3 3 0 0 1 15 6.6V8.2"
            },
            "library": {
                fill: "M3 3.2H7.2V7.4H3ZM9 3.2H13.2V7.4H9ZM15 3.2H19.2V7.4H15ZM3 9.2H7.2V13.4H3ZM9 9.2H13.2V13.4H9ZM12.4 15.4H20.4A2.6 2.6 0 0 1 23 18V19.4A2 2 0 0 1 19.4 20.6L18.6 19.6H14.2L13.4 20.6A2 2 0 0 1 9.8 19.4V18A2.6 2.6 0 0 1 12.4 15.4Z"
            },
            "welcome": {
                stroke: "M3.8 3.8H10V10H3.8ZM16.9 3.4L20.6 10H13.2ZM4.2 14.2L9.6 19.6M9.6 14.2L4.2 19.6M20.4 16.9A3.3 3.3 0 1 1 13.8 16.9A3.3 3.3 0 1 1 20.4 16.9Z"
            },
            "gamepad": {
                stroke: "M7.2 6.8H16.8C20.4 6.8 22 10 22 14.6C22 17.6 19.6 19 17.6 17.2L15.9 15.6H8.1L6.4 17.2C4.4 19 2 17.6 2 14.6C2 10 3.6 6.8 7.2 6.8ZM7.4 9.6V13M5.7 11.3H9.1",
                fill: "M17.9 10.2A1 1 0 1 1 15.9 10.2A1 1 0 1 1 17.9 10.2ZM16.4 12.6A1 1 0 1 1 14.4 12.6A1 1 0 1 1 16.4 12.6Z"
            },
            "controllers": {
                stroke: "M7.2 6.8H16.8C20.4 6.8 22 10 22 14.6C22 17.6 19.6 19 17.6 17.2L15.9 15.6H8.1L6.4 17.2C4.4 19 2 17.6 2 14.6C2 10 3.6 6.8 7.2 6.8ZM7.4 9.6V13M5.7 11.3H9.1",
                fill: "M17.9 10.2A1 1 0 1 1 15.9 10.2A1 1 0 1 1 17.9 10.2ZM16.4 12.6A1 1 0 1 1 14.4 12.6A1 1 0 1 1 16.4 12.6Z"
            },
            "user": {
                stroke: "M12 3.5A4 4 0 1 1 12 11.5A4 4 0 1 1 12 3.5ZM4.5 20.5C5.2 16.5 8.2 14.5 12 14.5C15.8 14.5 18.8 16.5 19.5 20.5"
            },
            "info": {
                stroke: "M21.5 12A9.5 9.5 0 1 1 2.5 12A9.5 9.5 0 1 1 21.5 12ZM12 11V16.6",
                fill: "M13.3 7.6A1.3 1.3 0 1 1 10.7 7.6A1.3 1.3 0 1 1 13.3 7.6Z"
            },
            "warning": {
                stroke: "M12 3.4L21.2 19.6H2.8ZM12 9.6V14.4",
                fill: "M13.2 17A1.2 1.2 0 1 1 10.8 17A1.2 1.2 0 1 1 13.2 17Z"
            },
            "more": {
                fill: "M6.6 12A1.7 1.7 0 1 1 3.2 12A1.7 1.7 0 1 1 6.6 12ZM13.7 12A1.7 1.7 0 1 1 10.3 12A1.7 1.7 0 1 1 13.7 12ZM20.8 12A1.7 1.7 0 1 1 17.4 12A1.7 1.7 0 1 1 20.8 12Z"
            },
            "plus": {
                stroke: "M12 4.5V19.5M4.5 12H19.5"
            },
            "minus": {
                stroke: "M4.5 12H19.5"
            },
            "check": {
                stroke: "M4.5 12.5L10 18L19.5 6.5",
                weight: 2.2
            },
            "cross": {
                stroke: "M5.5 5.5L18.5 18.5M18.5 5.5L5.5 18.5",
                weight: 2.2
            },
            "play": {
                fill: "M7 4.4L19.6 12L7 19.6Z"
            },
            "pause": {
                fill: "M6.5 4.5H10V19.5H6.5ZM14 4.5H17.5V19.5H14Z"
            },
            "stop": {
                fill: "M6.5 6.5H17.5V17.5H6.5Z"
            },
            "snowflake": {
                stroke: "M12 2.8V21.2M4 7.4L20 16.6M20 7.4L4 16.6M9.4 4.4L12 6.8L14.6 4.4M9.4 19.6L12 17.2L14.6 19.6"
            },
            "pulse": {
                stroke: "M2.5 12.4H6.5L9 5.2L14 18.8L16.5 12.4H21.5"
            },
            "bolt": {
                fill: "M13.4 2.2L4.4 13.6H11L10.2 21.8L19.6 9.8H13.2Z"
            },
            "sliders": {
                stroke: "M3 6H21M3 12H21M3 18H21",
                fill: "M17.4 6A2.4 2.4 0 1 1 12.6 6A2.4 2.4 0 1 1 17.4 6ZM10.4 12A2.4 2.4 0 1 1 5.6 12A2.4 2.4 0 1 1 10.4 12ZM14.4 18A2.4 2.4 0 1 1 9.6 18A2.4 2.4 0 1 1 14.4 18Z"
            },
            "folder": {
                stroke: "M3 6H9.5L11.5 8.5H21V19H3Z"
            },
            "file": {
                stroke: "M6 3H14L19 8V21H6ZM14 3V8H19"
            },
            "trash": {
                stroke: "M4 7H20M9 7V4H15V7M6 7L7 20H17L18 7M10 11V16.5M14 11V16.5"
            },
            "refresh": {
                stroke: "M15.63 4.87A8 8 0 1 1 8.37 4.87M17 3.5L17.6 8.3L12.8 7.6"
            },
            "eye-off": {
                stroke: "M3 12C6 6.5 18 6.5 21 12C18 17.5 6 17.5 3 12ZM15 12A3 3 0 1 1 9 12A3 3 0 1 1 15 12ZM5 20L19 4"
            },
            "eye": {
                stroke: "M3 12C6 6.5 18 6.5 21 12C18 17.5 6 17.5 3 12ZM15 12A3 3 0 1 1 9 12A3 3 0 1 1 15 12Z"
            },
            "image": {
                stroke: "M5 4H19A2 2 0 0 1 21 6V18A2 2 0 0 1 19 20H5A2 2 0 0 1 3 18V6A2 2 0 0 1 5 4ZM3.5 17L9 11.5L13 15.5L16 12.5L20.5 17",
                fill: "M17.1 8.5A1.6 1.6 0 1 1 13.9 8.5A1.6 1.6 0 1 1 17.1 8.5Z"
            },
            "film": {
                stroke: "M5 5H19A2 2 0 0 1 21 7V17A2 2 0 0 1 19 19H5A2 2 0 0 1 3 17V7A2 2 0 0 1 5 5ZM7 5V19M17 5V19M3 9.5H7M3 14.5H7M17 9.5H21M17 14.5H21"
            },
            "journal": {
                stroke: "M12 6C9 4 5 4 3 5V19C5 18 9 18 12 20C15 18 19 18 21 19V5C19 4 15 4 12 6ZM12 6V20"
            },
            "download": {
                stroke: "M12 3V15M6.5 10L12 15.5L17.5 10M4 20H20"
            },
            "heart": {
                fill: "M12 20.4L10.6 19.1C5.6 14.6 2.4 11.7 2.4 8.1C2.4 5.2 4.7 3 7.5 3C9.1 3 10.7 3.8 12 5C13.3 3.8 14.9 3 16.5 3C19.3 3 21.6 5.2 21.6 8.1C21.6 11.7 18.4 14.6 13.4 19.1Z"
            },
            "heart-outline": {
                stroke: "M12 20L10.8 18.9C6 14.6 3 11.8 3 8.4C3 5.8 5 3.8 7.6 3.8C9.1 3.8 10.6 4.5 12 5.9C13.4 4.5 14.9 3.8 16.4 3.8C19 3.8 21 5.8 21 8.4C21 11.8 18 14.6 13.2 18.9Z"
            },
            "sort": {
                stroke: "M6.5 4V19.5M3.2 16.4L6.5 19.8L9.8 16.4M12.5 6H21M12.5 11H19M12.5 16H17",
                weight: 2.1
            },
            "filter": {
                stroke: "M3.5 5H20.5L14 12.6V19L10 21V12.6Z"
            },
            "select": {
                stroke: "M7.5 7.5H19A1.5 1.5 0 0 1 20.5 9V19A1.5 1.5 0 0 1 19 20.5H9A1.5 1.5 0 0 1 7.5 19ZM4 16.5V5A1.5 1.5 0 0 1 5.5 3.5H17M10.5 14L13 16.5L17.5 11.5"
            },
            "list-add": {
                stroke: "M3 6.5H15M3 11.5H15M3 16.5H10M18 12.5V20.5M14 16.5H22"
            },
            "tag": {
                stroke: "M3.5 12V4.5A1 1 0 0 1 4.5 3.5H12L20.5 12L12 20.5Z",
                fill: "M9.6 7.8A1.6 1.6 0 1 1 6.4 7.8A1.6 1.6 0 1 1 9.6 7.8Z"
            },
            "clock": {
                stroke: "M21 12A9 9 0 1 1 3 12A9 9 0 1 1 21 12ZM12 7V12.4L15.4 14.4"
            },
            "globe": {
                stroke: "M21 12A9 9 0 1 1 3 12A9 9 0 1 1 21 12ZM3 12H21M12 3C9.4 5.6 9.4 18.4 12 21M12 3C14.6 5.6 14.6 18.4 12 21"
            },
            "cube": {
                stroke: "M12 2.8L20.5 7.4V16.6L12 21.2L3.5 16.6V7.4ZM3.5 7.4L12 12L20.5 7.4M12 12V21.2"
            },
            "storage": {
                stroke: "M4 6.2A8 2.8 0 1 0 20 6.2A8 2.8 0 1 0 4 6.2M4 6.2V17.8A8 2.8 0 0 0 20 17.8V6.2M4 12A8 2.8 0 0 0 20 12"
            },
            "accessibility": {
                stroke: "M21 12A9 9 0 1 1 3 12A9 9 0 1 1 21 12ZM7 9.2L12 10.4L17 9.2M12 10.4V14M9.4 18.4L12 14L14.6 18.4",
                fill: "M13.3 6.8A1.3 1.3 0 1 1 10.7 6.8A1.3 1.3 0 1 1 13.3 6.8Z"
            },
            "doctor": {
                stroke: "M12 20L10.8 18.9C6 14.6 3 11.8 3 8.4C3 5.8 5 3.8 7.6 3.8C9.1 3.8 10.6 4.5 12 5.9C13.4 4.5 14.9 3.8 16.4 3.8C19 3.8 21 5.8 21 8.4C21 11.8 18 14.6 13.2 18.9ZM6 12H9L10.5 9.2L13.2 14.6L14.8 12H18"
            },
            "palette": {
                stroke: "M12 3A9 9 0 1 0 12 21C13.4 21 14 20.2 14 19.2C14 17.8 12.8 17.4 12.8 16.2C12.8 15 13.8 14.4 15 14.4H17C19.2 14.4 21 12.6 21 10.4C21 6.2 17 3 12 3Z",
                fill: "M8.4 10.2A1.2 1.2 0 1 1 6 10.2A1.2 1.2 0 1 1 8.4 10.2ZM11.4 7A1.2 1.2 0 1 1 9 7A1.2 1.2 0 1 1 11.4 7ZM16 7.6A1.2 1.2 0 1 1 13.6 7.6A1.2 1.2 0 1 1 16 7.6Z"
            },
            "rocket": {
                stroke: "M12 2.8C15.6 5.2 17 9 16 14.2H8C7 9 8.4 5.2 12 2.8ZM8 14.2L5.4 17.2L8.6 17.6M16 14.2L18.6 17.2L15.4 17.6M10.2 18.2L12 21.2L13.8 18.2",
                fill: "M13.4 9A1.4 1.4 0 1 1 10.6 9A1.4 1.4 0 1 1 13.4 9Z"
            },
            "chip": {
                stroke: "M7 6H17A1 1 0 0 1 18 7V17A1 1 0 0 1 17 18H7A1 1 0 0 1 6 17V7A1 1 0 0 1 7 6ZM9.5 9.5H14.5V14.5H9.5ZM9 6V3.5M12 6V3.5M15 6V3.5M9 18V20.5M12 18V20.5M15 18V20.5M6 9H3.5M6 12H3.5M6 15H3.5M18 9H20.5M18 12H20.5M18 15H20.5"
            },
            "puzzle": {
                stroke: "M4 8H8.2A2.2 2.2 0 1 1 12.6 8H16.8V12.2A2.2 2.2 0 1 1 16.8 16.6V20.8H12.6A2.2 2.2 0 1 0 8.2 20.8H4V16.6A2.2 2.2 0 1 0 4 12.2Z"
            },
            "cloud": {
                stroke: "M7 19H17.5A4.5 4.5 0 0 0 17.2 10A6 6 0 0 0 5.6 11.5A3.8 3.8 0 0 0 7 19Z"
            },
            "display": {
                stroke: "M3.5 4.8H20.5V16.2H3.5ZM8.5 20H15.5M12 16.2V20"
            },
            "headphones": {
                stroke: "M4 15V12A8 8 0 0 1 20 12V15M4 15H7V20H5A1 1 0 0 1 4 19ZM20 15H17V20H19A1 1 0 0 0 20 19Z"
            },
            "keyboard": {
                stroke: "M3 6.5H21V17.5H3ZM6.5 10H7.5M10 10H11M13.5 10H14.5M17 10H17.5M6.5 13.8H7.5M9.5 13.8H14.5M17 13.8H17.5"
            },
            "lock": {
                stroke: "M6.5 11H17.5A1.5 1.5 0 0 1 19 12.5V19.5A1.5 1.5 0 0 1 17.5 21H6.5A1.5 1.5 0 0 1 5 19.5V12.5A1.5 1.5 0 0 1 6.5 11ZM8 11V7.5A4 4 0 0 1 16 7.5V11"
            },
            "star": {
                fill: "M12 2.8L14.8 8.6L21.2 9.4L16.5 13.8L17.7 20.2L12 17.1L6.3 20.2L7.5 13.8L2.8 9.4L9.2 8.6Z"
            },
            "chevron": {
                stroke: "M9 5L16 12L9 19"
            },
            "chevron-down": {
                stroke: "M5 9L12 16L19 9"
            },
            "back": {
                stroke: "M15 5L8 12L15 19"
            },
            "record": {
                fill: "M18 12A6 6 0 1 1 6 12A6 6 0 1 1 18 12Z"
            },
            "grid": {
                stroke: "M4.5 3H9A1.5 1.5 0 0 1 10.5 4.5V9A1.5 1.5 0 0 1 9 10.5H4.5A1.5 1.5 0 0 1 3 9V4.5A1.5 1.5 0 0 1 4.5 3ZM15 3H19.5A1.5 1.5 0 0 1 21 4.5V9A1.5 1.5 0 0 1 19.5 10.5H15A1.5 1.5 0 0 1 13.5 9V4.5A1.5 1.5 0 0 1 15 3ZM4.5 13.5H9A1.5 1.5 0 0 1 10.5 15V19.5A1.5 1.5 0 0 1 9 21H4.5A1.5 1.5 0 0 1 3 19.5V15A1.5 1.5 0 0 1 4.5 13.5ZM15 13.5H19.5A1.5 1.5 0 0 1 21 15V19.5A1.5 1.5 0 0 1 19.5 21H15A1.5 1.5 0 0 1 13.5 19.5V15A1.5 1.5 0 0 1 15 13.5Z"
            },
            "key": {
                stroke: "M12.5 12A4 4 0 1 1 4.5 12A4 4 0 1 1 12.5 12ZM12.5 12H20.5M17.5 12V15M20.5 12V14.5"
            },
            "gauge": {
                stroke: "M4.2 17.5A9 9 0 1 1 19.8 17.5M12 13L16 8.5",
                fill: "M13.5 13A1.5 1.5 0 1 1 10.5 13A1.5 1.5 0 1 1 13.5 13Z"
            },
            "sun": {
                stroke: "M16 12A4 4 0 1 1 8 12A4 4 0 1 1 16 12ZM12 2.8V5M12 19V21.2M2.8 12H5M19 12H21.2M5.5 5.5L7.1 7.1M16.9 16.9L18.5 18.5M18.5 5.5L16.9 7.1M7.1 16.9L5.5 18.5"
            },
            "moon": {
                stroke: "M20 14.5A8.5 8.5 0 1 1 9.5 4A7 7 0 0 0 20 14.5Z"
            },
            "exit": {
                stroke: "M10 4H5V20H10M15 8L19 12L15 16M19 12H9"
            },
            "restart": {
                stroke: "M12 3.2V5.8M12 18.2V20.8M3.2 12H5.8M18.2 12H20.8M5.8 5.8L7.6 7.6M16.4 16.4L18.2 18.2M18.2 5.8L16.4 7.6M7.6 16.4L5.8 18.2",
                weight: 2.4
            },
            "ring": {
                stroke: "M20.5 12A8.5 8.5 0 1 1 3.5 12A8.5 8.5 0 1 1 20.5 12Z",
                weight: 2.2
            },
            "terminal": {
                stroke: "M4 5H20V19H4ZM7.5 9.5L10.5 12L7.5 14.5M12.5 15H16.5"
            },
            "news": {
                stroke: "M4 5H16V19H6A2 2 0 0 1 4 17ZM16 9H20V17A2 2 0 0 1 18 19H16M7 9H13M7 12.5H13M7 16H11"
            },
            "link": {
                stroke: "M10 14A4 4 0 0 0 15.7 14.3L18.6 11.4A4 4 0 0 0 12.9 5.7L11.6 7M14 10A4 4 0 0 0 8.3 9.7L5.4 12.6A4 4 0 0 0 11.1 18.3L12.4 17"
            },
            "cursor": {
                fill: "M5 3L19 12.5L12.4 13.6L16 20.4L13.4 21.6L9.8 14.8L5 19.2Z"
            },
            "screen": {
                stroke: "M3.5 4.8H20.5V16.2H3.5ZM8.5 20H15.5M12 16.2V20"
            }
        })

    onKindChanged: requestPaint()
    onTintChanged: requestPaint()
    onStrokeChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d");
        ctx.reset();
        var s = width / 24;
        ctx.scale(s, s);
        ctx.fillStyle = tint;
        ctx.strokeStyle = tint;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        if (kind === "settings") {
            gear(ctx);
            return;
        }
        var g = glyphs[kind];
        if (!g)
            return;
        ctx.lineWidth = g.weight !== undefined ? g.weight * stroke / 1.8 : stroke;
        if (g.stroke) {
            ctx.beginPath();
            ctx.path = g.stroke;
            ctx.stroke();
        }
        if (g.fill) {
            ctx.beginPath();
            ctx.fillRule = g.evenOdd ? Qt.OddEvenFill : Qt.WindingFill;
            ctx.path = g.fill;
            ctx.fill();
        }
    }

    // The console's settings gear is solid: eight square teeth, a hole.
    function gear(ctx) {
        var teeth = 8, outer = 10.4, inner = 7.9, hole = 3.3;
        ctx.beginPath();
        for (var i = 0; i < teeth; i++) {
            var a = i * 2 * Math.PI / teeth;
            var half = Math.PI / teeth * 0.52;
            var pts = [[a - half * 1.35, inner], [a - half, outer], [a + half, outer], [a + half * 1.35, inner]];
            for (var k = 0; k < pts.length; k++) {
                var x = 12 + Math.cos(pts[k][0]) * pts[k][1], y = 12 + Math.sin(pts[k][0]) * pts[k][1];
                if (i === 0 && k === 0)
                    ctx.moveTo(x, y);
                else
                    ctx.lineTo(x, y);
            }
            var b = (i + 1) * 2 * Math.PI / teeth - half * 1.35;
            ctx.arc(12, 12, inner, a + half * 1.35, b, false);
        }
        ctx.closePath();
        ctx.moveTo(12 + hole, 12);
        ctx.arc(12, 12, hole, 0, Math.PI * 2, true);
        ctx.fillRule = Qt.OddEvenFill;
        ctx.fill();
    }
}
