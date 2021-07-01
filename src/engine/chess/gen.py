from functools import reduce
import random

def calculate_neighbours(n):
    neighbours = 0
    if n < 1<<56:
        neighbours |= n << 8
    if n > 1<<7:
        neighbours |= n >> 8
    if n & 0x8080808080808080 == 0:
        neighbours |= n << 1
    if n & 0x0101010101010101 == 0:
        neighbours |= n >> 1
    if n < 1<<56 and n & 0x8080808080808080 == 0:
        neighbours |= n << 9
    if n < 1<<56 and n & 0x0101010101010101 == 0:
        neighbours |= n << 7
    if n > 1<< 7 and n & 0x8080808080808080 == 0:
        neighbours |= n >> 7
    if n > 1 <<7 and n & 0x0101010101010101 == 0:
        neighbours |= n >> 9
    return neighbours

def print_as_board(n):
    for y in range(8):
        line = ""
        for x in range(8):
            if n >> (x+8*(7-y)) & 1 == 1:
                line = line + " *"
            else:
                line = line + " ."
        print(line)
    print("")


def calculate_rook_moves(n):
    moves = 0
    i = 1
    while (n << 8*i < 1 << 56):
        moves |= n << 8*i
        i += 1
    i = 1
    while (n >> 8*i > 1 << 7):
        moves |= n >> 8*i
        i += 1
    i = 1
    while (n << i) & 0x8080808080808080 == 0 and n & 0x8080808080808080 == 0:
        moves |= n << i
        i += 1
    i = 1
    while (n >> i) & 0x0101010101010101 == 0 and n & 0x0101010101010101 == 0:
        moves |= n >> i
        i += 1
    return moves

def pext(n, mask):
    outbit = 0
    out = 0
    for i in range(64):
        if 1 << i & mask != 0:
            out = out | ((n >> i) & 1) << outbit
            outbit += 1
    return out

def pdep(n, mask):
    inbit = 0
    out = 0
    for i in range(64):
        if 1 << i & mask != 0:
            out |= ((n >> inbit) & 1) << i
            inbit += 1
    return out

def set_bit(n, bit, val):
    n |= 1 << bit
    if val == 0:
        n ^= 1 << bit
    return n

def get_bit(n, bit):
    return (n >> bit) & 1

def popcnt(n):
    res = 0
    for i in range(64):
        res += (n >> i) & 1
    return res

def to_coordinates(n):
    i = 0
    while n & (1 << i) == 0:
        i += 1
    return i % 8, i // 8


def calculate_rook_move_masks(n):
    masks = []
    bits = 10
    if n < 1 << 8 or n > 1 << 55 or n & 0x0101010101010101 != 0 or n & 0x8080808080808080 != 0:
        bits = 11
    if n < 1 << 8 and (n & 0x0101010101010101 != 0 or n & 0x8080808080808080 != 0):
        bits = 12
    if n > 1 << 55 and (n & 0x0101010101010101 != 0 or n & 0x8080808080808080 != 0):
        bits = 12
    for i in range(2**bits):
        blockers = pdep(i, calculate_rook_moves(n))
        x0, y0 = to_coordinates(n)
        moves = 0
        for y in range(y0+1,8):
            moves = set_bit(moves, x0 + 8*y, 1)
            if get_bit(blockers, x0 + 8*y) == 1:
                break
        for y in range(y0-1,-1,-1):
            moves = set_bit(moves, x0 + 8*y, 1)
            if get_bit(blockers, x0 + 8*y) == 1:
                break
        for x in range(x0+1,8):
            moves = set_bit(moves, x + 8*y0, 1)
            if get_bit(blockers, x + 8*y0) == 1:
                break
        for x in range(x0-1,-1,-1):
            moves = set_bit(moves, x + 8*y0, 1)
            if get_bit(blockers, x + 8*y0) == 1:
                break
        masks += [moves]
    return masks

def calculate_bishop_masks(n):
    masks = []
    bmoves = calculate_bishop_moves(n)
    offset = 2**popcnt(bmoves)
    for i in range(offset):
        blockers = pdep(i, bmoves)
        x0,y0 = to_coordinates(n)
        moves = 0
        for x,y in zip(range(x0+1,8),range(y0+1,8)):
            moves = set_bit(moves, x+8*y,1)
            if get_bit(blockers,x+8*y) == 1:
                break
        for x,y in zip(range(x0-1,-1,-1),range(y0+1,8)):
            moves = set_bit(moves, x+8*y,1)
            if get_bit(blockers,x+8*y) == 1:
                break
        for x,y in zip(range(x0+1,8),range(y0-1,-1,-1)):
            moves = set_bit(moves, x+8*y,1)
            if get_bit(blockers,x+8*y) == 1:
                break
        for x,y in zip(range(x0-1,-1,-1),range(y0-1,-1,-1)):
            moves = set_bit(moves, x+8*y,1)
            if get_bit(blockers,x+8*y) == 1:
                break
        masks += [moves]
    return masks


def calculate_bishop_moves(n):
    moves = 0
    i = 1
    while (n << 8*(i-1)) < 1<<48 and (n << i) & 0x8080808080808080 == 0 and n & 0x8080808080808080 == 0:
        moves |= n << i*9
        i += 1
    i = 1
    while (n << 8*(i-1)) < 1<<48 and (n >> i) & 0x0101010101010101 == 0 and n & 0x0101010101010101 == 0:
        moves |= n << i*7
        i += 1
    i = 1
    while (n >> (i-1)*8) > 1 << 15 and (n << i) & 0x8080808080808080 == 0 and n & 0x8080808080808080 == 0:
        moves |= n >> i*7
        i += 1
    i = 1
    while (n >> (i-1)*8) > 1 << 15 and (n >> i) & 0x0101010101010101 == 0 and n & 0x0101010101010101 == 0:
        moves |= n >> i*9
        i += 1
    return moves

def calculate_knight_moves(n):
    moves = 0
    #moves up
    if n < 1 << 48 and n & 0x8080808080808080 == 0:
        moves |= n << 17
    if n < 1 << 48 and n & 0x0101010101010101 == 0:
        moves |= n << 15
    #moves south
    if n > 1 << 15 and n & 0x8080808080808080 == 0:
        moves |= n >> 15
    if n > 1 << 15 and n & 0x0101010101010101 == 0:
        moves |= n >> 17
    #knight moves west
    if n & 0x0303030303030303 == 0 and n < 1 << 56:
        moves |= n << 6
    if n & 0x0303030303030303 == 0 and n > 1 << 7:
        moves |= n >> 10
    #knight moves east
    if n & 0xc0c0c0c0c0c0c0c0 == 0 and n < 1 << 56:
        moves |= n << 10
    if n & 0xc0c0c0c0c0c0c0c0 == 0 and n > 1 << 7:
        moves |= n >> 6
    return moves

def calculate_connecting_ray(n,m):
    x0,y0 = to_coordinates(n)
    x1,y1 = to_coordinates(m)
    ray = 0
    if x0 == x1 and y0 == y1:
        return 0
    if x0 == x1:
        for y in range(min(y0,y1),max(y0,y1)+1):
            ray |= 1 << (x0 + 8*y)
    elif y0 == y1:
        for x in range(min(x0,x1),max(x0,x1)+1):
            ray |= 1 << (x + 8*y0)
    elif y0-x0 == y1-x1:
        for x in range(8):
            for y in range(8):
                if x-y==x0-y0 and x >= min(x0,x1) and x <= max(x0,x1):
                    ray |= 1 << (x + y*8)
    elif y0+x0 == y1+x1:
        for x in range(8):
            for y in range(8):
                if x+y==x0+y0 and x >= min(x0,x1) and x <= max(x0,x1):
                    ray |= 1 << (x + y*8)
    return ray

def calculate_pinning_ray(n,m):
    x0,y0 = to_coordinates(n)
    x1,y1 = to_coordinates(m)
    ray = 0
    if x0 == x1 and y0 == y1:
        return 0
    if x0 == x1:
        for y in range(0,8):
            ray |= 1 << (x0 + 8*y)
    elif y0 == y1:
        for x in range(0,8):
            ray |= 1 << (x + 8*y0)
    elif y0-x0 == y1-x1:
        for x in range(8):
            for y in range(8):
                if x-y==x0-y0:
                    ray |= 1 << (x + y*8)
    elif y0+x0 == y1+x1:
        for x in range(8):
            for y in range(8):
                if x+y==x0+y0:
                    ray |= 1 << (x + y*8)
    return ray

print("pub static BISHOP_MOVES: [u64;64] = [")
for i in range(64):
    moves = calculate_bishop_moves(1 << i)
    print("    0x{0:016x},".format(moves))
print("];")

print("pub static ROOK_MOVES: [u64;64] = [")
for i in range(64):
    moves = calculate_rook_moves(1 << i)
    print("    0x{0:016x},".format(moves))
print("];")

print("pub static NEIGHBOURS: [u64;64] = [")
for i in range(64):
    moves = calculate_neighbours(1 << i)
    print("    0x{0:016x},".format(moves))
print("];")

print("pub static KNIGHT_MOVES: [u64;64] = [")
for i in range(64):
    moves = calculate_knight_moves(1 << i)
    print("    0x{0:016x},".format(moves))
print("];")

offsets = [0]
print("pub static ROOK_MMASK: [u64;102400] = [")
for i in range(64):
    masks = calculate_rook_move_masks(1 << i)
    offsets += [offsets[-1]+len(masks)]
    for mask in masks:
        pass
        print("    0x{0:016x},".format(mask))
print("];")
offsets = offsets[:64]

print("pub static ROOK_MMASK_OFFSETS: [u64;64] = [")
for i in range(64):
    print("    0x{0:04x},".format(offsets[i]))
print("];")

offsets = [0]
print("pub static BISHOP_MMASK: [u64;5248] = [")
for i in range(64):
    masks = calculate_bishop_masks(1 << i)
    offsets += [offsets[-1]+len(masks)]
    for mask in masks:
        pass
        print("    0x{0:016x},".format(mask))
print("];")

offsets = offsets[:64]

print("pub static BISHOP_MMASK_OFFSETS: [u64;64] = [")
for i in range(64):
    print("    0x{0:04x},".format(offsets[i]))
print("];")

print ("pub static RAYS: [[u64; 64]; 64] = [")
for i in range(64):
    print("    [")
    for j in range(64):
        print("        0x{0:016x},".format(calculate_pinning_ray(1<<i, 1<<j)))
    print("    ],")
print("];")

print ("pub static CONNECTING_RAYS: [[u64; 64]; 64] = [")
for i in range(64):
    print("    [")
    for j in range(64):
        print("        0x{0:016x},".format(calculate_connecting_ray(1<<i, 1<<j)))
    print("    ],")
print("];")

for p in ["KING","QUEEN","BISHOP","KNIGHT","ROOK","PAWN"]:
    for c in ["BLACK", "WHITE"]:
        print("pub static ZOBRIST_{}_{}_NUMBERS: [u64;64] = [".format(c,p))
        for _ in range(64):
            print("    0x{:016x},".format(random.randint(0,18446744073709551615)))
        print("];")
print("pub static ZOBRIST_CASTLING_NUMBERS: [u64;4] = [")
for _ in range(4):
    print("    0x{:016x},".format(random.randint(0,18446744073709551615)))
print("];")
print("pub static ZOBRIST_ENPASSANT_NUMBERS: [u64;8] = [")
for _ in range(8):
    print("    0x{:016x},".format(random.randint(0,18446744073709551615)))
print("];")
print("pub static ZOBRIST_BLACK_NUMBER: u64 = 0x{:016x};".format(random.randint(0,18446744073709551615)))

