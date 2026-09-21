# Ceedoku Client

The desktop client for **Ceedoku**.

Ceedoku Client packages the Ceedoku web game into a lightweight desktop application with local caching and offline fallback support.

## Features

* **Desktop Ceedoku** — Run Ceedoku as a standalone application.
* **Automatic updates** — The client downloads the latest version of Ceedoku when it starts.
* **Local caching** — A working copy of Ceedoku is stored locally for faster loading.
* **Offline fallback** — If an update cannot be downloaded, the client can continue using the last cached version.
* **CSF support** — Open Ceedoku Save Format (`.csf`) files directly with the client.
* **Single instance** — Opening the client again while it is already running forwards supported arguments to the existing window.

## How it works

Ceedoku Client downloads the files used by the Ceedoku website and stores them in its local application cache.

On startup, the client:

1. Checks for a newer version of Ceedoku.
2. Downloads the required website files.
3. Places the downloaded files into a temporary update directory.
4. Replaces the previous cached version with the new version.
5. Loads the cached Ceedoku files in Electron.

If the update fails but a previous cached version exists, the client uses that cached version instead.

If no cached version exists, the client displays an offline page.

## Project structure

The client is an Electron application. Its main process is responsible for:

* Creating the application window
* Downloading and caching Ceedoku
* Handling redirects
* Resolving Ceedoku resources
* Opening `.csf` files
* Managing the application's single-instance lock
* Providing an offline fallback

## Requirements

* [Node.js](https://nodejs.org/)
* npm
* Electron

## Development

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

`https://ceedoku.github.io/`

Only files hosted by the Ceedoku website are accepted by the cache downloader. This keeps the client tied to the official Ceedoku deployment rather than arbitrary external resources.

The client downloads the files required by Ceedoku, including the puzzle worker at:

```text
src/js/puzzle-worker.js
```

## Offline mode

Ceedoku Client is designed to keep a previously downloaded version available.

If an update fails:

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

See [`LICENCE`] for licence information.

---

**Ceedoku** is free, open-source, and made by **Ceebug**.
