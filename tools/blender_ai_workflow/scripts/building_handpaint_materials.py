"""Opaque hand-painted material sources baked into physical, object-local UVs.

These bitmaps provide pigment/ink, never view-dependent images of another part.
Geometry still defines holes, silhouettes, depth, closure and moving surfaces.
"""
from __future__ import annotations

import hashlib
import math
from pathlib import Path

from building_clay_geometry import FIXTURES, read_json, require

STYLE = "handpaint-v1"
SOURCE_ROOT = FIXTURES / "handpaint-materials-20261115-v1"
SOURCE_MANIFEST_SHA256 = "3eb8c2cacd85475d34e21b614f031b63b0274387db873969206999040c691ed9"


def source_snapshot():
    manifest = SOURCE_ROOT / "materials.json"
    require(not SOURCE_ROOT.is_symlink() and not manifest.is_symlink(), "paint source symlink")
    require(hashlib.sha256(manifest.read_bytes()).hexdigest()==SOURCE_MANIFEST_SHA256, "paint source manifest changed")
    value=read_json(manifest)
    require(type(value["schema_version"]) is int and value["schema_version"]==1
            and value["scope"]=="unapproved_handpaint_material_inputs"
            and all(value[key] is False for key in ("art_approved","numeric_approved","release_approved")), "paint source cannot grant authority")
    records={}
    for row in value["images"]:
        name=row["name"]
        require(name in {"drum","iron","canvas"} and row["path"]==name+".png" and name not in records, "unknown/duplicate paint source")
        path=SOURCE_ROOT/row["path"]
        require(path.is_file() and not path.is_symlink() and path.stat().st_size==row["bytes"]
                and hashlib.sha256(path.read_bytes()).hexdigest()==row["sha256"], "paint source bytes changed")
        require(len(row["size"])==2 and all(type(n) is int and 16<=n<=2048 for n in row["size"]), "paint source size is not bounded")
        records[name]={key:row[key] for key in ("path","bytes","sha256","size")}
    require(set(records)=={"drum","iron","canvas"}, "paint source set incomplete")
    return {"contract":"building-handpaint-materials-v1","manifest_sha256":SOURCE_MANIFEST_SHA256,"images":records}


def source_paths():
    snapshot=source_snapshot()
    return {"materials.json":SOURCE_ROOT/"materials.json",
            **{row["path"]:SOURCE_ROOT/row["path"] for row in snapshot["images"].values()}}


def load_sources():
    from PIL import Image
    snapshot=source_snapshot()
    images={}
    for name,row in snapshot["images"].items():
        with Image.open(SOURCE_ROOT/row["path"]) as image:
            require(image.format=="PNG" and image.mode in ("RGB","RGBA") and list(image.size)==row["size"], "paint source decode differs")
            require(image.mode=="RGB" or image.getchannel("A").getextrema()==(255,255), "material pigment cannot create alpha holes")
            images[name]=image.convert("RGB")
    require(source_snapshot()==snapshot,"paint inputs changed during decode")
    return images,snapshot


def sample(image,u,v,*,wrap=False):
    require(math.isfinite(u) and math.isfinite(v),"nonfinite material coordinate")
    # Repeat is an explicit cylindrical material domain, not a hidden fill for
    # missing multiview/geometry evidence. Gutter samples extend the same field.
    x=(u%1 if wrap else max(0,min(1,u)))*(image.width-1)
    y=max(0,min(1,v))*(image.height-1)
    x0,y0=math.floor(x),math.floor(y)
    x1,y1=min(x0+1,image.width-1),min(y0+1,image.height-1)
    tx,ty=x-x0,y-y0
    a,b,c,d=(image.getpixel(p) for p in ((x0,y0),(x1,y0),(x0,y1),(x1,y1)))
    return tuple(round((1-ty)*((1-tx)*a[i]+tx*b[i])+ty*((1-tx)*c[i]+tx*d[i])) for i in range(3))


def mixer_color(role,surface,point,spec,images):
    x,y,z=point;label,side=surface.split(":",1)
    ink=(31,26,31)
    if role=="rotor" and label=="drum":
        band=int(side.removeprefix("band"));angle=math.atan2(z,x);u=(angle+math.pi)/math.tau
        if band>=5:
            return (*(round(c*.52) for c in sample(images["iron"],u,(y+16)/30,wrap=True)),255)
        if band==4:
            return (*(sample(images["iron"],u,.2,wrap=True) if math.sin(angle+.3)>.7 else ink),255)
        base=sample(images["drum"],u,(y+16)/30,wrap=True)
        # One physical sheet-metal weld explicitly closes the cylindrical wrap.
        if min(u,1-u)<.007 or abs(y+3.6+.15*math.sin(angle*5))<.45:
            base=ink
        return (*base,255)
    if role=="rotor" and label=="vane":
        return (*sample(images["iron"],(x+8)/16,(y+3)/16),255)
    if role=="body" and label=="wheel":
        radius=math.hypot(y-6,z-9)
        base=sample(images["iron"],(y/12)%1,(z/12)%1)
        if radius>4.8 or radius<1.1:base=ink
        return (*base,255)
    require(role=="body" and label.startswith("frame") and label[5:].isdigit(),"unknown handpainted Mixer member")
    index=int(label[5:]);boxes=spec["construction"]["body_boxes"]
    require(index<len(boxes),"unknown frame index")
    box=boxes[index];local=[p-c for p,c in zip(point,box["center"],strict=True)];half=[s/2 for s in box["size"]]
    axes={"top":(0,2),"bottom":(0,2),"front":(0,1),"back":(0,1),"left":(1,2),"right":(1,2)}[side]
    a,b=axes;base=sample(images["iron"],.5+local[a]/max(8,box["size"][a]),.5+local[b]/max(24,box["size"][b]))
    edge=min(half[a]-abs(local[a]),half[b]-abs(local[b]))
    # Ink belongs to the physical steel member's perimeter, never UV cells or
    # generated triangulation edges. A broad irregular stroke survives zoom.
    if edge<.5+.1*math.sin(local[a]*.7+local[b]*.3):base=ink
    return (*base,255)


def interpolate(x,xs,values):
    i=next((i for i in range(len(xs)-1) if xs[i]<=x<=xs[i+1]),0 if x<xs[0] else len(xs)-2)
    t=max(0,min(1,(x-xs[i])/(xs[i+1]-xs[i])))
    return values[i]+t*(values[i+1]-values[i])


def rest_color(surface,point,spec,images):
    x,y,z=point;material=surface.split(":",1)[0];ink=(38,29,33)
    if surface=="cloth:canopy":
        base=sample(images["canvas"],(x+31)/62,(z+29)/58)
        front=interpolate(x,spec["roof"]["x_profile"],spec["roof"]["front_z_profile"])
        if min(x+31,31-x,z+29,front-z)<.65+.12*math.sin(x*.4):base=ink
    elif surface.startswith("cloth:curtain"):
        base=sample(images["canvas"],.04+.2*(x+8)/16,.3+.25*y/26)
        hem=interpolate(x,spec["curtain"]["x_profile"],spec["curtain"]["bottom_y_profile"])
        if abs(x-.3*math.sin(y*.3))<.5 or abs(x)>7.3 or y<hem+.6:base=ink
        elif abs(abs(x)-4)<.25:base=tuple(round(c*.75) for c in base)
    elif material=="cloth":
        base=sample(images["canvas"],.05+.2*((x+z+50)/100),.3+.25*y/26)
    elif material=="wood":
        base=tuple(round(c*m) for c,m in zip(sample(images["iron"],(x+z+64)/128,y/48),(1.13,.95,.86),strict=True))
        if abs(math.sin(x*1.2+z*.9+.12*math.sin(y*.2)))<.15:base=ink
    elif material=="iron":
        base=sample(images["iron"],(x+32)/64,(z+32)/64)
    else:raise ValueError("unknown handpainted shelter surface")
    return (*base,255)


def paint(kind,path,stage,revision):
    from PIL import Image
    from building_mixer_uv_ink import local_point
    require(stage=="strokes" and ((kind=="MudMixer" and revision=="v5") or (kind=="RestArea" and revision=="v4")),"handpaint requires explicit new geometry/stroke stage")
    if kind=="MudMixer":
        from building_m2_art import candidate_geometry,face_layout
        from building_clay_geometry import load_pilot
        meshes=candidate_geometry(kind,revision);layout=face_layout(kind,revision);spec=load_pilot(kind,revision)
    else:
        from building_m3_art import geometry,face_layout,contract
        meshes=geometry(kind,revision);layout=face_layout(kind,revision);_,spec=contract(kind,revision)
    images,snapshot=load_sources();image=Image.new("RGBA",(512,512),(38,29,33,255));pixels=image.load()
    for role,faces in layout.items():
        mesh=meshes[role]
        for face,indices in zip(faces,mesh.faces,strict=True):
            x0,y0=(face["cell"]%16)*32,(face["cell"]//16)*32;points=[mesh.vertices[i] for i in indices]
            for py in range(y0,y0+32):
                for px in range(x0,x0+32):
                    point=local_point((px+.5,py+.5),face["polygon_px"],points)
                    pixels[px,py]=mixer_color(role,face["surface"],point,spec,images) if kind=="MudMixer" else rest_color(face["surface"],point,spec,images)
    image.save(path)
    for source in images.values():source.close()
    require(source_snapshot()==snapshot,"paint sources changed during atlas bake")
    return layout


def pack_in_blend():
    """Called only by the pinned safe-Blender source producer, before save."""
    import bpy
    import json
    snapshot=source_snapshot()
    for name,row in snapshot["images"].items():
        image=bpy.data.images.load(str(SOURCE_ROOT/row["path"]))
        image.name="HW_Handpaint_"+name
        image.pack()
    text=bpy.data.texts.new("HW_Handpaint_Sources.json")
    text.write(json.dumps(snapshot,sort_keys=True))
    require(source_snapshot()==snapshot,"paint inputs changed during pack")
    return snapshot
