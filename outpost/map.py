# The map: writes resources/outpost/map.json, which the game (outpost/src) and the server
# (resources/components/outpost.blade.php) both read. Run: python3 outpost/map.py resources/outpost/map.json

import json, math, sys
# Materials: 0 sand, 1 plaster, 2 stone, 3 crate, 4 concrete, 5 dark wood, 6 brick
SAND, PLASTER, STONE, CRATE, CONCRETE, BEAM, BRICK = range(7)
boxes = []
def box(x0, y0, z0, x1, y1, z1, m):
    boxes.append([min(x0,x1), y0, min(z0,z1), max(x0,x1), y1, max(z0,z1), m])
def both(x0, y0, z0, x1, y1, z1, m):  # east side and its mirror in the west
    box(x0, y0, z0, x1, y1, z1, m)
    box(-x0, y0, z0, -x1, y1, z1, m)
def crate(x, z, y=0.0, s=1.1):
    both(x - s/2, y, z - s/2, x + s/2, y + s, z + s/2, CRATE)

# Ground and the outer walls.
box(-33, -1, -23, 33, 0, 23, SAND)
box(-33, 0, 22, 33, 7, 23, PLASTER); box(-33, 0, -23, 33, 7, -22, PLASTER)
box(-33, 0, -23, -32, 7, 23, PLASTER); box(32, 0, -23, 33, 7, 23, PLASTER)

# North block (z 6..12): one doorway in the middle.
box(-14, 0, 6, -1.5, 5, 12, PLASTER); box(1.5, 0, 6, 14, 5, 12, PLASTER)
box(-1.5, 2.7, 6, 1.5, 5, 12, PLASTER)
box(-1.7, 2.55, 5.85, 1.7, 2.75, 6.0, BEAM)             # door lintel
box(-14, 4.55, 5.85, 14, 4.75, 6.0, BEAM)               # roof beam along the front
both(5, 2.8, 5.9, 6.2, 4.0, 6.0, BEAM)                   # shutters
both(10, 2.8, 5.9, 11.2, 4.0, 6.0, BEAM)
# South block (z -12..-6): brick middle, two doorways.
both(9, 0, -12, 14, 4.5, -6, PLASTER)
box(-7, 0, -12, 7, 4.5, -6, BRICK)
both(7, 2.7, -12, 9, 4.5, -6, PLASTER)
both(6.85, 2.55, -6.0, 9.15, 2.75, -5.85, BEAM)
both(3, 2.6, -6.0, 4.2, 3.8, -5.9, BEAM)

# Mid: the well, crates.
box(-1.4, 0, -1.4, 1.4, 0.9, 1.4, STONE)
crate(5.55, 0)
crate(4.05, 3.55); crate(4.05, 3.55, 1.1)
crate(4.05, -3.55)

# Spawn walls (x 20..21), with two wide openings each side, and cover in spawn.
both(20, 0, -22, 21, 3.2, -15, STONE)
both(20, 0, -3.5, 21, 3.2, 3.5, STONE)
both(20, 0, 15, 21, 3.2, 22, STONE)
crate(26, 6); crate(26, -6)
crate(17.5, -9.5)
crate(17.5, 9.5)

# North alley: a raised platform with a step, crates, a barrier.
both(16, 0, 16, 20, 1.0, 22, STONE)
both(15.4, 0, 16, 16, 0.45, 18, STONE)
crate(8.55, 16.55); crate(9.65, 16.55); crate(9.1, 16.55, 1.1)
both(2, 0, 14.5, 5, 1.0, 15.2, CONCRETE)

# South lane: barriers, a crate stack, a low wall.
both(10, 0, -18, 10.7, 1.1, -14.5, CONCRETE)
crate(4.55, -17.95); crate(4.55, -17.95, 1.1)
both(16, 0, -14.5, 19, 1.0, -14.0, STONE)
crate(13, -20.5)

spawns = {
    "raiders": [[-28, 0, z, math.pi / 2] for z in (-10, -5, 0, 5, 10)],
    "guards": [[28, 0, z, -math.pi / 2] for z in (-10, -5, 0, 5, 10)],
}
# No spawn may start inside something.
for team in spawns.values():
    for x, y, z, _ in team:
        for b in boxes[1:]:
            if x - .3 < b[3] and x + .3 > b[0] and y < b[4] and y + 1.8 > b[1] and z - .3 < b[5] and z + .3 > b[2]:
                sys.exit(f"spawn {x},{z} is inside {b}")
r = lambda n: round(n, 3)
out = {"boxes": [[r(n) for n in b[:6]] + [b[6]] for b in boxes], "spawns": {k: [[r(n) for n in s] for s in v] for k, v in spawns.items()}}
open(sys.argv[1], "w").write(json.dumps(out, separators=(",", ":")))
print(len(boxes), "boxes")
