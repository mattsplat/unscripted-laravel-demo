# The guns' models: writes public/wasm/outpost/guns/{rifle,magnum,sniper,fiveseven}.bin from
# outpost/models/ (Quaternius's Ultimate Gun Pack, CC0: see outpost/models/LICENSE.txt). Run:
#   python3 outpost/guns.py public/wasm/outpost/guns
#
# Each model's triangles, in a gun space the game places: the butt at z 0, the muzzle at z 1, the
# bore at y 0, x across (to the right). The game scales that to each gun's length. Each vertex is
# 10 little-endian float32s: x, y, z, normal x, y, z, red, green, blue, shiny (1 metal, 0 wood
# and paint).
import os, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
GUNS = {"rifle": "AssaultRifle_2", "magnum": "Pistol_6", "sniper": "SniperRifle_3", "fiveseven": "Pistol_5"}
SHINY = ("metal", "black", "grey", "glass")

def materials(path):
    out, name = {}, None
    for line in open(path):
        w = line.split()
        if w and w[0] == "newmtl":
            name = w[1]
        elif w and w[0] == "Kd":
            # Blender's colours are linear: the game's are as seen.
            out[name] = [round(float(c) ** (1 / 2.2), 3) for c in w[1:4]]
    return out

def model(name):
    mats = materials(os.path.join(HERE, "models", name + ".mtl"))
    vs, ns, faces, mat = [], [], [], None
    for line in open(os.path.join(HERE, "models", name + ".obj")):
        w = line.split()
        if not w:
            continue
        if w[0] == "v":
            vs.append([float(x) for x in w[1:4]])
        elif w[0] == "vn":
            ns.append([float(x) for x in w[1:4]])
        elif w[0] == "usemtl":
            mat = w[1]
        elif w[0] == "f":
            corners = [c.split("/") for c in w[1:]]
            faces.append(([int(c[0]) - 1 for c in corners], int(corners[0][2]) - 1, mat))
    # The barrel runs along +x, muzzle last; the bore's height is the barrel's at the muzzle.
    lo, hi = min(v[0] for v in vs), max(v[0] for v in vs)
    length = hi - lo
    tip = [v[1] for v in vs if v[0] > hi - 0.02 * length]
    bore = (min(tip) + max(tip)) / 2
    # Model (x, y, z) to gun space (-z, y, x): a turn, not a mirror, so faces still face out.
    place = lambda v: [-v[2] / length, (v[1] - bore) / length, (v[0] - lo) / length]
    turn = lambda n: [-n[2], n[1], n[0]]
    out = []
    for corners, normal, m in faces:
        colour = mats.get(m, [0.3, 0.3, 0.3])
        shiny = 1 if any(s in (m or "").lower() for s in SHINY) else 0
        n = turn(ns[normal])
        for i in range(1, len(corners) - 1):  # a fan of triangles
            for c in (corners[0], corners[i], corners[i + 1]):
                out += [round(x, 4) for x in place(vs[c])] + [round(x, 4) for x in n] + colour + [shiny]
    return out

os.makedirs(sys.argv[1], exist_ok=True)
for key, name in GUNS.items():
    floats = model(name)
    open(os.path.join(sys.argv[1], key + ".bin"), "wb").write(struct.pack("<%df" % len(floats), *floats))
    print(key, len(floats) // 10, "vertices")
