.pragma library

function recording(shell, screens, row, done) {
    var buttons = ["Cancel", "Trash recording"];
    if (row.hasJournal)
        buttons.push("Trash both");
    var detail = row.gameTitle + " · " + row.dateText + ". The file goes to the trash; the hours stay."
               + (row.hasJournal ? " Its journal entry can go with it." : "");
    shell.dialogAsk({ message: "Remove this recording?", detail: detail, buttons: buttons, index: 0, danger: buttons.length - 1 }, function(i) {
        if (i < 1)
            return;
        if (i === 2)
            screens.news.remove(row.gameId, row.session);
        screens.album.remove(row.gameId, row.session);
        done();
    });
}

function screenshot(shell, screens, row, done) {
    var detail = row.gameTitle + " · " + row.dateText + ". The picture goes to the trash" + (row.hasJournal ? "; its journal entry keeps the rest." : ".");
    shell.dialogAsk({ message: "Remove this screenshot?", detail: detail, buttons: ["Cancel", "Trash screenshot"], index: 0, danger: 1 }, function(i) {
        if (i < 1)
            return;
        screens.shots.remove(row.gameId, row.name);
        done();
    });
}

function entry(shell, screens, row, done) {
    var pending = row.state === "pending";
    var buttons = ["Cancel", pending ? "Stop writing" : "Trash entry"];
    if (row.hasRecording)
        buttons.push(pending ? "Stop, trash both" : "Trash both");
    var detail = row.gameTitle + " · " + row.dateText + ". "
               + (pending ? "The writing is cancelled." : "The entry and its frames go to the trash; your screenshots stay.")
               + (row.hasRecording ? " Its recording can go with it." : "");
    shell.dialogAsk({ message: pending ? "Cancel this entry?" : "Remove this entry?", detail: detail, buttons: buttons, index: 0, danger: buttons.length - 1 }, function(i) {
        if (i < 1)
            return;
        if (i === 2)
            screens.album.remove(row.gameId, row.session);
        screens.news.remove(row.gameId, row.session);
        done();
    });
}
