const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const root = path.join(__dirname, "..");
const binaryName =
    process.platform === "win32"
        ? "ceedoku-updater.exe"
        : "ceedoku-updater";
const binaryPath = path.join(root, "updater-bin", binaryName);
const sourcePaths = [
    path.join(root, "updater", "Cargo.toml"),
    path.join(root, "updater", "Cargo.lock"),
    path.join(root, "updater", "src", "main.rs")
];
const binaryExists = fs.existsSync(binaryPath);
const binaryTime = binaryExists
    ? fs.statSync(binaryPath).mtimeMs
    : 0;
const needsBuild = !binaryExists || sourcePaths.some(sourcePath =>
    fs.existsSync(sourcePath) &&
    fs.statSync(sourcePath).mtimeMs > binaryTime
);

if (needsBuild) {
    execFileSync(
        "cargo",
        ["build", "--manifest-path", "updater/Cargo.toml", "--release"],
        { cwd: root, stdio: "inherit" }
    );

    const builtPath = path.join(
        root,
        "updater",
        "target",
        "release",
        binaryName
    );

    fs.mkdirSync(path.dirname(binaryPath), { recursive: true });
    fs.copyFileSync(builtPath, binaryPath);
}
