"""Read-only safe-Blender entrypoint: validate actual retained packed source IDs."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

import bpy

sys.path.insert(0,str(Path(__file__).resolve().parent))
from building_handpaint_materials import check_packed_sources, source_snapshot
from building_clay_geometry import require


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--blend',type=Path,required=True)
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:])
    require(args.blend.is_absolute() and args.blend==args.blend.resolve()
            and Path(bpy.data.filepath)==args.blend,"readback opened a different blend")
    snapshot=source_snapshot()
    check_packed_sources(bpy.data.images,bpy.data.texts,snapshot)
    result={'contract':'building-handpaint-saved-sources-v1',
            'blend_sha256':hashlib.sha256(args.blend.read_bytes()).hexdigest(),'sources':snapshot}
    print('HW_HANDPAINT_BLEND_JSON:'+json.dumps(result,sort_keys=True))


if __name__=='__main__':main()
