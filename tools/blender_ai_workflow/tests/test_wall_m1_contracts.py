from __future__ import annotations

import importlib.util
import json
import struct
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/validate_wall_glb.py"
CONTRACT_PATH = WORKFLOW_ROOT / "fixtures/wall-production-v1.geometry.json"


def load_validator():
    spec = importlib.util.spec_from_file_location("validate_wall_glb", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load validate_wall_glb.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def box_geometry(
    half_x: float, half_z: float, half_y: float = 16.0
) -> tuple[list[tuple[float, float, float]], list[int]]:
    positions = [
        (x, y, z)
        for x, y, z in (
            (-half_x, -half_y, -half_z),
            (half_x, -half_y, -half_z),
            (half_x, half_y, -half_z),
            (-half_x, half_y, -half_z),
            (-half_x, -half_y, half_z),
            (half_x, -half_y, half_z),
            (half_x, half_y, half_z),
            (-half_x, half_y, half_z),
        )
    ]
    indices = [
        0,
        2,
        1,
        0,
        3,
        2,
        4,
        5,
        6,
        4,
        6,
        7,
        0,
        1,
        5,
        0,
        5,
        4,
        3,
        7,
        6,
        3,
        6,
        2,
        0,
        4,
        7,
        0,
        7,
        3,
        1,
        2,
        6,
        1,
        6,
        5,
    ]
    return positions, indices


def fixture_document(
    half_x: float, half_z: float, half_y: float = 16.0
) -> tuple[dict[str, object], bytes]:
    positions, indices = box_geometry(half_x, half_z, half_y)
    position_bytes = b"".join(struct.pack("<3f", *position) for position in positions)
    normal_bytes = b"".join(struct.pack("<3f", 0.0, 1.0, 0.0) for _ in positions)
    uv_bytes = b"".join(struct.pack("<2f", 0.0, 0.0) for _ in positions)
    index_bytes = b"".join(struct.pack("<H", index) for index in indices)
    chunks = [position_bytes, normal_bytes, uv_bytes, index_bytes]
    offsets: list[int] = []
    binary = b""
    for chunk in chunks:
        offsets.append(len(binary))
        binary += chunk
        binary += b"\0" * ((-len(binary)) % 4)
    document: dict[str, object] = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [
            {
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
                        "indices": 3,
                        "mode": 4,
                    }
                ]
            }
        ],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": offsets[index], "byteLength": len(chunk)}
            for index, chunk in enumerate(chunks)
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 8, "type": "VEC3"},
            {"bufferView": 1, "componentType": 5126, "count": 8, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5126, "count": 8, "type": "VEC2"},
            {
                "bufferView": 3,
                "componentType": 5123,
                "count": len(indices),
                "type": "SCALAR",
            },
        ],
    }
    return document, binary


def write_glb(path: Path, document: dict[str, object], binary: bytes) -> None:
    json_bytes = json.dumps(document, separators=(",", ":")).encode("utf-8")
    json_bytes += b" " * ((-len(json_bytes)) % 4)
    binary += b"\0" * ((-len(binary)) % 4)
    length = 12 + 8 + len(json_bytes) + 8 + len(binary)
    path.write_bytes(
        struct.pack("<4sII", b"glTF", 2, length)
        + struct.pack("<II", len(json_bytes), 0x4E4F534A)
        + json_bytes
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )


class WallPostExportContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = load_validator()

    def validate_fixture(
        self,
        family: str,
        half_x: float,
        half_z: float,
        half_y: float = 16.0,
        mutate=None,
    ):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "wall.glb"
            document, binary = fixture_document(half_x, half_z, half_y)
            if mutate is not None:
                mutate(document)
            write_glb(path, document, binary)
            return self.validator.validate_wall_glb(path, family, CONTRACT_PATH)

    def test_isolated_box_passes_two_axis_profile(self) -> None:
        report = self.validate_fixture("isolated", 4.8, 4.8)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["triangle_count"], 12)
        self.assertFalse(report["tangent_present"])
        self.assertEqual(len(report["cross_sections"]), 2)

    def test_straight_box_passes_both_ports(self) -> None:
        report = self.validate_fixture("straight", 4.8, 16.0)
        self.assertEqual(report["arms"], ["N", "S"])
        self.assertEqual(len(report["cross_sections"]), 12)

    def test_wrong_port_width_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            self.validator.ContractError, "core hole|port .* width differs"
        ):
            self.validate_fixture("straight", 4.0, 16.0)

    def test_non_identity_node_is_rejected(self) -> None:
        def mutate(document):
            document["nodes"][0]["translation"] = [1.0, 0.0, 0.0]

        with self.assertRaisesRegex(self.validator.ContractError, "node transform"):
            self.validate_fixture("isolated", 4.8, 4.8, mutate=mutate)

    def test_wrong_vertical_bounds_are_rejected(self) -> None:
        with self.assertRaisesRegex(self.validator.ContractError, "vertical bounds"):
            self.validate_fixture("isolated", 4.8, 4.8, half_y=15.0)

    def test_second_primitive_is_rejected(self) -> None:
        def mutate(document):
            document["meshes"][0]["primitives"].append(
                dict(document["meshes"][0]["primitives"][0])
            )

        with self.assertRaisesRegex(self.validator.ContractError, "one primitive"):
            self.validate_fixture("isolated", 4.8, 4.8, mutate=mutate)

    def test_embedded_image_is_rejected(self) -> None:
        def mutate(document):
            document["images"] = [{"uri": "data:image/png;base64,AA=="}]

        with self.assertRaisesRegex(self.validator.ContractError, "embedded image"):
            self.validate_fixture("isolated", 4.8, 4.8, mutate=mutate)

    def test_missing_external_image_is_rejected(self) -> None:
        def mutate(document):
            document["images"] = [{"uri": "missing.png"}]

        with self.assertRaisesRegex(self.validator.ContractError, "external image is missing"):
            self.validate_fixture("isolated", 4.8, 4.8, mutate=mutate)

    def test_missing_uv0_is_rejected(self) -> None:
        def mutate(document):
            del document["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"]

        with self.assertRaisesRegex(self.validator.ContractError, "requires POSITION"):
            self.validate_fixture("isolated", 4.8, 4.8, mutate=mutate)


if __name__ == "__main__":
    unittest.main()
