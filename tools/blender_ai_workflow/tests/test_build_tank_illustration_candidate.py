"""Image import and non-launching data-boundary tests, not art approval."""

import io
from contextlib import contextmanager, ExitStack
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import build_tank_illustration_candidate as source


def png(image):
    out = io.BytesIO()
    image.save(out, format="PNG")
    return out.getvalue()


class TankIllustrationSourceTests(unittest.TestCase):
    def test_preview_keeps_geometry_alpha_and_discards_old_render_rgb(self):
        art = png(Image.new("RGBA", (4, 4), (80, 60, 40, 255)))
        guide = Image.new("RGBA", (256, 256), (255, 0, 255, 0))
        guide.putpixel((128, 128), (255, 0, 255, 128))
        before = png(guide)
        rendered = source.preview(art, before)
        with Image.open(io.BytesIO(rendered)) as result:
            self.assertEqual(result.mode, "RGBA")
            self.assertEqual(
                result.getchannel("A").tobytes(), guide.getchannel("A").tobytes()
            )
            self.assertEqual(result.getpixel((128, 128)), (80, 60, 40, 128))
            self.assertEqual(result.getpixel((0, 0)), (80, 60, 40, 0))
        self.assertEqual(png(guide), before)

    def test_missing_rgba_wrong_mask_extent_and_oversized_art_are_rejected(self):
        guide = png(Image.new("RGBA", (256, 256)))
        for image in (Image.new("RGB", (4, 4)), Image.new("RGBA", (2049, 1))):
            with self.assertRaises(ValueError):
                source.preview(png(image), guide)
        with self.assertRaises(ValueError):
            source.preview(
                png(Image.new("RGBA", (4, 4))), png(Image.new("RGBA", (255, 256)))
            )

    def test_report_cannot_select_an_executable_or_claim_art_approval(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            trusted = root / "declared-blender"
            trusted.write_bytes(b"explicit executable fixture; never executed")
            report = {
                "schema_version": 1,
                "scope": "unapproved-tank-world-preview-source",
                "generation": 4,
                "base_bundle": str(root / "base"),
                "blender": "/bin/sh",
                "blender_chain": {},
                "source_audit": {},
                "producer_sha256": "a" * 64,
                "packer_sha256": "a" * 64,
                "files": {},
                "art_approved": False,
                "numeric_freeze": False,
                "runtime_published": False,
            }
            path = root / "illustration-source.json"
            path.write_bytes(source.pipeline.canonical(report))
            with (
                patch.object(source, "packer") as command,
                self.assertRaises(ValueError),
            ):
                source.verify(root, trusted, trusted, "a" * 64)
            command.assert_not_called()
            report["art_approved"] = True
            path.write_bytes(source.pipeline.canonical(report))
            with (
                patch.object(source, "packer") as command,
                self.assertRaises(ValueError),
            ):
                source.verify(root, trusted, trusted, "a" * 64)
            command.assert_not_called()

    @contextmanager
    def prepared(self):
        with tempfile.TemporaryDirectory() as temp, ExitStack() as stack:
            folder = Path(temp)
            base = folder / "base"
            base.mkdir()
            for name in source.BASE_FILES:
                (base / name).write_bytes(
                    png(Image.new("RGBA", (256, 256), (80, 60, 40, 255)))
                    if name == "Empty.png"
                    else b"immutable fixture " + name.encode()
                )
            pinned = {
                p.name: source.pipeline.digest(p.read_bytes()) for p in base.iterdir()
            }
            stack.enter_context(patch.object(source, "BASE_FILES", pinned))
            stack.enter_context(patch.object(source, "asset_root", return_value=folder))
            for symbol in ("PACKER", "__file__"):
                path = folder / (symbol + ".py")
                path.write_bytes(b"frozen test tool")
                stack.enter_context(patch.object(source, symbol, path))
            chain = {"files": {}, "application": {"commit": "a" * 64}}
            stack.enter_context(
                patch.object(source, "blender_chain", return_value=chain)
            )
            blender = folder / "trusted-blender"
            blender.write_bytes(b"never execute in tests")
            paint = folder / "paint.png"
            paint.write_bytes(png(Image.new("RGBA", (4, 4), (70, 50, 30, 255))))
            args = SimpleNamespace(
                output=folder / "staging/exports/fresh",
                base_bundle=base,
                generation=4,
                paint=paint,
                blender=blender,
                flatpak=blender,
            )

            def pack(_blender, _flatpak, action, _base, blend, _config):
                if action == "pack":
                    blend.write_bytes(b"packed source fixture")
                return {
                    "geometry_sha256": "b" * 64,
                    "source_only": True,
                    "art_approved": False,
                }

            with patch.object(source, "packer", side_effect=pack):
                self.assertEqual(
                    source.prepare(args)["status"], "verified-unapproved-source"
                )
            yield args, chain, pack

    def test_audit_rejects_source_recipe_base_tool_and_deployment_mutation(self):
        for target in (
            "paint",
            "recipe",
            "report",
            "base",
            "packer",
            "producer",
            "deployment",
        ):
            with self.subTest(target=target), self.prepared() as (args, chain, pack):
                files = {
                    "paint": args.output / "authoring/Empty.png",
                    "recipe": args.output / "recipe.json",
                    "report": args.output / "illustration-source.json",
                    "base": args.base_bundle / "water.glb",
                    "packer": source.PACKER,
                    "producer": Path(source.__file__),
                }

                def mutate(*argv):
                    audit = pack(*argv)
                    if target == "deployment":
                        chain["application"]["commit"] = "c" * 64
                    else:
                        files[target].write_bytes(b"changed during external audit")
                    return audit

                with (
                    patch.object(source, "packer", side_effect=mutate),
                    self.assertRaisesRegex(ValueError, "changed during audit"),
                ):
                    source.verify(
                        args.output,
                        args.blender,
                        args.flatpak,
                        source.pipeline.digest(args.paint.read_bytes()),
                    )

    def test_blender_environment_disables_network_and_shell_startup_injection(self):
        with patch.dict(
            source.os.environ,
            {
                "BASH_ENV": "/untrusted",
                "ENV": "/untrusted",
                "SHELLOPTS": "xtrace",
                "BASH_FUNC_flatpak%%": "() { arbitrary-command; }",
            },
        ):
            environment = source.blender_environment(Path("/usr/bin/flatpak"))
        self.assertFalse(
            set(environment) & {"BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS"}
        )
        self.assertEqual(environment["BLENDER_SAFE_NO_NETWORK"], "1")
        self.assertFalse(any(key.startswith("BASH_FUNC_") for key in environment))
        self.assertEqual(environment["PATH"].split(source.os.pathsep)[0], "/usr/bin")

    def test_safe_launcher_cannot_pin_a_different_flatpak_basename(self):
        with (
            patch.object(source.subprocess, "run") as command,
            self.assertRaisesRegex(ValueError, "resolve the declared"),
        ):
            source.blender_chain(source.SAFE_BLENDER, Path("/usr/bin/bash"))
        command.assert_not_called()


if __name__ == "__main__":
    unittest.main()
