// Runs the Tauri CLI with rustup's cargo first on PATH, and boots the iOS
// simulator named in `ios dev` or `ios run` first.
//
// rust-toolchain.toml only takes effect through rustup. When another Rust
// (asdf, Homebrew) comes earlier on PATH, mobile builds fail with "can't
// find crate for `core`", because that toolchain has no iOS or Android
// standard library. Tauri hands the PATH it was started with to the Xcode
// and Gradle build steps, so it has to be fixed here, before Tauri starts.

import { execFileSync, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";

const cargoBin = join(
  process.env.CARGO_HOME ?? join(homedir(), ".cargo"),
  "bin",
);
const env = { ...process.env };
// Windows spells it Path; keep whichever name is present.
const pathKey =
  Object.keys(env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
if (existsSync(cargoBin)) {
  env[pathKey] = [cargoBin, env[pathKey]].filter(Boolean).join(delimiter);
}

/**
 * Tauri installs the app on the simulator without booting it, so on one that
 * is shut down the install fails ("Unable to lookup in current state:
 * Shutdown") and the Simulator keeps showing the last app it ran.
 */
function bootNamedSimulator(args) {
  if (process.platform !== "darwin" || args[0] !== "ios") return;
  if (args[1] !== "dev" && args[1] !== "run") return;
  let devices;
  try {
    const json = execFileSync(
      "xcrun",
      ["simctl", "list", "devices", "available", "--json"],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      },
    );
    devices = Object.values(JSON.parse(json).devices).flat();
  } catch {
    return; // No Xcode tools; let Tauri report it.
  }
  // The device is a positional argument; only an exact simulator name or
  // UDID counts, so option values are never mistaken for one.
  const wanted = args.slice(2).filter((a) => !a.startsWith("-"));
  const device = devices.find(
    (d) => wanted.includes(d.name) || wanted.includes(d.udid),
  );
  if (!device) {
    if (!devices.some((d) => d.state === "Booted")) {
      console.warn(
        "No iOS simulator is running. Name one to have it started first, e.g.\n" +
          '  npm run tauri ios dev "iPhone 17 Pro"\n' +
          "or start it in the Simulator app; Tauri cannot install on a simulator that is shut down.",
      );
    }
    return;
  }
  if (device.state !== "Booted") {
    console.log(`Starting the ${device.name} simulator…`);
    // bootstatus -b boots and waits until the simulator can launch apps;
    // `simctl boot` returns earlier, and launching then can fail with "The
    // system shell probably crashed".
    execFileSync("xcrun", ["simctl", "bootstatus", device.udid, "-b"], {
      stdio: "ignore",
    });
  }
  // Bring the Simulator window up so the app can be seen.
  try {
    execFileSync("open", ["-a", "Simulator"], { stdio: "ignore" });
  } catch {
    // Not fatal: the app still installs and runs.
  }
}

bootNamedSimulator(process.argv.slice(2));

const cli = createRequire(import.meta.url).resolve("@tauri-apps/cli/tauri.js");
const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], {
  env,
  stdio: "inherit",
});
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
