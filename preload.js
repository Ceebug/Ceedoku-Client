const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("electronAPI", {

    onCSFOpen: (callback) => {

        ipcRenderer.on(
            "open-csf",
            (event, filePath) => {
                callback(filePath);
            }
        );

    }

});