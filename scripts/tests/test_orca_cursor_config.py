"""Optional offline compatibility check against the installed Cursor bundle.

Never invoke its CLI main, authentication flow, or a model. Private bundle
exports deliberately fail closed when a Cursor update changes their shape.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.orca_providers import cursor_permissions
from scripts.tests import test_orca_roles as fixtures


LOADER_PROBE = r'''
const fs = require("node:fs");
const assert = require("node:assert/strict");
const { Module } = require("node:module");
const path = require("node:path");
(async () => {
  const [filename, expectedPath, expectedFallback] = process.argv.slice(1);
  const source = fs.readFileSync(filename, "utf8");
  const entry = 'var __webpack_exports__=__webpack_require__("./src/main.tsx")';
  assert(source.endsWith(entry + "})();"), "Cursor bootstrap changed; re-audit first");
  const isolated = new Module(filename, module);
  isolated.filename = filename;
  isolated.paths = Module._nodeModulePaths(path.dirname(filename));
  isolated._compile(source.slice(0, source.lastIndexOf(entry)) +
    "module.exports=__webpack_require__})();", filename);
  if (expectedFallback === "session") {
    const kv = isolated.exports("../agent-kv/dist/index.js");
    const serializer = new kv.aY();
    const id = "e1fd2684-d55a-4794-9741-903c92b7dbea";
    const serialized = kv.nj(serializer.serialize({agentId: id, latestRootBlobId: new Uint8Array([1, 2])}));
    const metadata = JSON.parse(Buffer.from(serialized, "hex").toString("utf8"));
    assert.equal(metadata.agentId, id);
    assert.equal(metadata.latestRootBlobId, "0102");
    await isolated.exports.e(8176);
    const state = isolated.exports("./src/state/index.ts");
    const crypto = require("node:crypto");
    assert.equal(state.wk("/fixture"), path.join(expectedPath, "chats",
      crypto.createHash("md5").update("/fixture").digest("hex")));
    console.log("session metadata contract: pass");
    return;
  }
  const config = isolated.exports("../cursor-config/dist/index.js");
  if (expectedFallback === "project") {
    const project = path.join(process.cwd(), ".cursor", "cli.json");
    const original = fs.readFileSync(project, "utf8");
    const permissions = JSON.parse(original).permissions;
    const provider = await config.FO.loadFromDefaults(null, {
      onError: (_code, message) => { throw Error(message); },
    });
    assert.equal(provider.getConfigFilePath(), expectedPath);
    assert.deepStrictEqual(provider.get().permissions, permissions);
    await provider.transform(value => ({...value, hints: false,
      permissions: {allow: ["Shell(*)", "Write(**)"], deny: []}}));
    assert.deepStrictEqual(provider.get().permissions, permissions);
    assert.deepStrictEqual((await provider.reload()).permissions, permissions);
    assert.deepStrictEqual(JSON.parse(fs.readFileSync(expectedPath, "utf8")).permissions.deny, []);
    assert.equal(fs.readFileSync(project, "utf8"), original);
    assert.throws(() => fs.writeFileSync(project, "{}"), /EROFS|EACCES/);
    console.log("mutable metadata + immutable project permissions: pass");
    return;
  }
  const messages = [];
  const provider = await config.FO.loadFromDefaults(null, {
    skipGitRootDetection: true,
    disableProjectConfigs: true,
    onDebugLog: message => messages.push(String(message)),
    onError: (_code, message) => { throw Error(message); },
  });
  assert.equal(provider.getConfigFilePath(), expectedPath);
  const raw = JSON.parse(fs.readFileSync(expectedPath, "utf8"));
  const fallback = messages.some(message => message.includes("backing up and regenerating"));
  assert.equal(fallback, expectedFallback === "true");
  assert.deepStrictEqual(provider.get().permissions.deny,
    fallback ? [] : raw.permissions.deny);
  console.log(JSON.stringify({fallback, deny: provider.get().permissions.deny}));
})().catch(error => { console.error(error.message); process.exitCode = 1; });
'''


@unittest.skipUnless(sys.platform == "linux" and all(shutil.which(name) for name in
                     ("bwrap", "node", "cursor-agent")), "installed Cursor, Node and Linux bubblewrap required")
class CursorConfigCompatibilityTests(unittest.TestCase):
    run_git = staticmethod(fixtures.OrcaRoleTests.run_git)

    def test_project_policy_survives_global_rewrite_in_linked_worktree(self) -> None:
        fixtures.OrcaRoleTests.setUp(self)
        bundle = Path(shutil.which("cursor-agent")).resolve().parent / "index.js"
        project = self.repo / ".cursor/cli.json"
        project.parent.mkdir()
        global_dir = self.root / "cursor-global"
        global_dir.mkdir()
        global_config = global_dir / "cli-config.json"
        for readonly in (False, True):
            with self.subTest(readonly=readonly):
                config = cursor_permissions({"allowed_directories": ["src"], "read_only": readonly})
                project.write_text(json.dumps({"permissions": config["permissions"]}))
                global_config.write_text(json.dumps(config))
                result = subprocess.run([
                    shutil.which("bwrap"), "--die-with-parent", "--unshare-net", "--unshare-pid",
                    "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
                    "--bind", str(global_dir), str(global_dir), "--clearenv",
                    "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "HOME", "/tmp",
                    "--setenv", "CURSOR_CONFIG_DIR", str(global_dir), "--chdir", str(self.repo),
                    "--", shutil.which("node"), "-e", LOADER_PROBE, str(bundle), str(global_config), "project",
                ], text=True, capture_output=True, timeout=20, umask=0o077)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("immutable project permissions: pass", result.stdout)

    def test_installed_session_workspace_key_and_metadata_encoding(self) -> None:
        bundle = Path(shutil.which("cursor-agent")).resolve().parent / "index.js"
        result = subprocess.run([
            shutil.which("bwrap"), "--die-with-parent", "--unshare-net", "--unshare-pid",
            "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
            "--clearenv", "--setenv", "HOME", "/tmp", "--setenv", "CURSOR_CONFIG_DIR", "/tmp/fixture",
            "--", shutil.which("node"), "-e", LOADER_PROBE, str(bundle), "/tmp/fixture", "session",
        ], text=True, capture_output=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("session metadata contract: pass", result.stdout)

    def test_real_loader_retains_read_only_policy_without_default_fallback(self) -> None:
        bundle = Path(shutil.which("cursor-agent")).resolve().parent / "index.js"
        self.assertTrue(bundle.is_file(), "Cursor layout changed; re-audit first")
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=target) as directory:
            fixture = Path(directory) / "cli-config.json"
            complete = cursor_permissions({"allowed_directories": ["crates/hw_ui/src/interaction/help"]})
            readonly = cursor_permissions({"allowed_directories": [], "read_only": True})
            for config, fallback in (({"permissions": complete["permissions"]}, True),
                                     (complete, False), (readonly, False)):
                with self.subTest(fallback=fallback):
                    fixture.write_text(json.dumps(config))
                    result = subprocess.run([
                        shutil.which("bwrap"), "--die-with-parent", "--unshare-net", "--unshare-pid",
                        "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
                        "--clearenv", "--setenv", "HOME", "/tmp",
                        "--setenv", "CURSOR_CONFIG_DIR", directory,
                        "--", shutil.which("node"), "-e", LOADER_PROBE, str(bundle), str(fixture),
                        str(fallback).lower(),
                    ], text=True, capture_output=True, timeout=20)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    output = json.loads(result.stdout)
                    self.assertEqual(output["fallback"], fallback)
                    self.assertEqual(output["deny"], [] if fallback else config["permissions"]["deny"])


if __name__ == "__main__":
    unittest.main()
