const { app, BrowserWindow, Menu } = require("electron");
const path = require("path");
const fs = require("fs");
const https = require("https");
const cheerio = require("cheerio");

const APP_URL = "https://ceedoku.github.io/";
const CACHE_DIR = path.join(app.getPath("userData"), "cache");
const CURRENT_DIR = path.join(CACHE_DIR, "current");
const UPDATE_DIR = path.join(CACHE_DIR, "update");

let mainWindow;
let pendingCSF = null;


// Get a CSF file path from command-line arguments
function getCSFPath(argv) {
    return argv.find(arg =>
        arg.toLowerCase().endsWith(".csf")
    );
}


// Self-contained crash/offline page
function getOfflinePage() {
    return `
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<meta name="theme-color" content="#101418">

<title>Ceedoku - Offline</title>

<style>
    :root {
        color-scheme: dark;
        --text2: #f1f3f5;
        --surface: #1a1f24;
        --background: #101418;
    }


</style>
</head>

<body class="dark" style="background: var(--background);color: var(--text2);font-family: system-ui, -apple-system, BlinkMacSystemFont, &quot;Segoe UI&quot;, sans-serif;display: flex;align-items: center;justify-content: center;text-align: center;height: 100vh;margin: 0px;">

<div id="loader" style="display: flex;flex-direction: column;place-content: center;place-items: center;">

<img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAABF0AAADfCAMAAAATDlygAAAAAXNSR0IB2cksfwAAAAlwSFlzAAAOxAAADsQBlSsOGwAAAZhQTFRFAAAA/////////////v7+/////////////////////////////v7+/////////////v7+/////v7+/v7+/v7+/v7+/////////////////////v7+/////////////v7+/////////////////////v7+/v7+/////////////////////////////////////v7+/v7+/////////////////////////////v7+/////v7+/////////////////////////////////////////////////////////////////////////////////v7+/////////////////////////////////////////////////////////////v7+/////v7+/////////v7+/////v7+/////////////////////////v7+/v7+/v7+/////////////////////////////v7+/////////v7+/////////////////////////////////////////v7+////SWwVjgAAAIh0Uk5TALb/+6oO+v3x9wzUqAfXfyrNL9KUMbjbTTgixum/yVWx8I310L3O5+bEceKG2iPtFclL+b1Kopu3Y/LM01RTUov2G1HAahymKcaEOUYDSKhNKE+pnp8eJsJcymS0l6t7bXhfc3elBr6ZyEXFu0n8uZg/x+Og2L7gy90Wx7Ou5cPVVesqqoivucvRE3sAABQBSURBVHic7Z1ZrGVFFYZvywxNy6AgyHABkUlR4xiN2mhQo0AUhxgSYxziFEx8MPpkTIzDgw8mxsThwUTjrFGCDGqINHGKCohoBASkZWgGpdsGbOmmmzZn/d30uqdO1V27au9zzoXve/hzs6lTe53au75DUl17r1oAABiCVbMuAAAep2AXABgG7AIAwzDZLqsK/21pm51VZ32S5fZim30sdyTH93ZnX3D9pJX4CtXbI5nedri/c709mhzZ33Jb0pv/Xvskn8r1v5/l1mK1ueOqZIs7cpDl/5KWGpN9LbdlKknJnddzgOV/A72lrLbcskyreiK1HWyp8Xmo2FJXKh3bCE+2fDDQ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" alt="" width="1117" height="223" style="width: 80vw;height: fit-content;">

    <h1 style="color: var(--text2);margin: 0 0 1vw;font-size: 2.3vw;">Could not load Ceedoku</h1>

    <h3 style="margin: 0;line-height: 1.8vw;font-size: 1.4vw;font-weight: 400;color: #c9ced3;width: 100vw;">
        <strong style="line-height: 1.8vw;font-size: 1.4vw;font-weight: 400;color: #c9ced3;">
            Ceedoku could not load because there is not a local
            cached copy of Ceedoku on your device, and the game
            could not be fetched from the website.
        </strong>
        <br>
        <strong style="line-height: 1.8vw;font-size: 1.4vw;color: #c9ced3;">
            If you haven't already, please make sure you have
            a connection to the internet.
        </strong>
    </h3>

</div>



</body></html>
`;
}


// Only allow one Ceedoku instance
const gotTheLock = app.requestSingleInstanceLock();

if (!gotTheLock) {

    app.quit();

} else {

    app.on("second-instance", (event, commandLine) => {

        const csfPath = getCSFPath(commandLine);

        if (mainWindow) {

            if (mainWindow.isMinimized()) {
                mainWindow.restore();
            }

            mainWindow.focus();

            if (csfPath) {

                mainWindow.webContents.send(
                    "open-csf",
                    csfPath
                );

            }

        }

    });


    function download(url) {

        return new Promise((resolve, reject) => {

            https.get(url, response => {

                if (
                    response.statusCode >= 300 &&
                    response.statusCode < 400 &&
                    response.headers.location
                ) {

                    return resolve(
                        download(
                            new URL(
                                response.headers.location,
                                url
                            ).href
                        )
                    );

                }

                if (response.statusCode !== 200) {

                    response.resume();

                    return reject(
                        new Error(
                            `HTTP ${response.statusCode}: ${url}`
                        )
                    );

                }

                const chunks = [];

                response.on("data", chunk => {
                    chunks.push(chunk);
                });

                response.on("end", () => {
                    resolve(Buffer.concat(chunks));
                });

                response.on("error", reject);

            }).on("error", reject);

        });

    }


    function getLocalPath(url, baseUrl) {

        const parsed = new URL(url, baseUrl);

        if (parsed.hostname !== "ceedoku.github.io") {
            return null;
        }

        let pathname = decodeURIComponent(parsed.pathname);

        if (pathname === "/" || pathname === "") {
            pathname = "/index.html";
        }

        return path.join(
            UPDATE_DIR,
            pathname
        );

    }


    async function downloadSite() {

        fs.rmSync(UPDATE_DIR, {
            recursive: true,
            force: true
        });

        fs.mkdirSync(UPDATE_DIR, {
            recursive: true
        });


        const indexUrl = new URL(APP_URL);

        const indexBuffer =
            await download(indexUrl.href);

        const indexPath =
            path.join(
                UPDATE_DIR,
                "index.html"
            );

        fs.mkdirSync(
            path.dirname(indexPath),
            {
                recursive: true
            }
        );

        fs.writeFileSync(
            indexPath,
            indexBuffer
        );


        const $ =
            cheerio.load(
                indexBuffer.toString()
            );

        const resources = new Set();


        $("script[src]").each((_, element) => {
            resources.add($(element).attr("src"));
        });

        $("link[href]").each((_, element) => {
            resources.add($(element).attr("href"));
        });

        $("img[src]").each((_, element) => {
            resources.add($(element).attr("src"));
        });

        $("source[src]").each((_, element) => {
            resources.add($(element).attr("src"));
        });


        // Puzzle worker is required even though
        // it isn't referenced by a src attribute.
        resources.add("src/js/puzzle-worker.js");


        for (const resource of resources) {

            if (!resource) continue;

            const resourceUrl =
                new URL(
                    resource,
                    indexUrl.href
                );

            const localPath =
                getLocalPath(
                    resourceUrl.href,
                    indexUrl.href
                );

            if (!localPath) continue;

            const data =
                await download(
                    resourceUrl.href
                );

            fs.mkdirSync(
                path.dirname(localPath),
                {
                    recursive: true
                }
            );

            fs.writeFileSync(
                localPath,
                data
            );

        }


        fs.rmSync(CURRENT_DIR, {
            recursive: true,
            force: true
        });

        fs.renameSync(
            UPDATE_DIR,
            CURRENT_DIR
        );

    }


    async function prepareCache() {

        const cachedIndex =
            path.join(
                CURRENT_DIR,
                "index.html"
            );

        try {

            await downloadSite();

            return {
                path: cachedIndex,
                offline: false
            };

        } catch (error) {

            console.log(
                "Could not update Ceedoku:",
                error.message
            );


            if (fs.existsSync(cachedIndex)) {

                console.log(
                    "Using cached Ceedoku."
                );

                return {
                    path: cachedIndex,
                    offline: false
                };

            }


            // No cached copy exists.
            return {
                path: null,
                offline: true
            };

        }

    }


    async function createWindow() {

        mainWindow = new BrowserWindow({

            width: 1000,
            height: 700,

            maximizable: true,

            icon: path.join(
                __dirname,
                "assets",
                "ceedoku.ico"
            ),

            webPreferences: {

                preload:
                    path.join(
                        __dirname,
                        "preload.js"
                    ),

                contextIsolation: true,

                nodeIntegration: false,

                webSecurity: false

            }

        });


        mainWindow.maximize();


        const result =
            await prepareCache();


        if (result.offline) {

            await mainWindow.loadURL(
                "data:text/html;charset=utf-8," +
                encodeURIComponent(
                    getOfflinePage()
                )
            );

            return;
        }


        await mainWindow.loadFile(
            result.path
        );


        if (pendingCSF) {

            mainWindow.webContents.send(
                "open-csf",
                pendingCSF
            );

            pendingCSF = null;

        }

    }


    app.whenReady().then(() => {

        Menu.setApplicationMenu(null);


        pendingCSF =
            getCSFPath(
                process.argv
            );


        createWindow().catch(error => {

            console.error(
                "Failed to create Ceedoku:",
                error
            );

            app.quit();

        });


        app.on("activate", () => {

            if (
                BrowserWindow.getAllWindows().length === 0
            ) {

                createWindow();

            }

        });

    });


    app.on("window-all-closed", () => {

        if (process.platform !== "darwin") {
            app.quit();
        }

    });

}
