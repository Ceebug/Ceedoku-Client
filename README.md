# Ceedoku Client

The desktop client for **Ceedoku**.

Ceedoku Client packages the Ceedoku web game into a lightweight desktop application with local caching and offline fallback support.

## Features

- **Desktop Ceedoku** — Run Ceedoku as a standalone application.
- **Automatic updates** — A separate native updater checks for Ceedoku Client releases and refreshes the Ceedoku game files.
- **Offline fallback** — If an update cannot be downloaded, the client can continue using the last cached version.
- **`.CSF` support** — Open Ceedoku Save Format (`.csf`) files directly with the client.
- **Single instance** — Opening the client again while it is already running forwards supported arguments to the existing window.

## How it works

The standalone native updater opens before the game and displays the same update window while checking for a newer Ceedoku Client release and updating the Ceedoku game files. Client installers from new version tags are published as GitHub Releases. After updating the client, the updater starts the updated client to refresh the game cache, then opens the game.

On startup, the client:

1. Shows the standalone native update window.
2. Checks GitHub Releases for a newer client version and installs the matching platform update when available.
3. Starts a hidden client process to download the current website files.
4. Opens the game using the cached files.

If the website update fails but a previous cached version exists, the client uses that cached version instead. The native updater remains visible during both kinds of updates.

If no cached version exists, the client displays an offline page.

In development, the Electron client uses the local `assets/loading.html` loading screen. Installed releases use the native updater window during startup instead.

## Project structure

The client is an Electron application. Its main process is responsible for:

* Creating the application window
* Downloading and caching Ceedoku
* Handling redirects
* Resolving Ceedoku resources
* Opening `.csf` files
* Managing the application's single-instance lock
* Providing an offline fallback

The native updater source is in `updater/`. It is compiled separately for each release platform and packaged outside the Electron application bundle so it can remain running while a client update is installed.

Push a version tag matching `package.json` (for example, `v1.2.0`) to build installers and publish them as GitHub Release assets for automatic client updates.

## Development

### Requirements

- [Node.js](https://nodejs.org/)
- [npm](https://www.npmjs.com/)
- [Electron](https://www.electronjs.org/)
- [Rust and Cargo](https://www.rust-lang.org/tools/install) (for building the native updater)

Clone the repository:

```bash
git clone https://github.com/ceebug/ceedoku-client.git
cd ceedoku-client
```

Install dependencies:

```bash
npm install
```

Start the client:

```bash
npm start
```

## Updating Ceedoku

The client pulls its game files from the Ceedoku website:

[`https://ceedoku.github.io`](https://ceedoku.github.io/)

Only files hosted by the Ceedoku website are accepted by the cache downloader. This keeps the client tied to the official Ceedoku deployment rather than arbitrary external resources.

The client downloads the files required by Ceedoku, including the puzzle worker at:

```text
src/js/puzzle-worker.js
```

## Offline mode

Ceedoku Client is designed to keep a previously downloaded version available.

Error handling flow

```text
Latest version
      ↓
 Download fails
      ↓
Cached version exists?
   ↙           ↘
 Yes            No
  ↓              ↓
Load cache    Offline page
```

This means a temporary network or server problem does not necessarily prevent Ceedoku from launching.

## Ceedoku Save Format

Ceedoku Client supports opening **Ceedoku Save Format (CSF)** files.

CSF files can be passed to the application when launching it, allowing saved Ceedoku games to be opened directly in the client.

For more information about the format, see the [Ceedoku CSF specification](https://ceedoku.github.io/csfspec/).

## License

See the [`LICENSE`](./LICENSE) file for licence information.

---

**Ceedoku** is free, open-source, and made by **Ceebug**.
